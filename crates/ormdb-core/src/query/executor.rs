//! Query executor for running planned queries.
//!
//! The executor takes a query plan and executes it against the storage engine,
//! returning EntityBlock and EdgeBlock results.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use tracing::{debug, instrument};

use crate::catalog::Catalog;
use crate::error::Error;
use crate::metrics::{AccessPath, JoinStrategyMetric, SharedMetricsRegistry};
use crate::storage::StorageEngine;

use super::cache::{PlanCache, QueryFingerprint};
use super::cost::CostModel;
use super::filter::{extract_filter_fields, FilterEvaluator};
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
    metrics: Option<SharedMetricsRegistry>,
}

impl<'a> QueryExecutor<'a> {
    /// Create a new executor with storage and catalog references.
    pub fn new(storage: &'a StorageEngine, catalog: &'a Catalog) -> Self {
        Self {
            storage,
            catalog,
            metrics: None,
        }
    }

    /// Create a new executor with metrics tracking.
    pub fn with_metrics(
        storage: &'a StorageEngine,
        catalog: &'a Catalog,
        metrics: SharedMetricsRegistry,
    ) -> Self {
        Self {
            storage,
            catalog,
            metrics: Some(metrics),
        }
    }

    /// Execute a GraphQuery and return results.
    #[instrument(skip(self, query), fields(entity = %query.root_entity))]
    pub fn execute(&self, query: &GraphQuery) -> Result<QueryResult, Error> {
        self.execute_with_budget(query, FanoutBudget::default())
    }

