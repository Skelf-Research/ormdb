//! Query executor for running planned queries.
//!
//! The executor takes a query plan and executes it against the storage engine,
//! returning EntityBlock and EdgeBlock results.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::catalog::Catalog;
use crate::error::Error;
use crate::storage::StorageEngine;

use super::cache::{PlanCache, QueryFingerprint};
use super::cost::CostModel;
use super::filter::FilterEvaluator;
use super::join::{execute_join, EntityRow, JoinStrategy};
use super::planner::{FanoutBudget, IncludePlan, QueryPlan, QueryPlanner};
use super::statistics::TableStatistics;
use super::value_codec::decode_entity;

use ormdb_proto::{
    ColumnData, Edge, EdgeBlock, EntityBlock, GraphQuery, OrderDirection, QueryResult, Value,
};

/// Query executor that runs queries against storage.
pub struct QueryExecutor<'a> {
    storage: &'a StorageEngine,
    catalog: &'a Catalog,
}

impl<'a> QueryExecutor<'a> {
    /// Create a new executor with storage and catalog references.
    pub fn new(storage: &'a StorageEngine, catalog: &'a Catalog) -> Self {
        Self { storage, catalog }
    }

    /// Execute a GraphQuery and return results.
    pub fn execute(&self, query: &GraphQuery) -> Result<QueryResult, Error> {
        self.execute_with_budget(query, FanoutBudget::default())
    }

    /// Execute a query with a custom fanout budget.
    pub fn execute_with_budget(
        &self,
        query: &GraphQuery,
        budget: FanoutBudget,
    ) -> Result<QueryResult, Error> {
        // Plan the query
        let planner = QueryPlanner::new(self.catalog);
        let plan = planner.plan_with_budget(query, budget)?;

        // Execute the plan
        self.execute_plan(&plan)
    }

    /// Execute a query with plan caching.
    ///
    /// This method uses a plan cache to avoid replanning identical queries.
    /// The cache is keyed by query structure (not literal values), so queries
    /// with the same shape but different filter values share the same plan.
    ///
    /// # Arguments
    /// * `query` - The query to execute
    /// * `cache` - Plan cache for storing/retrieving compiled plans
    /// * `statistics` - Optional statistics for cost-based optimization
    pub fn execute_with_cache(
        &self,
        query: &GraphQuery,
        cache: &PlanCache,
        statistics: Option<&TableStatistics>,
    ) -> Result<QueryResult, Error> {
        let fingerprint = QueryFingerprint::from_query(query);
        let schema_version = self.catalog.current_version();

        // Try to get cached plan
        if let Some(mut plan) = cache.get(&fingerprint) {
            // Optionally optimize with current statistics
            if statistics.is_some() {
                plan.optimize_include_order();
            }
            return self.execute_plan(&plan);
        }

        // Plan the query
        let planner = QueryPlanner::new(self.catalog);
        let mut plan = planner.plan(query)?;

        // Optimize include order if statistics available
        if statistics.is_some() {
            plan.optimize_include_order();
            plan.deduplicate_includes();
        }

        // Cache the plan
        cache.insert(fingerprint, plan.clone(), schema_version);

        // Execute
        self.execute_plan(&plan)
    }

    /// Execute a query with statistics for cost-based join strategy selection.
    ///
    /// This uses the cost model to choose optimal join strategies based on
    /// actual table statistics rather than hardcoded estimates.
    pub fn execute_with_statistics(
        &self,
        query: &GraphQuery,
        statistics: &TableStatistics,
    ) -> Result<QueryResult, Error> {
        let planner = QueryPlanner::new(self.catalog);
        let mut plan = planner.plan(query)?;

        // Optimize based on statistics
        plan.optimize_include_order();
        plan.deduplicate_includes();

        // Execute with cost model
        self.execute_plan_with_cost_model(&plan, statistics)
    }

