//! EXPLAIN command result types.
//!
//! These types are returned by the EXPLAIN operation to provide
//! query plan visualization and cost estimation.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Result of an EXPLAIN command.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct ExplainResult {
    /// The query plan summary.
    pub plan: QueryPlanSummary,
    /// Cost estimates.
    pub cost: CostSummary,
    /// Join strategies used for includes.
    pub joins: Vec<JoinInfo>,
    /// Whether the plan was found in cache.
    pub plan_cached: bool,
    /// Human-readable explanation text.
    pub explanation: String,
}

/// Summary of a query plan.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct QueryPlanSummary {
    /// Root entity being queried.
    pub root_entity: String,
    /// Fields being projected (empty means all fields).
    pub fields: Vec<String>,
    /// Filter description (if any).
    pub filter_description: Option<String>,
    /// Filter selectivity estimate (0.0-1.0).
    pub filter_selectivity: Option<f64>,
    /// Include plans for related entities.
    pub includes: Vec<IncludeSummary>,
    /// Fanout budget constraints.
    pub budget: BudgetSummary,
    /// Ordering specification.
    pub order_by: Vec<OrderSummary>,
    /// Pagination parameters.
    pub pagination: Option<PaginationSummary>,
}

/// Summary of an include plan.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct IncludeSummary {
    /// Path like "posts" or "posts.comments".
    pub path: String,
    /// Target entity type.
    pub target_entity: String,
    /// Relation type (OneToOne, OneToMany, ManyToMany).
    pub relation_type: String,
    /// Depth level (1 for top-level, 2 for nested, etc.).
    pub depth: u32,
    /// Estimated rows from this include.
    pub estimated_rows: u64,
    /// Filter on this include (if any).
    pub filter_description: Option<String>,
}

/// Cost summary for the query.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct CostSummary {
    /// Estimated total rows returned.
    pub estimated_rows: u64,
    /// Estimated I/O operations (scans, lookups).
    pub io_cost: u64,
    /// Estimated CPU cost (filter evaluations, comparisons).
    pub cpu_cost: u64,
    /// Total weighted cost (io_cost * 10 + cpu_cost).
    pub total_cost: f64,
}

/// Information about a join strategy.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct JoinInfo {
    /// Relation path being joined.
    pub path: String,
    /// Join strategy selected.
    pub strategy: JoinStrategyType,
    /// Reason for selecting this strategy.
    pub reason: String,
    /// Estimated parent count.
    pub parent_count: u64,
    /// Estimated child count.
    pub child_count: u64,
}

/// Join strategy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum JoinStrategyType {
    /// Nested loop join - for small datasets.
    NestedLoop,
    /// Hash join - for larger datasets.
    HashJoin,
}

impl std::fmt::Display for JoinStrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinStrategyType::NestedLoop => write!(f, "NestedLoop"),
            JoinStrategyType::HashJoin => write!(f, "HashJoin"),
        }
    }
}

/// Budget summary.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct BudgetSummary {
    /// Maximum entities allowed.
    pub max_entities: u64,
    /// Maximum edges allowed.
    pub max_edges: u64,
    /// Maximum depth allowed.
    pub max_depth: u32,
}

/// Order by specification summary.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct OrderSummary {
    /// Field being ordered by.
    pub field: String,
    /// Direction (ASC or DESC).
    pub direction: String,
}

/// Pagination summary.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct PaginationSummary {
    /// Maximum rows to return.
    pub limit: Option<u64>,
    /// Rows to skip.
    pub offset: Option<u64>,
}

impl ExplainResult {
    /// Create a new explain result.
    pub fn new(
        plan: QueryPlanSummary,
        cost: CostSummary,
        joins: Vec<JoinInfo>,
        plan_cached: bool,
        explanation: String,
    ) -> Self {
        Self {
            plan,
            cost,
            joins,
            plan_cached,
            explanation,
        }
    }
}