    /// Execute a query with a custom fanout budget.
    #[instrument(skip(self, query), fields(entity = %query.root_entity, max_entities = budget.max_entities))]
    pub fn execute_with_budget(
        &self,
        query: &GraphQuery,
        budget: FanoutBudget,
    ) -> Result<QueryResult, Error> {
        let start = std::time::Instant::now();

        // Plan the query
        let planner = QueryPlanner::new(self.catalog);
        let plan = planner.plan_with_budget(query, budget)?;

        // Execute the plan
        let result = self.execute_plan(&plan);
        let duration_us = start.elapsed().as_micros() as u64;

        if let Some(metrics) = &self.metrics {
            if result.is_ok() {
                metrics.record_query(&query.root_entity, duration_us);
            }
        }

        debug!(duration_us = duration_us, "query executed");
        result
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
    #[instrument(skip(self, query, cache, statistics), fields(entity = %query.root_entity))]
    pub fn execute_with_cache(
        &self,
        query: &GraphQuery,
        cache: &PlanCache,
        statistics: Option<&TableStatistics>,
    ) -> Result<QueryResult, Error> {
        let start = std::time::Instant::now();
        let fingerprint = QueryFingerprint::from_query(query);
        let schema_version = self.catalog.current_version();

        // Try to get cached plan
        if let Some(mut plan) = cache.get(&fingerprint) {
            debug!(cache_hit = true, "using cached plan");
            // Optionally optimize with current statistics
            if statistics.is_some() {
                plan.optimize_include_order();
            }
            let result = self.execute_plan(&plan);
            let duration_us = start.elapsed().as_micros() as u64;
            if let Some(metrics) = &self.metrics {
                if result.is_ok() {
                    metrics.record_query(&query.root_entity, duration_us);
                }
            }
            return result;
        }

        debug!(cache_hit = false, "compiling new plan");

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
        let result = self.execute_plan(&plan);
        let duration_us = start.elapsed().as_micros() as u64;
        if let Some(metrics) = &self.metrics {
            if result.is_ok() {
                metrics.record_query(&query.root_entity, duration_us);
            }
        }
        result
    }

    /// Execute a query with statistics for cost-based join strategy selection.
    ///
    /// This uses the cost model to choose optimal join strategies based on
    /// actual table statistics rather than hardcoded estimates.
    #[instrument(skip(self, query, statistics), fields(entity = %query.root_entity))]
    pub fn execute_with_statistics(
        &self,
        query: &GraphQuery,
        statistics: &TableStatistics,
    ) -> Result<QueryResult, Error> {
        let start = std::time::Instant::now();
        let planner = QueryPlanner::new(self.catalog);
        let mut plan = planner.plan(query)?;

        // Optimize based on statistics
        plan.optimize_include_order();
        plan.deduplicate_includes();

        // Execute with cost model
        let result = self.execute_plan_with_cost_model(&plan, statistics);
        let duration_us = start.elapsed().as_micros() as u64;
        if let Some(metrics) = &self.metrics {
            if result.is_ok() {
                metrics.record_query(&query.root_entity, duration_us);
            }
        }
        result
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
            self.record_join_strategy(strategy);

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
    #[instrument(skip(self, plan), fields(entity = %plan.root_entity, includes = plan.includes.len()))]
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
    ///
    /// Automatically selects the optimal execution path:
    /// - Hash index for simple equality filters (O(1) lookup)
    /// - B-tree index for range filters (O(log N) lookup)
    /// - Columnar path for complex filtered queries (streaming filter with early termination)
    /// - Row-based path for unfiltered queries (simpler, fewer overhead)
    ///
    /// Both paths support early termination for LIMIT queries when no ORDER BY is specified.
    #[instrument(skip(self, plan), fields(entity = %plan.root_entity, has_filter = plan.filter.is_some()))]
    fn fetch_entities(&self, plan: &QueryPlan) -> Result<Vec<EntityRow>, Error> {
        // Try hash index for simple equality filters (O(1) lookup)
        if let Some(ormdb_proto::FilterExpr::Eq { field, value }) = &plan.filter {
            // Check if hash index exists for this column
            if self.storage.hash_index().has_index(&plan.root_entity, field)? {
                debug!(path = "hash-index", field = %field, "using hash index for equality filter");
                return self.fetch_entities_via_hash_index(plan, field, value);
            }
        }

        // Try B-tree index for range filters (O(log N) lookup)
        if let Some(btree) = self.storage.btree_index() {
            match &plan.filter {
                Some(ormdb_proto::FilterExpr::Gt { field, value }) => {
                    if btree.has_index(&plan.root_entity, field)? {
                        debug!(path = "btree-index", field = %field, op = "gt", "using B-tree index for range filter");
                        return self.fetch_entities_via_btree_gt(plan, field, value, false);
                    }
                }
                Some(ormdb_proto::FilterExpr::Ge { field, value }) => {
                    if btree.has_index(&plan.root_entity, field)? {
                        debug!(path = "btree-index", field = %field, op = "ge", "using B-tree index for range filter");
                        return self.fetch_entities_via_btree_gt(plan, field, value, true);
                    }
                }
                Some(ormdb_proto::FilterExpr::Lt { field, value }) => {
                    if btree.has_index(&plan.root_entity, field)? {
                        debug!(path = "btree-index", field = %field, op = "lt", "using B-tree index for range filter");
                        return self.fetch_entities_via_btree_lt(plan, field, value, false);
                    }
                }
                Some(ormdb_proto::FilterExpr::Le { field, value }) => {
                    if btree.has_index(&plan.root_entity, field)? {
                        debug!(path = "btree-index", field = %field, op = "le", "using B-tree index for range filter");
                        return self.fetch_entities_via_btree_lt(plan, field, value, true);
                    }
                }
                _ => {}
            }
        }

        // Use columnar path for filtered queries - more efficient due to:
        // 1. Two-phase streaming filter with early termination
        // 2. Only needed columns are read
        // 3. No per-row deserialization for non-matching entities
        if plan.filter.is_some() {
            debug!(path = "columnar", "using columnar path for filtered query");
            return self.fetch_entities_columnar(plan);
        }

        // Use row-based path for unfiltered queries (simpler, avoids columnar overhead)
        self.fetch_entities_row_based(plan)
    }

    /// Fetch entities using the columnar store with two-phase streaming filter.
    ///
    /// This is more efficient than row-based scanning because:
    /// 1. Only needed columns are read (not all fields)
    /// 2. No per-row deserialization overhead
    /// 3. Better cache locality for column scans
    /// 4. Streaming filter with early termination support
    ///
    /// Two-phase approach:
    /// - Phase 1: Scan filter column(s) → Apply predicate → Collect matching IDs (with LIMIT)
    /// - Phase 2: For matching IDs → Batch-fetch remaining columns → Build EntityRows
    fn fetch_entities_columnar(&self, plan: &QueryPlan) -> Result<Vec<EntityRow>, Error> {
        self.record_access_path(AccessPath::Columnar);
        let projection = self.storage.columnar().projection(&plan.root_entity)?;

        // Early termination parameters
        let can_early_terminate = plan.order_by.is_empty();
        let target_count = if can_early_terminate {
            plan.pagination
                .as_ref()
                .map(|p| (p.offset + p.limit) as usize)
        } else {
            None
        };

        // Determine which columns we need: filter fields + select fields
        let filter_fields: HashSet<String> = plan
            .filter
            .as_ref()
            .map(extract_filter_fields)
            .unwrap_or_default();

        let select_fields: HashSet<String> = if plan.fields.is_empty() {
            // Get all field names from the entity definition in the catalog
            plan.root_entity_def
                .fields
                .iter()
                .map(|f| f.name.clone())
                .collect()
        } else {
            plan.fields.iter().cloned().collect()
        };

        // PHASE 1: Collect matching entity IDs with streaming filter + early termination
        let (matching_ids, filter_values) = self.collect_matching_ids_columnar(
            &projection,
            &plan.filter,
            &filter_fields,
            target_count,
        )?;

        if matching_ids.is_empty() {
            debug!(rows_fetched = 0, path = "columnar-two-phase", "no matching entities");
            return Ok(vec![]);
        }

        // PHASE 2: Batch-fetch remaining columns for matching IDs
        let remaining_fields: Vec<&str> = select_fields
            .iter()
            .filter(|f| !filter_fields.contains(*f))
            .map(|s| s.as_str())
            .collect();

        let additional_data = if remaining_fields.is_empty() {
            // All needed fields were filter fields - already have them
            HashMap::new()
        } else {
            projection.fetch_columns_for_ids(&matching_ids, &remaining_fields)?
        };

        // Build EntityRow results by combining filter values with fetched columns
        let rows: Vec<EntityRow> = matching_ids
            .into_iter()
            .map(|id| {
                let mut fields: Vec<(String, Value)> = Vec::new();

                // Add filter field values (from phase 1)
                if let Some(filter_vals) = filter_values.get(&id) {
                    for (name, value) in filter_vals {
                        if select_fields.contains(name) {
                            fields.push((name.clone(), value.clone()));
                        }
                    }
                }

                // Add additional field values (from phase 2)
                if let Some(additional) = additional_data.get(&id) {
                    for (name, value) in additional {
                        fields.push((name.clone(), value.clone()));
                    }
                }

                EntityRow { id, fields }
            })
            .collect();

        debug!(rows_fetched = rows.len(), path = "columnar-two-phase", "entities fetched");
        Ok(rows)
    }

    /// Phase 1: Collect matching entity IDs with streaming filter + early termination.
    ///
    /// Returns (matching_ids, filter_field_values) where filter_field_values contains
    /// the values of filter fields for each matching entity (to avoid re-fetching).
    fn collect_matching_ids_columnar(
        &self,
        projection: &crate::storage::ColumnarProjection,
        filter: &Option<ormdb_proto::FilterExpr>,
        filter_fields: &HashSet<String>,
        target_count: Option<usize>,
    ) -> Result<(Vec<[u8; 16]>, HashMap<[u8; 16], HashMap<String, Value>>), Error> {
        // For simple equality filters on a single field, use optimized path
        if let Some(ormdb_proto::FilterExpr::Eq { field, value }) = filter {
            return self.collect_ids_eq_filter(projection, field, value, target_count);
        }

        // For complex filters, scan filter columns and evaluate
        let filter_field_names: Vec<&str> = filter_fields.iter().map(|s| s.as_str()).collect();

        let mut matching_ids = Vec::new();
        let mut filter_values: HashMap<[u8; 16], HashMap<String, Value>> = HashMap::new();

        // Scan filter columns as a streaming iterator (supports early termination)
        for item in projection.scan_columns_iter(&filter_field_names) {
            let (entity_id, fields_map) = item?;

            // Evaluate filter
            if let Some(f) = filter {
                let fields: Vec<(String, Value)> = fields_map.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !FilterEvaluator::evaluate(f, &fields)? {
                    continue;
                }
            }

            matching_ids.push(entity_id);
            filter_values.insert(entity_id, fields_map);

            // Early termination
            if let Some(target) = target_count {
                if matching_ids.len() >= target {
                    debug!(early_terminate = true, target = target, "stopping early - enough IDs collected");
                    break;
                }
            }
        }

        Ok((matching_ids, filter_values))
    }

    /// Optimized path for simple WHERE field = value queries.
    ///
    /// Uses `scan_column_eq()` for efficient single-column equality scan with early termination.
    fn collect_ids_eq_filter(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        value: &Value,
        target_count: Option<usize>,
    ) -> Result<(Vec<[u8; 16]>, HashMap<[u8; 16], HashMap<String, Value>>), Error> {
        let mut matching_ids = Vec::new();
        let mut filter_values: HashMap<[u8; 16], HashMap<String, Value>> = HashMap::new();

        // Use scan_column_eq for efficient equality scan
        for result in projection.scan_column_eq(field, value) {
            let entity_id = result?;
            matching_ids.push(entity_id);

            // Store the filter field value (we know it equals the target)
            let mut fields = HashMap::new();
            fields.insert(field.to_string(), value.clone());
            filter_values.insert(entity_id, fields);

            // Early termination
            if let Some(target) = target_count {
                if matching_ids.len() >= target {
                    debug!(early_terminate = true, target = target, "stopping early - enough IDs collected (eq filter)");
                    break;
                }
            }
        }

        Ok((matching_ids, filter_values))
    }

    /// Fetch entities using hash index for O(1) equality lookup.
    ///
    /// This is the fastest path for equality filters on indexed columns:
    /// 1. O(1) hash index lookup to get matching entity IDs
    /// 2. Batch fetch entities from row store (1 read per entity vs N reads for columnar)
    /// 3. Apply LIMIT/offset
    fn fetch_entities_via_hash_index(
        &self,
        plan: &QueryPlan,
        field: &str,
        value: &Value,
    ) -> Result<Vec<EntityRow>, Error> {
        self.record_access_path(AccessPath::HashIndex);
        // O(1) lookup - get all entity IDs that match this value
        let matching_ids = self.storage.hash_index().lookup(&plan.root_entity, field, value)?;

        if matching_ids.is_empty() {
            debug!(rows_fetched = 0, path = "hash-index", "no matching entities");
            return Ok(vec![]);
        }

        // Apply early LIMIT if no ORDER BY (we can truncate IDs early)
        let can_early_terminate = plan.order_by.is_empty();
        let ids_to_fetch: &[[u8; 16]] = if can_early_terminate {
            if let Some(ref pag) = plan.pagination {
                let target = (pag.offset + pag.limit) as usize;
                if matching_ids.len() > target {
                    &matching_ids[..target]
                } else {
                    &matching_ids[..]
                }
            } else {
                &matching_ids[..]
            }
        } else {
            &matching_ids[..]
        };

        // Fetch entities from row store (1 read per entity, much faster than columnar)
        // Row store keeps all fields together, so we avoid the N reads per entity overhead
        let mut rows = Vec::with_capacity(ids_to_fetch.len());

        for &id in ids_to_fetch {
            if let Some((_version, record)) = self.storage.get_latest(&id)? {
                let fields = decode_entity(&record.data)?;
                rows.push(EntityRow { id, fields });
            }
        }

        debug!(rows_fetched = rows.len(), path = "hash-index-row", "entities fetched via hash index + row store");
        Ok(rows)
    }

    /// Fetch entities using B-tree index for greater-than (or greater-equal) range filter.
    ///
    /// This is O(log N + K) where K is the number of matching entities:
    /// 1. B-tree range scan to get matching entity IDs
    /// 2. Batch fetch entities from row store
    fn fetch_entities_via_btree_gt(
        &self,
        plan: &QueryPlan,
        field: &str,
        value: &Value,
        include_equal: bool,
    ) -> Result<Vec<EntityRow>, Error> {
        self.record_access_path(AccessPath::BfTree);
        let btree = self.storage.btree_index().ok_or_else(|| {
            Error::InvalidData("B-tree index not available".to_string())
        })?;

        // O(log N + K) scan
        let matching_ids = if include_equal {
            btree.scan_greater_equal(&plan.root_entity, field, value)?
        } else {
            btree.scan_greater_than(&plan.root_entity, field, value)?
        };

        self.fetch_rows_for_ids(plan, &matching_ids)
    }

    /// Fetch entities using B-tree index for less-than (or less-equal) range filter.
    ///
    /// This is O(log N + K) where K is the number of matching entities:
    /// 1. B-tree range scan to get matching entity IDs
    /// 2. Batch fetch entities from row store
    fn fetch_entities_via_btree_lt(
        &self,
        plan: &QueryPlan,
        field: &str,
        value: &Value,
        include_equal: bool,
    ) -> Result<Vec<EntityRow>, Error> {
        self.record_access_path(AccessPath::BfTree);
        let btree = self.storage.btree_index().ok_or_else(|| {
            Error::InvalidData("B-tree index not available".to_string())
        })?;

        // O(log N + K) scan
        let matching_ids = if include_equal {
            btree.scan_less_equal(&plan.root_entity, field, value)?
        } else {
            btree.scan_less_than(&plan.root_entity, field, value)?
        };

        self.fetch_rows_for_ids(plan, &matching_ids)
    }

    /// Fetch rows from row store for a list of entity IDs.
    ///
    /// Shared helper for index-based lookups (hash and B-tree).
    fn fetch_rows_for_ids(
        &self,
        plan: &QueryPlan,
        matching_ids: &[[u8; 16]],
    ) -> Result<Vec<EntityRow>, Error> {
        if matching_ids.is_empty() {
            debug!(rows_fetched = 0, path = "index-row", "no matching entities");
            return Ok(vec![]);
        }

        // Apply early LIMIT if no ORDER BY (we can truncate IDs early)
        let can_early_terminate = plan.order_by.is_empty();
        let ids_to_fetch: &[[u8; 16]] = if can_early_terminate {
            if let Some(ref pag) = plan.pagination {
                let target = (pag.offset + pag.limit) as usize;
                if matching_ids.len() > target {
                    &matching_ids[..target]
                } else {
                    matching_ids
                }
            } else {
                matching_ids
            }
        } else {
            matching_ids
        };

        // Fetch entities from row store (1 read per entity)
        let mut rows = Vec::with_capacity(ids_to_fetch.len());

        for &id in ids_to_fetch {
            if let Some((_version, record)) = self.storage.get_latest(&id)? {
                let fields = decode_entity(&record.data)?;
                rows.push(EntityRow { id, fields });
            }
        }

        debug!(rows_fetched = rows.len(), path = "index-row", "entities fetched via index + row store");
        Ok(rows)
    }

    /// Fetch entities using the row store (fallback path).
    fn fetch_entities_row_based(&self, plan: &QueryPlan) -> Result<Vec<EntityRow>, Error> {
        self.record_access_path(AccessPath::Row);
        // Early termination optimization
        let can_early_terminate = plan.order_by.is_empty();
        let target_count = if can_early_terminate {
            plan.pagination
                .as_ref()
                .map(|p| (p.offset + p.limit) as usize)
        } else {
            None
        };

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

            // Early termination
            if let Some(target) = target_count {
                if rows.len() >= target {
                    debug!(early_terminate = true, target = target, "stopping early - enough rows collected");
                    break;
                }
            }
        }

        debug!(rows_fetched = rows.len(), path = "row-based", "entities fetched");
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
    #[instrument(skip(self, parent_ids, includes, budget), fields(parent_count = parent_ids.len(), include_count = includes.len()))]
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
    #[instrument(skip(self, source_ids, include), fields(path = %include.path, source_count = source_ids.len()))]
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
        self.record_join_strategy(strategy);
        debug!(strategy = ?strategy, "selected join strategy");

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

    fn record_access_path(&self, path: AccessPath) {
        if let Some(metrics) = &self.metrics {
            metrics.record_access_path(path);
        }
    }

    fn record_join_strategy(&self, strategy: JoinStrategy) {
        if let Some(metrics) = &self.metrics {
            let metric = match strategy {
                JoinStrategy::NestedLoop => JoinStrategyMetric::NestedLoop,
                JoinStrategy::HashJoin => JoinStrategyMetric::HashJoin,
            };
            metrics.record_join_strategy(metric);
        }
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