    /// Execute a pre-planned query using cost model for join strategy.
    fn execute_plan_with_cost_model(
        &self,
        plan: &QueryPlan,
        statistics: &TableStatistics,
    ) -> Result<QueryResult, Error> {
        let cost_model = CostModel::new(statistics);

        // Fetch and filter root entities
        let mut rows = self.fetch_entities(plan)?;

        // Check entity budget
        if rows.len() > plan.budget.max_entities {
            return Err(Error::InvalidData(format!(
                "Query would return {} entities, exceeding budget of {}",
                rows.len(),
                plan.budget.max_entities
            )));
        }

        // Apply sorting
        self.sort_rows(&mut rows, &plan.order_by);

        // Apply pagination and track has_more
        let has_more = self.apply_pagination(&mut rows, &plan.pagination);

        // Collect root entity IDs for relation resolution
        let root_ids: Vec<[u8; 16]> = rows.iter().map(|r| r.id).collect();

        // Build root entity block
        let root_block = self.build_entity_block(&plan.root_entity, rows, &plan.fields);

        // Resolve includes with cost-based strategy selection
        let (related_blocks, edge_blocks) =
            self.resolve_includes_with_cost_model(&root_ids, &plan.includes, &plan.budget, &cost_model)?;

        // Combine all entity blocks
        let mut entities = vec![root_block];
        entities.extend(related_blocks);

        Ok(QueryResult::new(entities, edge_blocks, has_more))
    }

    /// Resolve relation includes using cost model for strategy selection.
    fn resolve_includes_with_cost_model(
        &self,
        parent_ids: &[[u8; 16]],
        includes: &[IncludePlan],
        budget: &FanoutBudget,
        cost_model: &CostModel,
    ) -> Result<(Vec<EntityBlock>, Vec<EdgeBlock>), Error> {
        if includes.is_empty() {
            return Ok((vec![], vec![]));
        }

        let mut entity_blocks = Vec::new();
        let mut edge_blocks = Vec::new();
        let mut total_edges = 0;

        let mut resolved_ids: HashMap<String, Vec<[u8; 16]>> = HashMap::new();
        resolved_ids.insert(String::new(), parent_ids.to_vec());

        for include in includes {
            let source_ids = if include.is_top_level() {
                parent_ids
            } else {
                let parent_path = include.parent_path().unwrap();
                resolved_ids.get(parent_path).map(|v| v.as_slice()).ok_or_else(|| {
                    Error::InvalidData(format!(
                        "Parent path '{}' not resolved for include '{}'",
                        parent_path, include.path
                    ))
                })?
            };

            if source_ids.is_empty() {
                continue;
            }

            // Use cost model to estimate child count for strategy selection
            let estimated_child_count = cost_model.estimated_child_count(include);
            let strategy = JoinStrategy::select(source_ids.len(), estimated_child_count);

            // Execute join with selected strategy
            let (mut rows, edges) = execute_join(strategy, self.storage, source_ids, include)?;

            // Check edge budget
            total_edges += edges.len();
            if total_edges > budget.max_edges {
                return Err(Error::InvalidData(format!(
                    "Query would return {} edges, exceeding budget of {}",
                    total_edges, budget.max_edges
                )));
            }

            // Apply sorting
            self.sort_rows(&mut rows, &include.order_by);

            // Apply per-parent pagination if needed
            let (rows, edges) = if let Some(pagination) = &include.pagination {
                let source_id_set: HashSet<[u8; 16]> = source_ids.iter().cloned().collect();
                self.apply_per_parent_pagination(&rows, &edges, &source_id_set, pagination)
            } else {
                (rows, edges)
            };

            // Store resolved IDs
            let resolved: Vec<[u8; 16]> = rows.iter().map(|r| r.id).collect();
            resolved_ids.insert(include.path.clone(), resolved);

            // Build blocks
            let block = self.build_entity_block(include.target_entity(), rows, &include.fields);
            entity_blocks.push(block);

            if !edges.is_empty() {
                edge_blocks.push(EdgeBlock::with_edges(&include.path, edges));
            }
        }

        Ok((entity_blocks, edge_blocks))
    }