impl QueryPlanSummary {
    /// Create a new query plan summary.
    pub fn new(root_entity: impl Into<String>) -> Self {
        Self {
            root_entity: root_entity.into(),
            fields: Vec::new(),
            filter_description: None,
            filter_selectivity: None,
            includes: Vec::new(),
            budget: BudgetSummary::default(),
            order_by: Vec::new(),
            pagination: None,
        }
    }

    /// Set the fields to project.
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    /// Set the filter description.
    pub fn with_filter(mut self, description: String, selectivity: f64) -> Self {
        self.filter_description = Some(description);
        self.filter_selectivity = Some(selectivity);
        self
    }

    /// Add an include.
    pub fn with_include(mut self, include: IncludeSummary) -> Self {
        self.includes.push(include);
        self
    }

    /// Set the budget.
    pub fn with_budget(mut self, budget: BudgetSummary) -> Self {
        self.budget = budget;
        self
    }
}

impl Default for BudgetSummary {
    fn default() -> Self {
        Self {
            max_entities: 10_000,
            max_edges: 50_000,
            max_depth: 5,
        }
    }
}

impl CostSummary {
    /// Create a new cost summary.
    pub fn new(estimated_rows: u64, io_cost: u64, cpu_cost: u64) -> Self {
        let total_cost = (io_cost as f64 * 10.0) + (cpu_cost as f64);
        Self {
            estimated_rows,
            io_cost,
            cpu_cost,
            total_cost,
        }
    }

    /// Create zero cost (for when statistics are unavailable).
    pub fn zero() -> Self {
        Self {
            estimated_rows: 0,
            io_cost: 0,
            cpu_cost: 0,
            total_cost: 0.0,
        }
    }
}

impl IncludeSummary {
    /// Create a new include summary.
    pub fn new(
        path: impl Into<String>,
        target_entity: impl Into<String>,
        relation_type: impl Into<String>,
        depth: u32,
    ) -> Self {
        Self {
            path: path.into(),
            target_entity: target_entity.into(),
            relation_type: relation_type.into(),
            depth,
            estimated_rows: 0,
            filter_description: None,
        }
    }

    /// Set estimated rows.
    pub fn with_estimated_rows(mut self, rows: u64) -> Self {
        self.estimated_rows = rows;
        self
    }

    /// Set filter description.
    pub fn with_filter(mut self, description: String) -> Self {
        self.filter_description = Some(description);
        self
    }
}

impl JoinInfo {
    /// Create a new join info.
    pub fn new(
        path: impl Into<String>,
        strategy: JoinStrategyType,
        reason: impl Into<String>,
        parent_count: u64,
        child_count: u64,
    ) -> Self {
        Self {
            path: path.into(),
            strategy,
            reason: reason.into(),
            parent_count,
            child_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explain_result_serialization() {
        let result = ExplainResult::new(
            QueryPlanSummary::new("User")
                .with_fields(vec!["id".into(), "name".into()])
                .with_filter("status == 'active'".into(), 0.1),
            CostSummary::new(100, 1500, 200),
            vec![JoinInfo::new(
                "posts",
                JoinStrategyType::HashJoin,
                "Large dataset",
                100,
                5000,
            )],
            false,
            "Query Plan for User".into(),
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
        let archived =
            rkyv::access::<ArchivedExplainResult, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: ExplainResult =
            rkyv::deserialize::<ExplainResult, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_cost_summary() {
        let cost = CostSummary::new(100, 1500, 200);
        assert_eq!(cost.estimated_rows, 100);
        assert_eq!(cost.io_cost, 1500);
        assert_eq!(cost.cpu_cost, 200);
        // total = 1500 * 10 + 200 = 15200
        assert!((cost.total_cost - 15200.0).abs() < 0.01);
    }

    #[test]
    fn test_join_strategy_display() {
        assert_eq!(format!("{}", JoinStrategyType::NestedLoop), "NestedLoop");
        assert_eq!(format!("{}", JoinStrategyType::HashJoin), "HashJoin");
    }
}
