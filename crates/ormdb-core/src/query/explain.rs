//! Query explanation service.
//!
//! This module provides functionality to explain query plans without executing them,
//! showing cost estimates, join strategies, and execution details.

use crate::catalog::Catalog;
use crate::error::Error;

use super::cache::{PlanCache, QueryFingerprint};
use super::cost::CostModel;
use super::join::JoinStrategy;
use super::planner::{QueryPlan, QueryPlanner};
use super::statistics::TableStatistics;

use ormdb_proto::{
    BudgetSummary, CostSummary, ExplainResult, GraphQuery, IncludeSummary, JoinInfo,
    JoinStrategyType, OrderSummary, PaginationSummary, QueryPlanSummary,
};

/// Service for explaining query plans without executing them.
pub struct ExplainService<'a> {
    catalog: &'a Catalog,
    statistics: Option<&'a TableStatistics>,
    cache: Option<&'a PlanCache>,
}

impl<'a> ExplainService<'a> {
    /// Create a new explain service.
    pub fn new(catalog: &'a Catalog) -> Self {
        Self {
            catalog,
            statistics: None,
            cache: None,
        }
    }

    /// Add statistics for cost estimation.
    pub fn with_statistics(mut self, stats: &'a TableStatistics) -> Self {
        self.statistics = Some(stats);
        self
    }

