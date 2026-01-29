//! Query planner for transforming GraphQuery into execution plans.
//!
//! The planner resolves entity and relation definitions from the catalog,
//! validates the query, and produces an execution plan with fanout budgets.

use crate::catalog::{Cardinality, Catalog, EntityDef, RelationDef};
use crate::error::Error;
use ormdb_proto::{FilterExpr, GraphQuery, OrderSpec, Pagination, RelationInclude};

/// Budget limits for query execution to prevent runaway queries.
#[derive(Debug, Clone)]
pub struct FanoutBudget {
    /// Maximum total entities to fetch across all entity blocks.
    pub max_entities: usize,
    /// Maximum total edges to fetch across all edge blocks.
    pub max_edges: usize,
    /// Maximum depth of relation includes.
    pub max_depth: usize,
}

impl Default for FanoutBudget {
    fn default() -> Self {
        Self {
            max_entities: 10_000,
            max_edges: 50_000,
            max_depth: 5,
        }
    }
}

impl FanoutBudget {
    /// Create a budget with custom limits.
    pub fn new(max_entities: usize, max_edges: usize, max_depth: usize) -> Self {
        Self {
            max_entities,
            max_edges,
            max_depth,
        }
    }

    /// Create an unlimited budget (use with caution).
    pub fn unlimited() -> Self {
        Self {
            max_entities: usize::MAX,
            max_edges: usize::MAX,
            max_depth: usize::MAX,
        }
    }
}

/// An execution plan for a query.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// Root entity type to fetch.
    pub root_entity: String,
    /// Resolved entity definition.
    pub root_entity_def: EntityDef,
    /// Fields to project (empty = all fields).
    pub fields: Vec<String>,
    /// Filter to apply to root entities.
    pub filter: Option<FilterExpr>,
    /// Ordering specification.
    pub order_by: Vec<OrderSpec>,
    /// Pagination parameters.
    pub pagination: Option<Pagination>,
    /// Nested relation fetches.
    pub includes: Vec<IncludePlan>,
    /// Fanout budget for this query.
    pub budget: FanoutBudget,
}

/// Plan for fetching a related entity set.
#[derive(Debug, Clone)]
pub struct IncludePlan {
    /// Original path from query (e.g., "posts.comments").
    pub path: String,
    /// Relation definition.
    pub relation: RelationDef,
    /// Target entity definition.
    pub target_entity_def: EntityDef,
    /// Fields to project from related entity.
    pub fields: Vec<String>,
    /// Filter for related entities.
    pub filter: Option<FilterExpr>,
    /// Ordering for related entities.
    pub order_by: Vec<OrderSpec>,
    /// Pagination per parent (limit per parent entity).
    pub pagination: Option<Pagination>,
}

impl IncludePlan {
    /// Get the depth of this include (1 for top-level, 2 for nested, etc.)
    pub fn depth(&self) -> usize {
        self.path.matches('.').count() + 1
    }

    /// Check if this is a top-level include.
    pub fn is_top_level(&self) -> bool {
        !self.path.contains('.')
    }

    /// Get the parent path for nested includes.
    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('.').map(|(parent, _)| parent)
    }

    /// Get the target entity name from the relation.
    pub fn target_entity(&self) -> &str {
        &self.relation.to_entity
    }
}

impl QueryPlan {
    /// Optimize the include order by estimated fanout (smallest first).
    ///
    /// This reorders includes while preserving parent-before-child constraints.
    /// Processing smaller result sets first can reduce the overall work
    /// in subsequent joins.
    pub fn optimize_include_order(&mut self) {
        if self.includes.len() <= 1 {
            return;
        }

        // Build dependency graph: child path -> parent path
        let mut dependencies: std::collections::HashMap<&str, Option<&str>> =
            std::collections::HashMap::new();
        for include in &self.includes {
            dependencies.insert(&include.path, include.parent_path());
        }

        // Estimate cost for each include based on cardinality
        let mut costs: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for include in &self.includes {
            let cost = estimate_fanout(include.relation.cardinality);
            costs.insert(&include.path, cost);
        }

        // Topological sort with cost-based ordering within each level
        let mut sorted_indices: Vec<usize> = Vec::with_capacity(self.includes.len());
        let mut remaining: std::collections::HashSet<usize> =
            (0..self.includes.len()).collect();

        while !remaining.is_empty() {
            // Find includes whose dependencies are satisfied
            let mut available: Vec<usize> = remaining
                .iter()
                .filter(|&&idx| {
                    let path = &self.includes[idx].path;
                    match dependencies.get(path.as_str()) {
                        Some(Some(parent)) => {
                            // Check if parent is already in sorted_indices
                            sorted_indices.iter().any(|&i| self.includes[i].path == *parent)
                        }
                        _ => true, // Top-level or no dependency
                    }
                })
                .cloned()
                .collect();

            if available.is_empty() {
                // Should not happen with valid includes, but break to avoid infinite loop
                break;
            }

            // Sort available by cost (ascending)
            available.sort_by_key(|&idx| {
                costs.get(self.includes[idx].path.as_str()).unwrap_or(&0)
            });

            // Add the cheapest one
            if let Some(idx) = available.first() {
                sorted_indices.push(*idx);
                remaining.remove(idx);
            }
        }

        // Reorder includes based on sorted_indices
        let mut new_includes = Vec::with_capacity(self.includes.len());
        for idx in sorted_indices {
            new_includes.push(self.includes[idx].clone());
        }
        self.includes = new_includes;
    }

