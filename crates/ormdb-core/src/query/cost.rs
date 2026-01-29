//! Cost model for query planning.
//!
//! This module estimates the cost of query execution plans, enabling
//! the optimizer to choose between different execution strategies.

use crate::catalog::{Cardinality, RelationDef};
use ormdb_proto::{FilterExpr, SimpleFilter};

use super::planner::{IncludePlan, QueryPlan};
use super::statistics::TableStatistics;

/// Cost estimate for a query plan or sub-plan.
#[derive(Debug, Clone, Default)]
pub struct CostEstimate {
    /// Estimated number of rows returned.
    pub estimated_rows: u64,
    /// Estimated I/O operations (scans, lookups).
    pub io_cost: u64,
    /// Estimated CPU cost (filter evaluations, comparisons).
    pub cpu_cost: u64,
    /// Total weighted cost (io_cost * IO_WEIGHT + cpu_cost * CPU_WEIGHT).
    pub total_cost: f64,
}

impl CostEstimate {
    /// Weight for I/O operations in total cost calculation.
    const IO_WEIGHT: f64 = 10.0;
    /// Weight for CPU operations in total cost calculation.
    const CPU_WEIGHT: f64 = 1.0;

    /// Create a new cost estimate.
    pub fn new(estimated_rows: u64, io_cost: u64, cpu_cost: u64) -> Self {
        let total_cost = (io_cost as f64 * Self::IO_WEIGHT) + (cpu_cost as f64 * Self::CPU_WEIGHT);
        Self {
            estimated_rows,
            io_cost,
            cpu_cost,
            total_cost,
        }
    }

    /// Create a zero-cost estimate.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Add another cost estimate to this one.
    pub fn add(&self, other: &CostEstimate) -> CostEstimate {
        CostEstimate::new(
            self.estimated_rows + other.estimated_rows,
            self.io_cost + other.io_cost,
            self.cpu_cost + other.cpu_cost,
        )
    }
}

/// Cost model for query planning decisions.
///
/// Uses statistics to estimate cardinalities and costs for different
/// query execution strategies.
pub struct CostModel<'a> {
    statistics: &'a TableStatistics,
}

impl<'a> CostModel<'a> {
    /// Create a new cost model with statistics reference.
    pub fn new(statistics: &'a TableStatistics) -> Self {
        Self { statistics }
    }

    /// Estimate the cost of executing a full query plan.
    pub fn estimate_plan(&self, plan: &QueryPlan) -> CostEstimate {
        let root_count = self.statistics.entity_count(&plan.root_entity);

        // Estimate rows after filter
        let selectivity = plan
            .filter
            .as_ref()
            .map(|f| self.estimate_selectivity(f))
            .unwrap_or(1.0);

        let estimated_root_rows = ((root_count as f64) * selectivity).ceil() as u64;

        // I/O: one full scan of root entity
        let io_cost = root_count;

        // CPU: filter evaluation for each row
        let cpu_cost = if plan.filter.is_some() {
            root_count
        } else {
            0
        };

        let mut total = CostEstimate::new(estimated_root_rows, io_cost, cpu_cost);

        // Add costs for all includes
        let mut parent_count = estimated_root_rows;
        for include in &plan.includes {
            let include_cost = self.estimate_include(include, parent_count);
            total = total.add(&include_cost);

            // Update parent count for nested includes
            if include.is_top_level() {
                parent_count = include_cost.estimated_rows;
            }
        }

        total
    }

    /// Estimate the cost of resolving an include.
    pub fn estimate_include(&self, include: &IncludePlan, parent_count: u64) -> CostEstimate {
        let target_entity = include.target_entity();
        let target_count = self.statistics.entity_count(target_entity);

        // Estimate fanout based on relation cardinality
        let fanout = self.relation_fanout(&include.relation, parent_count);

        // Apply filter selectivity if present
        let selectivity = include
            .filter
            .as_ref()
            .map(|f| self.estimate_selectivity(f))
            .unwrap_or(1.0);

        let estimated_rows = ((fanout as f64) * selectivity).ceil() as u64;

        // I/O: scan of target entity (hash join does one scan)
        let io_cost = target_count;

        // CPU: filter + join key comparison for each target row
        let cpu_cost = target_count + parent_count;

        CostEstimate::new(estimated_rows, io_cost, cpu_cost)
    }