    /// Add plan cache for cache status checking.
    pub fn with_cache(mut self, cache: &'a PlanCache) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Explain a query without executing it.
    pub fn explain(&self, query: &GraphQuery) -> Result<ExplainResult, Error> {
        // Plan the query
        let planner = QueryPlanner::new(self.catalog);
        let plan = planner.plan(query)?;

        // Check if plan is cached
        let plan_cached = self
            .cache
            .map(|c| c.get(&QueryFingerprint::from_query(query)).is_some())
            .unwrap_or(false);

        // Calculate costs
        let cost = self.calculate_cost(&plan);

        // Determine join strategies
        let joins = self.determine_joins(&plan);

        // Build summary
        let plan_summary = self.build_plan_summary(&plan);

        // Generate human-readable explanation
        let explanation = self.generate_explanation(&plan, &cost, &joins, plan_cached);

        Ok(ExplainResult::new(
            plan_summary,
            cost,
            joins,
            plan_cached,
            explanation,
        ))
    }

    /// Calculate cost estimates for the plan.
    fn calculate_cost(&self, plan: &QueryPlan) -> CostSummary {
        if let Some(stats) = self.statistics {
            let model = CostModel::new(stats);
            let estimate = model.estimate_plan(plan);
            CostSummary::new(estimate.estimated_rows, estimate.io_cost, estimate.cpu_cost)
        } else {
            // No statistics available, return zero estimates
            CostSummary::zero()
        }
    }

    /// Determine join strategies for each include.
    fn determine_joins(&self, plan: &QueryPlan) -> Vec<JoinInfo> {
        plan.includes
            .iter()
            .map(|include| {
                let target_entity = include.target_entity();

                // Estimate parent and child counts
                let parent_count = if let Some(stats) = self.statistics {
                    let root_count = stats.entity_count(&plan.root_entity);
                    // Apply filter selectivity estimate
                    let selectivity = plan.filter.as_ref().map(|_| 0.1).unwrap_or(1.0);
                    ((root_count as f64) * selectivity) as u64
                } else {
                    100 // Default assumption
                };

                let child_count = self
                    .statistics
                    .map(|s| s.entity_count(target_entity))
                    .unwrap_or(1000);

                // Select strategy
                let strategy = JoinStrategy::select(parent_count as usize, child_count);
                let (strategy_type, reason) = match strategy {
                    JoinStrategy::NestedLoop => (
                        JoinStrategyType::NestedLoop,
                        format!(
                            "Small dataset ({} parents, {} children)",
                            parent_count, child_count
                        ),
                    ),
                    JoinStrategy::HashJoin => (
                        JoinStrategyType::HashJoin,
                        format!(
                            "Large dataset ({} parents, {} children)",
                            parent_count, child_count
                        ),
                    ),
                };

                JoinInfo::new(
                    include.path.clone(),
                    strategy_type,
                    reason,
                    parent_count,
                    child_count,
                )
            })
            .collect()
    }

    /// Build the query plan summary.
    fn build_plan_summary(&self, plan: &QueryPlan) -> QueryPlanSummary {
        let filter_description = plan.filter.as_ref().map(|f| format!("{:?}", f));

        let filter_selectivity = if plan.filter.is_some() && self.statistics.is_some() {
            let stats = self.statistics.unwrap();
            let model = CostModel::new(stats);
            Some(model.estimate_selectivity(plan.filter.as_ref().unwrap()))
        } else {
            None
        };

        let includes: Vec<IncludeSummary> = plan
            .includes
            .iter()
            .map(|inc| {
                let estimated_rows = if let Some(stats) = self.statistics {
                    let target_count = stats.entity_count(inc.target_entity());
                    // Rough estimate based on cardinality
                    target_count.min(1000)
                } else {
                    0
                };

                let mut summary = IncludeSummary::new(
                    inc.path.clone(),
                    inc.target_entity(),
                    format!("{:?}", inc.relation.cardinality),
                    inc.depth() as u32,
                )
                .with_estimated_rows(estimated_rows);

                if let Some(filter) = &inc.filter {
                    summary = summary.with_filter(format!("{:?}", filter));
                }

                summary
            })
            .collect();

        let order_by = plan
            .order_by
            .iter()
            .map(|o| OrderSummary {
                field: o.field.clone(),
                direction: format!("{:?}", o.direction),
            })
            .collect();

        let pagination = plan.pagination.as_ref().map(|p| PaginationSummary {
            limit: if p.limit > 0 { Some(p.limit as u64) } else { None },
            offset: if p.offset > 0 { Some(p.offset as u64) } else { None },
        });

        let mut summary = QueryPlanSummary::new(&plan.root_entity)
            .with_fields(plan.fields.clone())
            .with_budget(BudgetSummary {
                max_entities: plan.budget.max_entities as u64,
                max_edges: plan.budget.max_edges as u64,
                max_depth: plan.budget.max_depth as u32,
            });

        if let Some(desc) = filter_description {
            summary = summary.with_filter(desc, filter_selectivity.unwrap_or(1.0));
        }

        for inc in includes {
            summary = summary.with_include(inc);
        }

        summary.order_by = order_by;
        summary.pagination = pagination;

        summary
    }

    /// Generate a human-readable explanation.
    fn generate_explanation(
        &self,
        plan: &QueryPlan,
        cost: &CostSummary,
        joins: &[JoinInfo],
        cached: bool,
    ) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Query Plan for {}", plan.root_entity));
        lines.push("=".repeat(40));

        // Plan status
        if cached {
            lines.push("Plan Status: CACHED".to_string());
        } else {
            lines.push("Plan Status: COMPILED".to_string());
        }

        lines.push(String::new());

        // Root entity
        lines.push(format!("Root Entity: {}", plan.root_entity));

        // Fields
        if plan.fields.is_empty() {
            lines.push("Fields: * (all)".to_string());
        } else {
            lines.push(format!("Fields: {}", plan.fields.join(", ")));
        }

        // Filter
        if let Some(filter) = &plan.filter {
            let selectivity = if let Some(stats) = self.statistics {
                let model = CostModel::new(stats);
                format!(" (selectivity: {:.2})", model.estimate_selectivity(filter))
            } else {
                String::new()
            };
            lines.push(format!("Filter: {:?}{}", filter, selectivity));
        }

        // Order by
        if !plan.order_by.is_empty() {
            let orders: Vec<String> = plan
                .order_by
                .iter()
                .map(|o| format!("{} {:?}", o.field, o.direction))
                .collect();
            lines.push(format!("Order By: {}", orders.join(", ")));
        }

        // Pagination
        if let Some(p) = &plan.pagination {
            let mut parts = Vec::new();
            if p.limit > 0 {
                parts.push(format!("LIMIT {}", p.limit));
            }
            if p.offset > 0 {
                parts.push(format!("OFFSET {}", p.offset));
            }
            if !parts.is_empty() {
                lines.push(format!("Pagination: {}", parts.join(", ")));
            }
        }

        // Includes
        if !plan.includes.is_empty() {
            lines.push(String::new());
            lines.push("Includes:".to_string());
            for (include, join) in plan.includes.iter().zip(joins.iter()) {
                lines.push(format!(
                    "  - {} -> {} (depth: {}, strategy: {})",
                    include.path,
                    include.target_entity(),
                    include.depth(),
                    join.strategy
                ));
                lines.push(format!("      {}", join.reason));
            }
        }

        // Cost estimates
        lines.push(String::new());
        lines.push("Cost Estimates:".to_string());
        lines.push(format!("  Estimated Rows: {}", cost.estimated_rows));
        lines.push(format!("  I/O Cost: {}", cost.io_cost));
        lines.push(format!("  CPU Cost: {}", cost.cpu_cost));
        lines.push(format!("  Total Cost: {:.2}", cost.total_cost));

        // Budget limits
        lines.push(String::new());
        lines.push("Budget Limits:".to_string());
        lines.push(format!("  Max Entities: {}", plan.budget.max_entities));
        lines.push(format!("  Max Edges: {}", plan.budget.max_edges));
        lines.push(format!("  Max Depth: {}", plan.budget.max_depth));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{
        EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle,
    };

    fn test_db() -> sled::Db {
        sled::Config::new().temporary(true).open().unwrap()
    }

    fn create_test_catalog() -> (sled::Db, Catalog) {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();

        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new(
                "status",
                FieldType::Scalar(ScalarType::String),
            ));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new(
                "author_id",
                FieldType::Scalar(ScalarType::Uuid),
            ));

