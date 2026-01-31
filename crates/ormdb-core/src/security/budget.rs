//! Query budget limits based on capability level.
//!
//! Security budgets control resource usage per connection/context.

use crate::query::FanoutBudget;

/// Capability level for budget determination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityLevel {
    /// Anonymous/unauthenticated access.
    Anonymous,
    /// Basic authenticated access.
    Authenticated,
    /// Elevated privileges.
    Privileged,
    /// Full administrative access.
    Admin,
}

impl Default for CapabilityLevel {
    fn default() -> Self {
        CapabilityLevel::Anonymous
    }
}

/// Security-aware query budget.
///
/// Extends `FanoutBudget` with additional limits and capability-based defaults.
#[derive(Debug, Clone)]
pub struct SecurityBudget {
    /// Maximum query depth (include nesting).
    pub max_depth: usize,
    /// Maximum entities per query.
    pub max_entities: usize,
    /// Maximum edges per query.
    pub max_edges: usize,
    /// Maximum query complexity score (optional).
    pub max_complexity: Option<u64>,
    /// Rate limit: queries per minute (optional).
    pub queries_per_minute: Option<u32>,
}

impl Default for SecurityBudget {
    fn default() -> Self {
        Self::for_level(CapabilityLevel::Authenticated)
    }
}

impl SecurityBudget {
    /// Create a budget for the given capability level.
    pub fn for_level(level: CapabilityLevel) -> Self {
        match level {
            CapabilityLevel::Anonymous => Self {
                max_depth: 2,
                max_entities: 100,
                max_edges: 500,
                max_complexity: Some(1000),
                queries_per_minute: Some(60),
            },
            CapabilityLevel::Authenticated => Self {
                max_depth: 5,
                max_entities: 10_000,
                max_edges: 50_000,
                max_complexity: Some(100_000),
                queries_per_minute: Some(1000),
            },
            CapabilityLevel::Privileged => Self {
                max_depth: 10,
                max_entities: 100_000,
                max_edges: 500_000,
                max_complexity: Some(1_000_000),
                queries_per_minute: Some(10_000),
            },
            CapabilityLevel::Admin => Self {
                max_depth: 20,
                max_entities: 1_000_000,
                max_edges: 5_000_000,
                max_complexity: None, // Unlimited
                queries_per_minute: None, // Unlimited
            },
        }
    }

    /// Create an unlimited budget (for internal/testing use).
    pub fn unlimited() -> Self {
        Self {
            max_depth: usize::MAX,
            max_entities: usize::MAX,
            max_edges: usize::MAX,
            max_complexity: None,
            queries_per_minute: None,
        }
    }

    /// Convert to a FanoutBudget for use with the query planner.
    pub fn to_fanout_budget(&self) -> FanoutBudget {
        FanoutBudget {
            max_entities: self.max_entities,
            max_edges: self.max_edges,
            max_depth: self.max_depth,
        }
    }

    /// Merge with another FanoutBudget, taking the minimum of each limit.
    ///
    /// This is useful when combining security limits with user-specified limits.
    pub fn merge_with_fanout(&self, other: &FanoutBudget) -> FanoutBudget {
        FanoutBudget {
            max_entities: self.max_entities.min(other.max_entities),
            max_edges: self.max_edges.min(other.max_edges),
            max_depth: self.max_depth.min(other.max_depth),
        }
    }

    /// Create a budget with custom limits.
    pub fn custom(
        max_depth: usize,
        max_entities: usize,
        max_edges: usize,
    ) -> Self {
        Self {
            max_depth,
            max_entities,
            max_edges,
            max_complexity: None,
            queries_per_minute: None,
        }
    }

    /// Set the maximum complexity score.
    pub fn with_max_complexity(mut self, max_complexity: u64) -> Self {
        self.max_complexity = Some(max_complexity);
        self
    }

    /// Set the rate limit.
    pub fn with_rate_limit(mut self, queries_per_minute: u32) -> Self {
        self.queries_per_minute = Some(queries_per_minute);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_for_level() {
        let anon = SecurityBudget::for_level(CapabilityLevel::Anonymous);
        assert_eq!(anon.max_depth, 2);
        assert_eq!(anon.max_entities, 100);

        let auth = SecurityBudget::for_level(CapabilityLevel::Authenticated);
        assert_eq!(auth.max_depth, 5);
        assert_eq!(auth.max_entities, 10_000);

        let admin = SecurityBudget::for_level(CapabilityLevel::Admin);
        assert!(admin.max_complexity.is_none()); // Unlimited
    }

    #[test]
    fn test_budget_to_fanout() {
        let budget = SecurityBudget::for_level(CapabilityLevel::Authenticated);
        let fanout = budget.to_fanout_budget();

        assert_eq!(fanout.max_depth, 5);
        assert_eq!(fanout.max_entities, 10_000);
        assert_eq!(fanout.max_edges, 50_000);
    }

    #[test]
    fn test_budget_merge() {
        let security = SecurityBudget::for_level(CapabilityLevel::Authenticated);
        let user = FanoutBudget {
            max_depth: 3,
            max_entities: 1_000,
            max_edges: 100_000, // Higher than security limit
        };

        let merged = security.merge_with_fanout(&user);

        // Takes minimum of each
        assert_eq!(merged.max_depth, 3);
        assert_eq!(merged.max_entities, 1_000);
        assert_eq!(merged.max_edges, 50_000); // Security limit is lower
    }

    #[test]
    fn test_unlimited_budget() {
        let budget = SecurityBudget::unlimited();
        assert_eq!(budget.max_depth, usize::MAX);
        assert!(budget.max_complexity.is_none());
        assert!(budget.queries_per_minute.is_none());
    }

    #[test]
    fn test_custom_budget() {
        let budget = SecurityBudget::custom(3, 500, 2000)
            .with_max_complexity(5000)
            .with_rate_limit(100);

        assert_eq!(budget.max_depth, 3);
        assert_eq!(budget.max_entities, 500);
        assert_eq!(budget.max_edges, 2000);
        assert_eq!(budget.max_complexity, Some(5000));
        assert_eq!(budget.queries_per_minute, Some(100));
    }
}