    /// Estimate the selectivity of a filter expression.
    ///
    /// Returns a value between 0.0 and 1.0 representing the fraction
    /// of rows that are expected to pass the filter.
    pub fn estimate_selectivity(&self, filter: &FilterExpr) -> f64 {
        match filter {
            // Equality: very selective for unique fields
            FilterExpr::Eq { .. } => 0.1,
            FilterExpr::Ne { .. } => 0.9,

            // Range: moderately selective
            FilterExpr::Lt { .. } | FilterExpr::Le { .. } => 0.3,
            FilterExpr::Gt { .. } | FilterExpr::Ge { .. } => 0.3,

            // IN: depends on number of values
            FilterExpr::In { values, .. } => {
                // Each value has ~0.1 selectivity
                (values.len() as f64 * 0.1).min(1.0)
            }
            FilterExpr::NotIn { values, .. } => {
                // Inverse of IN
                1.0 - (values.len() as f64 * 0.1).min(0.9)
            }

            // Null checks: typically low selectivity
            FilterExpr::IsNull { .. } => 0.1,
            FilterExpr::IsNotNull { .. } => 0.9,

            // LIKE: depends on pattern
            FilterExpr::Like { pattern, .. } => {
                if pattern.starts_with('%') {
                    0.5 // No prefix, less selective
                } else {
                    0.1 // Has prefix, more selective
                }
            }
            FilterExpr::NotLike { pattern, .. } => {
                if pattern.starts_with('%') {
                    0.5
                } else {
                    0.9
                }
            }

            // Compound: combine selectivities
            FilterExpr::And(filters) => {
                filters
                    .iter()
                    .map(|f| self.estimate_simple_selectivity(f))
                    .product()
            }
            FilterExpr::Or(filters) => {
                // P(A or B) = P(A) + P(B) - P(A and B)
                // Simplified: sum of selectivities capped at 1.0
                filters
                    .iter()
                    .map(|f| self.estimate_simple_selectivity(f))
                    .sum::<f64>()
                    .min(1.0)
            }
        }
    }

    /// Estimate selectivity for SimpleFilter (used in And/Or).
    fn estimate_simple_selectivity(&self, filter: &SimpleFilter) -> f64 {
        match filter {
            SimpleFilter::Eq { .. } => 0.1,
            SimpleFilter::Ne { .. } => 0.9,
            SimpleFilter::Lt { .. } | SimpleFilter::Le { .. } => 0.3,
            SimpleFilter::Gt { .. } | SimpleFilter::Ge { .. } => 0.3,
            SimpleFilter::In { values, .. } => (values.len() as f64 * 0.1).min(1.0),
            SimpleFilter::NotIn { values, .. } => 1.0 - (values.len() as f64 * 0.1).min(0.9),
            SimpleFilter::IsNull { .. } => 0.1,
            SimpleFilter::IsNotNull { .. } => 0.9,
            SimpleFilter::Like { pattern, .. } => {
                if pattern.starts_with('%') { 0.5 } else { 0.1 }
            }
            SimpleFilter::NotLike { pattern, .. } => {
                if pattern.starts_with('%') { 0.5 } else { 0.9 }
            }
        }
    }

    /// Estimate the fanout (number of related rows) for a relation.
    pub fn relation_fanout(&self, relation: &RelationDef, parent_count: u64) -> u64 {
        let target_count = self.statistics.entity_count(&relation.to_entity);
        let source_count = self.statistics.entity_count(&relation.from_entity);

        match relation.cardinality {
            Cardinality::OneToOne => {
                // At most one per parent
                parent_count.min(target_count)
            }
            Cardinality::OneToMany => {
                if source_count > 0 && target_count > 0 {
                    // Average children per parent
                    let avg_per_parent = target_count / source_count;
                    parent_count * avg_per_parent.max(1)
                } else {
                    // Default: 10 per parent
                    parent_count * 10
                }
            }
            Cardinality::ManyToMany => {
                // Conservative estimate
                parent_count * 25
            }
        }
    }