    /// Remove duplicate include paths, keeping the first occurrence.
    pub fn deduplicate_includes(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.includes.retain(|include| seen.insert(include.path.clone()));
    }
}

/// Planner that transforms GraphQuery into QueryPlan.
pub struct QueryPlanner<'a> {
    catalog: &'a Catalog,
}

impl<'a> QueryPlanner<'a> {
    /// Create a new planner with a catalog reference.
    pub fn new(catalog: &'a Catalog) -> Self {
        Self { catalog }
    }

    /// Plan a query with the default fanout budget.
    pub fn plan(&self, query: &GraphQuery) -> Result<QueryPlan, Error> {
        self.plan_with_budget(query, FanoutBudget::default())
    }

    /// Plan a query with a custom fanout budget.
    pub fn plan_with_budget(
        &self,
        query: &GraphQuery,
        budget: FanoutBudget,
    ) -> Result<QueryPlan, Error> {
        // Resolve root entity
        let root_entity_def = self
            .catalog
            .get_entity(&query.root_entity)?
            .ok_or(Error::NotFound)?;

        // Validate fields exist
        for field in &query.fields {
            if root_entity_def.get_field(field).is_none() {
                return Err(Error::InvalidData(format!(
                    "Unknown field '{}' on entity '{}'",
                    field, query.root_entity
                )));
            }
        }

        // Plan includes
        let includes = self.plan_includes(&query.root_entity, &query.includes, &budget)?;

        // Validate max depth
        let max_depth = includes.iter().map(|i| i.depth()).max().unwrap_or(0);
        if max_depth > budget.max_depth {
            return Err(Error::InvalidData(format!(
                "Query depth {} exceeds maximum allowed depth {}",
                max_depth, budget.max_depth
            )));
        }

        Ok(QueryPlan {
            root_entity: query.root_entity.clone(),
            root_entity_def,
            fields: query.fields.clone(),
            filter: query.filter.as_ref().map(|f| f.expression.clone()),
            order_by: query.order_by.clone(),
            pagination: query.pagination.clone(),
            includes,
            budget,
        })
    }

    /// Plan relation includes.
    fn plan_includes(
        &self,
        root_entity: &str,
        includes: &[RelationInclude],
        budget: &FanoutBudget,
    ) -> Result<Vec<IncludePlan>, Error> {
        let mut plans = Vec::with_capacity(includes.len());

        for include in includes {
            let plan = self.plan_single_include(root_entity, include, &plans, budget)?;
            plans.push(plan);
        }

        Ok(plans)
    }

    /// Plan a single include.
    fn plan_single_include(
        &self,
        root_entity: &str,
        include: &RelationInclude,
        existing_plans: &[IncludePlan],
        budget: &FanoutBudget,
    ) -> Result<IncludePlan, Error> {
        // Validate depth
        let depth = include.depth();
        if depth > budget.max_depth {
            return Err(Error::InvalidData(format!(
                "Include path '{}' exceeds maximum depth {}",
                include.path, budget.max_depth
            )));
        }

        // Resolve the source entity for this include
        let source_entity = if include.is_top_level() {
            root_entity.to_string()
        } else {
            // Find the parent include to determine source entity
            let parent_path = include.parent_path().unwrap();
            let parent_plan = existing_plans
                .iter()
                .find(|p| p.path == parent_path)
                .ok_or_else(|| {
                    Error::InvalidData(format!(
                        "Include '{}' references non-existent parent '{}'",
                        include.path, parent_path
                    ))
                })?;
            parent_plan.target_entity().to_string()
        };

        // Find the relation
        let relation_name = include.relation_name();
        let relations = self.catalog.relations_from(&source_entity)?;
        let relation = relations
            .iter()
            .find(|r| r.name == relation_name)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "Unknown relation '{}' on entity '{}'",
                    relation_name, source_entity
                ))
            })?;

        // Resolve target entity
        let target_entity_def = self
            .catalog
            .get_entity(&relation.to_entity)?
            .ok_or(Error::NotFound)?;

        // Validate fields
        for field in &include.fields {
            if target_entity_def.get_field(field).is_none() {
                return Err(Error::InvalidData(format!(
                    "Unknown field '{}' on entity '{}'",
                    field, relation.to_entity
                )));
            }
        }

        Ok(IncludePlan {
            path: include.path.clone(),
            relation: relation.clone(),
            target_entity_def,
            fields: include.fields.clone(),
            filter: include.filter.as_ref().map(|f| f.expression.clone()),
            order_by: include.order_by.clone(),
            pagination: include.pagination.clone(),
        })
    }
}