    /// Execute a pre-planned query.
    pub fn execute_plan(&self, plan: &QueryPlan) -> Result<QueryResult, Error> {
        // Fetch and filter root entities
        let mut rows = self.fetch_entities(plan)?;

        // Check entity budget
        if rows.len() > plan.budget.max_entities {
            return Err(Error::InvalidData(format!(
                "Query would return {} entities, exceeding budget of {}",
                rows.len(),
                plan.budget.max_entities
            )));
        }

        // Apply sorting
        self.sort_rows(&mut rows, &plan.order_by);

        // Apply pagination and track has_more
        let has_more = self.apply_pagination(&mut rows, &plan.pagination);

        // Collect root entity IDs for relation resolution
        let root_ids: Vec<[u8; 16]> = rows.iter().map(|r| r.id).collect();

        // Build root entity block
        let root_block = self.build_entity_block(&plan.root_entity, rows, &plan.fields);

        // Resolve includes
        let (related_blocks, edge_blocks) =
            self.resolve_includes(&root_ids, &plan.includes, &plan.budget)?;

        // Combine all entity blocks
        let mut entities = vec![root_block];
        entities.extend(related_blocks);

        Ok(QueryResult::new(entities, edge_blocks, has_more))
    }

    /// Fetch entities of a given type, applying filters.
    fn fetch_entities(&self, plan: &QueryPlan) -> Result<Vec<EntityRow>, Error> {
        let mut rows = Vec::new();

        for result in self.storage.scan_entity_type(&plan.root_entity) {
            let (entity_id, _version_ts, record) = result?;

            // Decode the entity data
            let fields = decode_entity(&record.data)?;

            // Apply filter if present
            if let Some(filter) = &plan.filter {
                if !FilterEvaluator::evaluate(filter, &fields)? {
                    continue;
                }
            }

            rows.push(EntityRow {
                id: entity_id,
                fields,
            });
        }

        Ok(rows)
    }

    /// Sort rows according to order specifications.
    fn sort_rows(&self, rows: &mut [EntityRow], order_by: &[ormdb_proto::OrderSpec]) {
        if order_by.is_empty() {
            return;
        }

        rows.sort_by(|a, b| {
            for spec in order_by {
                let a_val = a.fields.iter().find(|(n, _)| n == &spec.field).map(|(_, v)| v);
                let b_val = b.fields.iter().find(|(n, _)| n == &spec.field).map(|(_, v)| v);

                let cmp = Self::compare_values_opt(a_val, b_val);

                let cmp = match spec.direction {
                    OrderDirection::Asc => cmp,
                    OrderDirection::Desc => cmp.reverse(),
                };

                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            Ordering::Equal
        });
    }

    /// Compare two optional values for sorting.
    fn compare_values_opt(a: Option<&Value>, b: Option<&Value>) -> Ordering {
        match (a, b) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less, // NULLs first
            (Some(_), None) => Ordering::Greater,
            (Some(av), Some(bv)) => Self::compare_values(av, bv),
        }
    }