    /// Compare two plans and return the cheaper one.
    pub fn cheaper_plan<'b>(
        &self,
        plan_a: &'b QueryPlan,
        plan_b: &'b QueryPlan,
    ) -> &'b QueryPlan {
        let cost_a = self.estimate_plan(plan_a);
        let cost_b = self.estimate_plan(plan_b);

        if cost_a.total_cost <= cost_b.total_cost {
            plan_a
        } else {
            plan_b
        }
    }

    /// Get the estimated child count for join strategy selection.
    pub fn estimated_child_count(&self, include: &IncludePlan) -> u64 {
        self.statistics.entity_count(include.target_entity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::EntityDef;
    use ormdb_proto::Value;

    fn create_test_statistics() -> TableStatistics {
        let stats = TableStatistics::new();
        stats.set_count("User", 1000);
        stats.set_count("Post", 5000);
        stats.set_count("Comment", 20000);
        stats
    }

    fn create_test_relation() -> RelationDef {
        RelationDef::one_to_many("posts", "User", "id", "Post", "author_id")
    }

    #[test]
    fn test_cost_estimate_new() {
        let cost = CostEstimate::new(100, 50, 200);
        assert_eq!(cost.estimated_rows, 100);
        assert_eq!(cost.io_cost, 50);
        assert_eq!(cost.cpu_cost, 200);
        // total = 50 * 10 + 200 * 1 = 700
        assert_eq!(cost.total_cost, 700.0);
    }

    #[test]
    fn test_cost_estimate_add() {
        let a = CostEstimate::new(100, 10, 20);
        let b = CostEstimate::new(50, 5, 10);
        let sum = a.add(&b);

        assert_eq!(sum.estimated_rows, 150);
        assert_eq!(sum.io_cost, 15);
        assert_eq!(sum.cpu_cost, 30);
    }

    #[test]
    fn test_selectivity_eq() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let filter = FilterExpr::eq("name", Value::String("Alice".to_string()));
        assert_eq!(model.estimate_selectivity(&filter), 0.1);
    }

    #[test]
    fn test_selectivity_range() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let filter = FilterExpr::gt("age", Value::Int32(25));
        assert_eq!(model.estimate_selectivity(&filter), 0.3);
    }

    #[test]
    fn test_selectivity_in() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let filter = FilterExpr::in_values(
            "status",
            vec![Value::String("active".to_string()), Value::String("pending".to_string())],
        );
        // 2 values * 0.1 = 0.2
        assert_eq!(model.estimate_selectivity(&filter), 0.2);
    }

    #[test]
    fn test_selectivity_like_with_prefix() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let filter = FilterExpr::like("name", "A%");
        assert_eq!(model.estimate_selectivity(&filter), 0.1);
    }

    #[test]
    fn test_selectivity_like_without_prefix() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let filter = FilterExpr::like("name", "%test%");
        assert_eq!(model.estimate_selectivity(&filter), 0.5);
    }

    #[test]
    fn test_selectivity_and() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        // AND combines with multiplication
        let filter = FilterExpr::and(vec![
            SimpleFilter::eq("status", Value::String("active".to_string())),
            SimpleFilter::eq("role", Value::String("admin".to_string())),
        ]);
        // 0.1 * 0.1 = 0.01
        assert_eq!(model.estimate_selectivity(&filter), 0.010000000000000002);
    }

    #[test]
    fn test_selectivity_or() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        // OR combines with addition (capped at 1.0)
        let filter = FilterExpr::or(vec![
            SimpleFilter::eq("status", Value::String("a".to_string())),
            SimpleFilter::eq("status", Value::String("b".to_string())),
        ]);
        // 0.1 + 0.1 = 0.2
        assert_eq!(model.estimate_selectivity(&filter), 0.2);
    }

    #[test]
    fn test_relation_fanout_one_to_many() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let relation = create_test_relation();

        // 5000 posts / 1000 users = 5 posts per user
        // 100 users * 5 = 500
        let fanout = model.relation_fanout(&relation, 100);
        assert_eq!(fanout, 500);
    }

    #[test]
    fn test_relation_fanout_one_to_one() {
        let stats = TableStatistics::new();
        stats.set_count("User", 1000);
        stats.set_count("Profile", 800); // Not all users have profiles
        let model = CostModel::new(&stats);

        let relation = RelationDef::one_to_one("profile", "User", "id", "Profile", "user_id");

        // One-to-one: min(parent_count, target_count)
        let fanout = model.relation_fanout(&relation, 100);
        assert_eq!(fanout, 100); // min(100, 800) = 100
    }

    #[test]
    fn test_estimate_include() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let include = IncludePlan {
            path: "posts".to_string(),
            relation: create_test_relation(),
            target_entity_def: EntityDef::new("Post", "id"),
            fields: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
        };

        let cost = model.estimate_include(&include, 100);

        // Expected: 500 rows (100 users * 5 avg posts)
        assert_eq!(cost.estimated_rows, 500);
        // I/O: scan all 5000 posts
        assert_eq!(cost.io_cost, 5000);
    }

    #[test]
    fn test_estimate_include_with_filter() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let include = IncludePlan {
            path: "posts".to_string(),
            relation: create_test_relation(),
            target_entity_def: EntityDef::new("Post", "id"),
            fields: vec![],
            filter: Some(FilterExpr::eq("status", Value::String("published".to_string()))),
            order_by: vec![],
            pagination: None,
        };

        let cost = model.estimate_include(&include, 100);

        // Expected: 500 * 0.1 = 50 rows after filter
        assert_eq!(cost.estimated_rows, 50);
    }

    #[test]
    fn test_estimated_child_count() {
        let stats = create_test_statistics();
        let model = CostModel::new(&stats);

        let include = IncludePlan {
            path: "posts".to_string(),
            relation: create_test_relation(),
            target_entity_def: EntityDef::new("Post", "id"),
            fields: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
        };

        assert_eq!(model.estimated_child_count(&include), 5000);
    }
}