/// Estimate the fanout for a relation based on cardinality.
pub fn estimate_fanout(cardinality: Cardinality) -> usize {
    match cardinality {
        Cardinality::OneToOne => 1,
        Cardinality::OneToMany => 10, // Assume ~10 on the many side
        Cardinality::ManyToMany => 25, // Assume ~25 for many-to-many
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle};

    fn test_db() -> sled::Db {
        sled::Config::new().temporary(true).open().unwrap()
    }

    fn create_test_catalog() -> (sled::Db, Catalog) {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();

        // Create a schema with User -> Posts -> Comments
        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

        let comment = EntityDef::new("Comment", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("text", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("post_id", FieldType::Scalar(ScalarType::Uuid)));

        // User has many Posts (from User, to Post via author_id)
        let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");

        // Post belongs to User (inverse)
        let post_author = user_posts.inverse("author");

        // Post has many Comments
        let post_comments = RelationDef::one_to_many("comments", "Post", "id", "Comment", "post_id");

        let schema = SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post)
            .with_entity(comment)
            .with_relation(user_posts)
            .with_relation(post_author)
            .with_relation(post_comments);

        catalog.apply_schema(schema).unwrap();
        (db, catalog)
    }

    #[test]
    fn test_simple_query_plan() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User").with_fields(vec!["id".into(), "name".into()]);

        let plan = planner.plan(&query).unwrap();

        assert_eq!(plan.root_entity, "User");
        assert_eq!(plan.fields, vec!["id", "name"]);
        assert!(plan.includes.is_empty());
    }

    #[test]
    fn test_query_with_include() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

        let plan = planner.plan(&query).unwrap();

        assert_eq!(plan.includes.len(), 1);
        assert_eq!(plan.includes[0].path, "posts");
        assert_eq!(plan.includes[0].target_entity(), "Post");
    }

    #[test]
    fn test_nested_includes() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"));

        let plan = planner.plan(&query).unwrap();

        assert_eq!(plan.includes.len(), 2);
        assert_eq!(plan.includes[0].path, "posts");
        assert_eq!(plan.includes[1].path, "posts.comments");
        assert_eq!(plan.includes[1].target_entity(), "Comment");
        assert_eq!(plan.includes[1].depth(), 2);
    }

    #[test]
    fn test_unknown_entity_fails() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("Unknown");
        let result = planner.plan(&query);

        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_field_fails() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User").with_fields(vec!["nonexistent".into()]);

        let result = planner.plan(&query);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_relation_fails() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User").include(RelationInclude::new("unknown"));

        let result = planner.plan(&query);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_limit_enforced() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let budget = FanoutBudget::new(10000, 50000, 1); // Max depth 1

        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments")); // Depth 2

        let result = planner.plan_with_budget(&query, budget);
        assert!(result.is_err());
    }

    #[test]
    fn test_include_plan_helpers() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"));

        let plan = planner.plan(&query).unwrap();

        // First include
        assert!(plan.includes[0].is_top_level());
        assert_eq!(plan.includes[0].depth(), 1);
        assert_eq!(plan.includes[0].parent_path(), None);

        // Second include (nested)
        assert!(!plan.includes[1].is_top_level());
        assert_eq!(plan.includes[1].depth(), 2);
        assert_eq!(plan.includes[1].parent_path(), Some("posts"));
    }

    #[test]
    fn test_fanout_estimate() {
        assert_eq!(estimate_fanout(Cardinality::OneToOne), 1);
        assert_eq!(estimate_fanout(Cardinality::OneToMany), 10);
        assert_eq!(estimate_fanout(Cardinality::ManyToMany), 25);
    }

    #[test]
    fn test_missing_parent_include_fails() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        // Try to include posts.comments without including posts first
        let query = GraphQuery::new("User").include(RelationInclude::new("posts.comments"));

        let result = planner.plan(&query);
        assert!(result.is_err());
    }

    #[test]
    fn test_deduplicate_includes() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        // Create a plan with duplicate includes (manually)
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts")); // Duplicate

        let mut plan = planner.plan(&query).unwrap();

        // Should have 2 includes initially
        assert_eq!(plan.includes.len(), 2);

        // Deduplicate
        plan.deduplicate_includes();

        // Should have 1 include now
        assert_eq!(plan.includes.len(), 1);
        assert_eq!(plan.includes[0].path, "posts");
    }

    #[test]
    fn test_optimize_include_order_preserves_dependencies() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"));

        let mut plan = planner.plan(&query).unwrap();

        // Optimize order
        plan.optimize_include_order();

        // posts.comments depends on posts, so posts must come first
        assert_eq!(plan.includes[0].path, "posts");
        assert_eq!(plan.includes[1].path, "posts.comments");
    }

    #[test]
    fn test_optimize_single_include_is_noop() {
        let (_db, catalog) = create_test_catalog();
        let planner = QueryPlanner::new(&catalog);

        let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

        let mut plan = planner.plan(&query).unwrap();
        let original_path = plan.includes[0].path.clone();

        plan.optimize_include_order();

        assert_eq!(plan.includes.len(), 1);
        assert_eq!(plan.includes[0].path, original_path);
    }
}