    /// Compare two values for sorting.
    fn compare_values(a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Int32(a), Value::Int32(b)) => a.cmp(b),
            (Value::Int64(a), Value::Int64(b)) => a.cmp(b),
            (Value::Int32(a), Value::Int64(b)) => (*a as i64).cmp(b),
            (Value::Int64(a), Value::Int32(b)) => a.cmp(&(*b as i64)),
            (Value::Float32(a), Value::Float32(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Float32(a), Value::Float64(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (Value::Float64(a), Value::Float32(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Timestamp(a), Value::Timestamp(b)) => a.cmp(b),
            (Value::Uuid(a), Value::Uuid(b)) => a.cmp(b),
            (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),
            _ => Ordering::Equal, // Incompatible types are considered equal
        }
    }

    /// Apply pagination to rows. Returns true if there are more results.
    fn apply_pagination(
        &self,
        rows: &mut Vec<EntityRow>,
        pagination: &Option<ormdb_proto::Pagination>,
    ) -> bool {
        if let Some(pag) = pagination {
            let offset = pag.offset as usize;
            let limit = pag.limit as usize;

            // Apply offset
            if offset > 0 {
                if offset >= rows.len() {
                    rows.clear();
                    return false;
                }
                rows.drain(0..offset);
            }

            // Apply limit
            if limit < rows.len() {
                rows.truncate(limit);
                return true; // More results available
            }
        }
        false
    }

    /// Build an EntityBlock from rows.
    fn build_entity_block(
        &self,
        entity_type: &str,
        rows: Vec<EntityRow>,
        projected_fields: &[String],
    ) -> EntityBlock {
        if rows.is_empty() {
            return EntityBlock::new(entity_type);
        }

        let ids: Vec<[u8; 16]> = rows.iter().map(|r| r.id).collect();

        // Determine which fields to include
        let field_names: Vec<String> = if projected_fields.is_empty() {
            // Include all fields from the first row
            rows[0].fields.iter().map(|(n, _)| n.clone()).collect()
        } else {
            projected_fields.to_vec()
        };

        // Build columns
        let columns: Vec<ColumnData> = field_names
            .iter()
            .map(|name| {
                let values: Vec<Value> = rows
                    .iter()
                    .map(|row| {
                        row.fields
                            .iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(Value::Null)
                    })
                    .collect();
                ColumnData::new(name.clone(), values)
            })
            .collect();

        EntityBlock::with_data(entity_type, ids, columns)
    }

    /// Resolve relation includes, returning related entity blocks and edge blocks.
    fn resolve_includes(
        &self,
        parent_ids: &[[u8; 16]],
        includes: &[IncludePlan],
        budget: &FanoutBudget,
    ) -> Result<(Vec<EntityBlock>, Vec<EdgeBlock>), Error> {
        if includes.is_empty() {
            return Ok((vec![], vec![]));
        }

        let mut entity_blocks = Vec::new();
        let mut edge_blocks = Vec::new();
        let mut total_edges = 0;

        // Process includes in order (parent includes must come before nested ones)

        // Map from path -> resolved entity IDs (for nested includes)
        let mut resolved_ids: HashMap<String, Vec<[u8; 16]>> = HashMap::new();

        // Root entities have empty path
        resolved_ids.insert(String::new(), parent_ids.to_vec());

        for include in includes {
            // Find the source entity IDs for this include
            let source_ids = if include.is_top_level() {
                parent_ids
            } else {
                let parent_path = include.parent_path().unwrap();
                resolved_ids.get(parent_path).map(|v| v.as_slice()).ok_or_else(|| {
                    Error::InvalidData(format!(
                        "Parent path '{}' not resolved for include '{}'",
                        parent_path, include.path
                    ))
                })?
            };

            if source_ids.is_empty() {
                continue;
            }

            // Resolve this include
            let (rows, edges) = self.resolve_single_include(source_ids, include)?;

            // Check edge budget
            total_edges += edges.len();
            if total_edges > budget.max_edges {
                return Err(Error::InvalidData(format!(
                    "Query would return {} edges, exceeding budget of {}",
                    total_edges, budget.max_edges
                )));
            }

            // Store the resolved IDs for nested includes
            let resolved: Vec<[u8; 16]> = rows.iter().map(|r| r.id).collect();
            resolved_ids.insert(include.path.clone(), resolved);

            // Build entity block
            let block =
                self.build_entity_block(include.target_entity(), rows, &include.fields);
            entity_blocks.push(block);

            // Build edge block
            if !edges.is_empty() {
                edge_blocks.push(EdgeBlock::with_edges(&include.path, edges));
            }
        }

        Ok((entity_blocks, edge_blocks))
    }

    /// Resolve a single include, returning related rows and edges.
    ///
    /// Uses hash join for larger datasets (>100 parents or >1000 estimated children)
    /// and nested loop for smaller datasets to minimize overhead.
    fn resolve_single_include(
        &self,
        source_ids: &[[u8; 16]],
        include: &IncludePlan,
    ) -> Result<(Vec<EntityRow>, Vec<Edge>), Error> {
        // Select join strategy based on cardinality
        // For now, use a simple heuristic based on parent count
        // TODO: Use statistics for better estimation of child count
        let estimated_child_count = 1000; // Conservative estimate
        let strategy = JoinStrategy::select(source_ids.len(), estimated_child_count);

        // Execute the join
        let (mut rows, edges) = execute_join(strategy, self.storage, source_ids, include)?;

        // Apply sorting for this include
        self.sort_rows(&mut rows, &include.order_by);

        // For includes, pagination is per-parent
        // Group rows by their source parent, apply pagination per group
        if let Some(pagination) = &include.pagination {
            let source_id_set: HashSet<[u8; 16]> = source_ids.iter().cloned().collect();
            let grouped = self.apply_per_parent_pagination(
                &rows,
                &edges,
                &source_id_set,
                pagination,
            );
            return Ok(grouped);
        }

        Ok((rows, edges))
    }

    /// Apply pagination per parent entity.
    fn apply_per_parent_pagination(
        &self,
        rows: &[EntityRow],
        edges: &[Edge],
        _source_ids: &HashSet<[u8; 16]>,
        pagination: &ormdb_proto::Pagination,
    ) -> (Vec<EntityRow>, Vec<Edge>) {
        // Group edges by source (from_id)
        let mut by_parent: HashMap<[u8; 16], Vec<usize>> = HashMap::new();
        for (idx, edge) in edges.iter().enumerate() {
            by_parent.entry(edge.from_id).or_default().push(idx);
        }

        let mut result_rows = Vec::new();
        let mut result_edges = Vec::new();

        for (_parent_id, indices) in by_parent {
            let offset = pagination.offset as usize;
            let limit = pagination.limit as usize;

            let start = offset.min(indices.len());
            let end = (offset + limit).min(indices.len());

            for &edge_idx in &indices[start..end] {
                let edge = &edges[edge_idx];

                // Find the corresponding row
                if let Some(row) = rows.iter().find(|r| r.id == edge.to_id) {
                    result_rows.push(row.clone());
                    result_edges.push(edge.clone());
                }
            }
        }

        (result_rows, result_edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle};
    use crate::storage::{Record, StorageConfig, VersionedKey};
    use super::super::value_codec::encode_entity;

    struct TestDb {
        storage: StorageEngine,
        catalog: Catalog,
        _dir: tempfile::TempDir,
        _db: sled::Db,
    }

    fn setup_test_db() -> TestDb {
        let dir = tempfile::tempdir().unwrap();
        let storage = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();

        // Create catalog in a separate sled db
        let catalog_db = sled::Config::new().temporary(true).open().unwrap();
        let catalog = Catalog::open(&catalog_db).unwrap();

        // Create schema with User -> Posts
        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("age", FieldType::Scalar(ScalarType::Int32)));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

        let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");

        let schema = SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post)
            .with_relation(user_posts);

        catalog.apply_schema(schema).unwrap();

        TestDb {
            storage,
            catalog,
            _dir: dir,
            _db: catalog_db,
        }
    }

    fn insert_user(db: &TestDb, id: [u8; 16], name: &str, age: i32) {
        let fields = vec![
            ("id".to_string(), Value::Uuid(id)),
            ("name".to_string(), Value::String(name.to_string())),
            ("age".to_string(), Value::Int32(age)),
        ];
        let data = encode_entity(&fields).unwrap();
        let key = VersionedKey::now(id);
        db.storage
            .put_typed("User", key, Record::new(data))
            .unwrap();
    }

    fn insert_post(db: &TestDb, id: [u8; 16], title: &str, author_id: [u8; 16]) {
        let fields = vec![
            ("id".to_string(), Value::Uuid(id)),
            ("title".to_string(), Value::String(title.to_string())),
            ("author_id".to_string(), Value::Uuid(author_id)),
        ];
        let data = encode_entity(&fields).unwrap();
        let key = VersionedKey::now(id);
        db.storage
            .put_typed("Post", key, Record::new(data))
            .unwrap();
    }

    #[test]
    fn test_simple_query() {
        let db = setup_test_db();

        // Insert users
        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        insert_user(&db, user1_id, "Alice", 30);
        insert_user(&db, user2_id, "Bob", 25);

        db.storage.flush().unwrap();

        // Query all users
        let executor = QueryExecutor::new(&db.storage, &db.catalog);
        let query = GraphQuery::new("User");
        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].entity, "User");
        assert_eq!(result.entities[0].len(), 2);
        assert!(!result.has_more);
    }

    #[test]
    fn test_query_with_filter() {
        let db = setup_test_db();

        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        insert_user(&db, user1_id, "Alice", 30);
        insert_user(&db, user2_id, "Bob", 25);

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        // Filter for age > 28
        let query = GraphQuery::new("User")
            .with_filter(ormdb_proto::FilterExpr::gt("age", Value::Int32(28)).into());

        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities[0].len(), 1);
        // Should only return Alice (age 30)
        let name_col = result.entities[0].column("name").unwrap();
        assert_eq!(name_col.values[0], Value::String("Alice".to_string()));
    }

    #[test]
    fn test_query_with_sorting() {
        let db = setup_test_db();

        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        let user3_id = StorageEngine::generate_id();
        insert_user(&db, user1_id, "Charlie", 35);
        insert_user(&db, user2_id, "Alice", 30);
        insert_user(&db, user3_id, "Bob", 25);

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        // Sort by name ascending
        let query =
            GraphQuery::new("User").with_order(ormdb_proto::OrderSpec::asc("name"));

        let result = executor.execute(&query).unwrap();

        let name_col = result.entities[0].column("name").unwrap();
        assert_eq!(name_col.values[0], Value::String("Alice".to_string()));
        assert_eq!(name_col.values[1], Value::String("Bob".to_string()));
        assert_eq!(name_col.values[2], Value::String("Charlie".to_string()));
    }

    #[test]
    fn test_query_with_pagination() {
        let db = setup_test_db();

        for i in 0..10 {
            let id = StorageEngine::generate_id();
            insert_user(&db, id, &format!("User{}", i), 20 + i);
        }

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        // Get first 3 users
        let query = GraphQuery::new("User")
            .with_order(ormdb_proto::OrderSpec::asc("name"))
            .with_pagination(ormdb_proto::Pagination::new(3, 0));

        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities[0].len(), 3);
        assert!(result.has_more);

        // Get next 3 with offset
        let query = GraphQuery::new("User")
            .with_order(ormdb_proto::OrderSpec::asc("name"))
            .with_pagination(ormdb_proto::Pagination::new(3, 3));

        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities[0].len(), 3);
        assert!(result.has_more);
    }

    #[test]
    fn test_query_with_field_projection() {
        let db = setup_test_db();

        let user_id = StorageEngine::generate_id();
        insert_user(&db, user_id, "Alice", 30);

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        // Only select name field
        let query = GraphQuery::new("User").with_fields(vec!["name".into()]);

        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities[0].columns.len(), 1);
        assert_eq!(result.entities[0].columns[0].name, "name");
    }

    #[test]
    fn test_query_with_include() {
        let db = setup_test_db();

        // Create user and posts
        let user_id = StorageEngine::generate_id();
        insert_user(&db, user_id, "Alice", 30);

        let post1_id = StorageEngine::generate_id();
        let post2_id = StorageEngine::generate_id();
        insert_post(&db, post1_id, "First Post", user_id);
        insert_post(&db, post2_id, "Second Post", user_id);

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        // Query users with posts
        let query = GraphQuery::new("User")
            .include(ormdb_proto::RelationInclude::new("posts"));

        let result = executor.execute(&query).unwrap();

        // Should have User and Post blocks
        assert_eq!(result.entities.len(), 2);
        assert_eq!(result.entities[0].entity, "User");
        assert_eq!(result.entities[1].entity, "Post");

        // Should have posts
        assert_eq!(result.entities[1].len(), 2);

        // Should have edges
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].relation, "posts");
        assert_eq!(result.edges[0].len(), 2);
    }

    #[test]
    fn test_empty_query_result() {
        let db = setup_test_db();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);
        let query = GraphQuery::new("User");
        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities.len(), 1);
        assert!(result.entities[0].is_empty());
    }

    #[test]
    fn test_query_with_like_filter() {
        let db = setup_test_db();

        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        insert_user(&db, user1_id, "Alice", 30);
        insert_user(&db, user2_id, "Bob", 25);

        db.storage.flush().unwrap();

        let executor = QueryExecutor::new(&db.storage, &db.catalog);

        let query = GraphQuery::new("User")
            .with_filter(ormdb_proto::FilterExpr::like("name", "A%").into());

        let result = executor.execute(&query).unwrap();

        assert_eq!(result.entities[0].len(), 1);
        let name_col = result.entities[0].column("name").unwrap();
        assert_eq!(name_col.values[0], Value::String("Alice".to_string()));
    }
}