        let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");

        let schema = SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post)
            .with_relation(user_posts);

        catalog.apply_schema(schema).unwrap();
        (db, catalog)
    }

    #[test]
    fn test_explain_simple_query() {
        let (_db, catalog) = create_test_catalog();
        let service = ExplainService::new(&catalog);

        let query = GraphQuery::new("User").with_fields(vec!["id".into(), "name".into()]);

        let result = service.explain(&query).unwrap();

        assert_eq!(result.plan.root_entity, "User");
        assert!(!result.plan_cached);
        assert!(result.explanation.contains("Query Plan for User"));
        assert!(result.explanation.contains("Fields: id, name"));
    }

    #[test]
    fn test_explain_with_include() {
        let (_db, catalog) = create_test_catalog();
        let stats = TableStatistics::new();
        stats.set_count("User", 1000);
        stats.set_count("Post", 5000);

        let service = ExplainService::new(&catalog).with_statistics(&stats);

        let query =
            GraphQuery::new("User").include(ormdb_proto::RelationInclude::new("posts"));

        let result = service.explain(&query).unwrap();

        assert_eq!(result.plan.includes.len(), 1);
        assert_eq!(result.plan.includes[0].path, "posts");
        assert_eq!(result.joins.len(), 1);
        assert!(result.explanation.contains("posts -> Post"));
    }

    #[test]
    fn test_explain_with_statistics() {
        let (_db, catalog) = create_test_catalog();
        let stats = TableStatistics::new();
        stats.set_count("User", 1000);

        let service = ExplainService::new(&catalog).with_statistics(&stats);

        let query = GraphQuery::new("User");
        let result = service.explain(&query).unwrap();

        // With statistics, cost should be non-zero
        assert!(result.cost.io_cost > 0 || result.cost.estimated_rows > 0);
    }

    #[test]
    fn test_explain_unknown_entity() {
        let (_db, catalog) = create_test_catalog();
        let service = ExplainService::new(&catalog);

        let query = GraphQuery::new("Unknown");
        let result = service.explain(&query);

        assert!(result.is_err());
    }

    #[test]
    fn test_join_strategy_selection() {
        let (_db, catalog) = create_test_catalog();
        let stats = TableStatistics::new();
        stats.set_count("User", 10); // Small
        stats.set_count("Post", 50);

        let service = ExplainService::new(&catalog).with_statistics(&stats);

        let query =
            GraphQuery::new("User").include(ormdb_proto::RelationInclude::new("posts"));

        let result = service.explain(&query).unwrap();

        // Small dataset should use NestedLoop
        assert_eq!(result.joins[0].strategy, JoinStrategyType::NestedLoop);
    }

    #[test]
    fn test_explanation_format() {
        let (_db, catalog) = create_test_catalog();
        let service = ExplainService::new(&catalog);

        let query = GraphQuery::new("User");
        let result = service.explain(&query).unwrap();

        // Check key sections are present
        assert!(result.explanation.contains("Query Plan for User"));
        assert!(result.explanation.contains("Root Entity: User"));
        assert!(result.explanation.contains("Cost Estimates:"));
        assert!(result.explanation.contains("Budget Limits:"));
    }
}
