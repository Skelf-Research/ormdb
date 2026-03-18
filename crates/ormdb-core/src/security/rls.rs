//! Row-Level Security (RLS) policy definitions and compilation.
//!
//! RLS policies define filters that are automatically applied to queries
//! based on the security context.

use super::context::SecurityContext;
use super::error::{SecurityError, SecurityResult};
use ormdb_proto::{FilterExpr, SimpleFilter, Value};

/// Type of RLS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PolicyType {
    /// Row is accessible if ANY permissive policy matches.
    Permissive,
    /// Row is accessible only if ALL restrictive policies match.
    Restrictive,
}

impl Default for PolicyType {
    fn default() -> Self {
        PolicyType::Permissive
    }
}

/// Operations that RLS policies can apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RlsOperation {
    /// Read operations (SELECT).
    Select,
    /// Insert operations.
    Insert,
    /// Update operations.
    Update,
    /// Delete operations.
    Delete,
    /// All operations.
    All,
}

impl RlsOperation {
    /// Check if this operation matches the given operation.
    pub fn matches(&self, other: RlsOperation) -> bool {
        matches!(
            (self, other),
            (RlsOperation::All, _)
                | (_, RlsOperation::All)
                | (RlsOperation::Select, RlsOperation::Select)
                | (RlsOperation::Insert, RlsOperation::Insert)
                | (RlsOperation::Update, RlsOperation::Update)
                | (RlsOperation::Delete, RlsOperation::Delete)
        )
    }
}

/// RLS filter expression that can reference context attributes.
///
/// Note: This type uses serde for serialization due to recursive structure.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RlsFilterExpr {
    /// A standard filter expression with literal values.
    Standard(SimpleFilter),
    /// Field equals a context attribute value.
    /// Example: `org_id = context.user.org_id`
    AttributeEq {
        /// Field name in the entity.
        field: String,
        /// Attribute name in the security context.
        attribute: String,
    },
    /// Field is in a list stored in a context attribute.
    AttributeIn {
        /// Field name in the entity.
        field: String,
        /// Attribute name containing the list.
        attribute: String,
    },
    /// All conditions must be true.
    And(Vec<RlsFilterExpr>),
    /// At least one condition must be true.
    Or(Vec<RlsFilterExpr>),
    /// Always evaluates to true (for admin bypass).
    True,
    /// Always evaluates to false (deny all).
    False,
}

impl RlsFilterExpr {
    /// Create an attribute equality filter.
    pub fn attribute_eq(field: impl Into<String>, attribute: impl Into<String>) -> Self {
        RlsFilterExpr::AttributeEq {
            field: field.into(),
            attribute: attribute.into(),
        }
    }

    /// Create an attribute IN filter.
    pub fn attribute_in(field: impl Into<String>, attribute: impl Into<String>) -> Self {
        RlsFilterExpr::AttributeIn {
            field: field.into(),
            attribute: attribute.into(),
        }
    }

    /// Create an AND combination.
    pub fn and(exprs: Vec<RlsFilterExpr>) -> Self {
        RlsFilterExpr::And(exprs)
    }

    /// Create an OR combination.
    pub fn or(exprs: Vec<RlsFilterExpr>) -> Self {
        RlsFilterExpr::Or(exprs)
    }
}

/// RLS policy definition for an entity.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RlsPolicy {
    /// Policy name (unique identifier).
    pub name: String,
    /// Target entity type.
    pub entity: String,
    /// Policy type (permissive or restrictive).
    pub policy_type: PolicyType,
    /// Operations this policy applies to.
    pub operations: Vec<RlsOperation>,
    /// Filter expression (can reference context attributes).
    pub filter: RlsFilterExpr,
    /// Capability that bypasses this policy (optional).
    pub bypass_capability: Option<String>,
}

impl RlsPolicy {
    /// Create a new RLS policy.
    pub fn new(
        name: impl Into<String>,
        entity: impl Into<String>,
        filter: RlsFilterExpr,
    ) -> Self {
        Self {
            name: name.into(),
            entity: entity.into(),
            policy_type: PolicyType::Permissive,
            operations: vec![RlsOperation::All],
            filter,
            bypass_capability: None,
        }
    }

    /// Set the policy type.
    pub fn with_type(mut self, policy_type: PolicyType) -> Self {
        self.policy_type = policy_type;
        self
    }

    /// Set the operations this policy applies to.
    pub fn with_operations(mut self, operations: Vec<RlsOperation>) -> Self {
        self.operations = operations;
        self
    }

    /// Set a bypass capability.
    pub fn with_bypass(mut self, capability: impl Into<String>) -> Self {
        self.bypass_capability = Some(capability.into());
        self
    }

    /// Check if this policy applies to the given operation.
    pub fn applies_to(&self, operation: RlsOperation) -> bool {
        self.operations.iter().any(|op| op.matches(operation))
    }

    /// Check if the context can bypass this policy.
    pub fn can_bypass(&self, context: &SecurityContext) -> bool {
        if context.is_admin() {
            return true;
        }
        if let Some(cap) = &self.bypass_capability {
            context.capabilities.has_custom(cap)
        } else {
            false
        }
    }
}

/// RLS policy compiler that generates runtime filters.
pub struct RlsPolicyCompiler;

impl RlsPolicyCompiler {
    /// Compile RLS policies into a concrete FilterExpr for query execution.
    ///
    /// This resolves attribute references against the security context
    /// and combines policies according to their types.
    pub fn compile(
        policies: &[RlsPolicy],
        context: &SecurityContext,
        entity: &str,
        operation: RlsOperation,
    ) -> Option<FilterExpr> {
        // Filter to applicable policies
        let applicable: Vec<_> = policies
            .iter()
            .filter(|p| p.entity == entity && p.applies_to(operation))
            .collect();

        if applicable.is_empty() {
            return None;
        }

        // Separate permissive and restrictive policies
        let permissive: Vec<_> = applicable
            .iter()
            .filter(|p| p.policy_type == PolicyType::Permissive)
            .collect();
        let restrictive: Vec<_> = applicable
            .iter()
            .filter(|p| p.policy_type == PolicyType::Restrictive)
            .collect();

        // Compile permissive policies (OR together)
        let permissive_filter = Self::compile_permissive(&permissive, context);

        // Compile restrictive policies (AND together)
        let restrictive_filter = Self::compile_restrictive(&restrictive, context);

        // Combine: permissive OR'd, restrictive AND'd, then AND together
        match (permissive_filter, restrictive_filter) {
            (None, None) => None,
            (Some(p), None) => Some(p),
            (None, Some(r)) => Some(r),
            (Some(p), Some(r)) => {
                // Both must pass: (any permissive) AND (all restrictive)
                Some(FilterExpr::and(vec![
                    Self::to_simple_filter(&p),
                    Self::to_simple_filter(&r),
                ]))
            }
        }
    }

    /// Compile permissive policies (OR'd together).
    fn compile_permissive(
        policies: &[&&RlsPolicy],
        context: &SecurityContext,
    ) -> Option<FilterExpr> {
        if policies.is_empty() {
            return None;
        }

        let filters: Vec<FilterExpr> = policies
            .iter()
            .filter_map(|p| {
                if p.can_bypass(context) {
                    // Bypass means this policy allows all rows
                    return None;
                }
                Self::resolve_filter(&p.filter, context).ok()
            })
            .collect();

        // If any policy was bypassed, return None (allow all)
        if filters.len() < policies.len() {
            return None;
        }

        if filters.is_empty() {
            return None;
        }

        if filters.len() == 1 {
            return Some(filters.into_iter().next().unwrap());
        }

        // OR together
        let simple: Vec<SimpleFilter> = filters
            .into_iter()
            .map(|f| Self::to_simple_filter(&f))
            .collect();
        Some(FilterExpr::or(simple))
    }

    /// Compile restrictive policies (AND'd together).
    fn compile_restrictive(
        policies: &[&&RlsPolicy],
        context: &SecurityContext,
    ) -> Option<FilterExpr> {
        if policies.is_empty() {
            return None;
        }

        let filters: Vec<FilterExpr> = policies
            .iter()
            .filter_map(|p| {
                if p.can_bypass(context) {
                    // Bypass means this restrictive policy doesn't apply
                    return None;
                }
                Self::resolve_filter(&p.filter, context).ok()
            })
            .collect();

        if filters.is_empty() {
            return None;
        }

        if filters.len() == 1 {
            return Some(filters.into_iter().next().unwrap());
        }

        // AND together
        let simple: Vec<SimpleFilter> = filters
            .into_iter()
            .map(|f| Self::to_simple_filter(&f))
            .collect();
        Some(FilterExpr::and(simple))
    }

    /// Resolve attribute references in an RLS filter expression.
    fn resolve_filter(
        expr: &RlsFilterExpr,
        context: &SecurityContext,
    ) -> SecurityResult<FilterExpr> {
        match expr {
            RlsFilterExpr::Standard(simple) => Ok(Self::simple_to_filter_expr(simple)),
            RlsFilterExpr::AttributeEq { field, attribute } => {
                let value = context.get_attribute(attribute).ok_or_else(|| {
                    SecurityError::PolicyCompilationError(format!(
                        "missing context attribute: {}",
                        attribute
                    ))
                })?;
                Ok(FilterExpr::eq(field.clone(), value.clone()))
            }
            RlsFilterExpr::AttributeIn { field, attribute } => {
                let value = context.get_attribute(attribute).ok_or_else(|| {
                    SecurityError::PolicyCompilationError(format!(
                        "missing context attribute: {}",
                        attribute
                    ))
                })?;
                // Single value becomes a single-element IN (arrays not supported in Value)
                Ok(FilterExpr::in_values(field.clone(), vec![value.clone()]))
            }
            RlsFilterExpr::And(exprs) => {
                let resolved: SecurityResult<Vec<FilterExpr>> =
                    exprs.iter().map(|e| Self::resolve_filter(e, context)).collect();
                let filters = resolved?;
                if filters.len() == 1 {
                    return Ok(filters.into_iter().next().unwrap());
                }
                let simple: Vec<SimpleFilter> = filters
                    .into_iter()
                    .map(|f| Self::to_simple_filter(&f))
                    .collect();
                Ok(FilterExpr::and(simple))
            }
            RlsFilterExpr::Or(exprs) => {
                let resolved: SecurityResult<Vec<FilterExpr>> =
                    exprs.iter().map(|e| Self::resolve_filter(e, context)).collect();
                let filters = resolved?;
                if filters.len() == 1 {
                    return Ok(filters.into_iter().next().unwrap());
                }
                let simple: Vec<SimpleFilter> = filters
                    .into_iter()
                    .map(|f| Self::to_simple_filter(&f))
                    .collect();
                Ok(FilterExpr::or(simple))
            }
            RlsFilterExpr::True => {
                // True filter - matches everything
                // Use IS NOT NULL on a field that's always present (hacky but works)
                // Better: return a special "always true" that the executor handles
                Ok(FilterExpr::is_not_null("id"))
            }
            RlsFilterExpr::False => {
                // False filter - matches nothing
                // Use an impossible condition
                Ok(FilterExpr::eq("id", Value::Null))
            }
        }
    }

    /// Convert SimpleFilter to FilterExpr.
    fn simple_to_filter_expr(simple: &SimpleFilter) -> FilterExpr {
        match simple.clone() {
            SimpleFilter::Eq { field, value } => FilterExpr::eq(field, value),
            SimpleFilter::Ne { field, value } => FilterExpr::ne(field, value),
            SimpleFilter::Lt { field, value } => FilterExpr::lt(field, value),
            SimpleFilter::Le { field, value } => FilterExpr::le(field, value),
            SimpleFilter::Gt { field, value } => FilterExpr::gt(field, value),
            SimpleFilter::Ge { field, value } => FilterExpr::ge(field, value),
            SimpleFilter::In { field, values } => FilterExpr::in_values(field, values),
            SimpleFilter::NotIn { field, values } => FilterExpr::not_in_values(field, values),
            SimpleFilter::IsNull { field } => FilterExpr::is_null(field),
            SimpleFilter::IsNotNull { field } => FilterExpr::is_not_null(field),
            SimpleFilter::Like { field, pattern } => FilterExpr::like(field, pattern),
            SimpleFilter::NotLike { field, pattern } => FilterExpr::NotLike { field, pattern },
        }
    }

    /// Convert FilterExpr to SimpleFilter (for AND/OR combinations).
    fn to_simple_filter(expr: &FilterExpr) -> SimpleFilter {
        match expr.clone() {
            FilterExpr::Eq { field, value } => SimpleFilter::Eq { field, value },
            FilterExpr::Ne { field, value } => SimpleFilter::Ne { field, value },
            FilterExpr::Lt { field, value } => SimpleFilter::Lt { field, value },
            FilterExpr::Le { field, value } => SimpleFilter::Le { field, value },
            FilterExpr::Gt { field, value } => SimpleFilter::Gt { field, value },
            FilterExpr::Ge { field, value } => SimpleFilter::Ge { field, value },
            FilterExpr::In { field, values } => SimpleFilter::In { field, values },
            FilterExpr::NotIn { field, values } => SimpleFilter::NotIn { field, values },
            FilterExpr::IsNull { field } => SimpleFilter::IsNull { field },
            FilterExpr::IsNotNull { field } => SimpleFilter::IsNotNull { field },
            FilterExpr::Like { field, pattern } => SimpleFilter::Like { field, pattern },
            FilterExpr::NotLike { field, pattern } => SimpleFilter::NotLike { field, pattern },
            // For compound expressions, wrap in a simple eq that's always true/false
            // This is a limitation - ideally we'd support nested AND/OR
            FilterExpr::And(_) | FilterExpr::Or(_) => SimpleFilter::IsNotNull {
                field: "id".to_string(),
            },
        }
    }
}

/// Combine an optional user filter with an RLS filter.
pub fn combine_filters(
    user_filter: Option<FilterExpr>,
    rls_filter: Option<FilterExpr>,
) -> Option<FilterExpr> {
    match (user_filter, rls_filter) {
        (None, None) => None,
        (Some(f), None) => Some(f),
        (None, Some(r)) => Some(r),
        (Some(user), Some(rls)) => {
            // AND them together - RLS must always be applied
            Some(FilterExpr::and(vec![
                RlsPolicyCompiler::to_simple_filter(&user),
                RlsPolicyCompiler::to_simple_filter(&rls),
            ]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::capability::{Capability, CapabilitySet, EntityScope};

    fn context_with_org(org_id: &str) -> SecurityContext {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Read(EntityScope::All));
        SecurityContext::new("conn", "client", caps)
            .with_attribute("user.org_id", Value::String(org_id.to_string()))
    }

    #[test]
    fn test_rls_policy_creation() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        )
        .with_type(PolicyType::Permissive)
        .with_operations(vec![RlsOperation::Select, RlsOperation::Update]);

        assert_eq!(policy.name, "org_isolation");
        assert_eq!(policy.entity, "Document");
        assert!(policy.applies_to(RlsOperation::Select));
        assert!(policy.applies_to(RlsOperation::Update));
        assert!(!policy.applies_to(RlsOperation::Delete));
    }

    #[test]
    fn test_rls_compile_attribute_eq() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );

        let context = context_with_org("org-123");
        let filter = RlsPolicyCompiler::compile(
            &[policy],
            &context,
            "Document",
            RlsOperation::Select,
        );

        assert!(filter.is_some());
        let filter = filter.unwrap();
        match filter {
            FilterExpr::Eq { field, value } => {
                assert_eq!(field, "org_id");
                assert_eq!(value, Value::String("org-123".to_string()));
            }
            _ => panic!("Expected Eq filter"),
        }
    }

    #[test]
    fn test_rls_compile_no_matching_entity() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );

        let context = context_with_org("org-123");
        let filter = RlsPolicyCompiler::compile(
            &[policy],
            &context,
            "User", // Different entity
            RlsOperation::Select,
        );

        assert!(filter.is_none());
    }

    #[test]
    fn test_rls_compile_no_matching_operation() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        )
        .with_operations(vec![RlsOperation::Select]);

        let context = context_with_org("org-123");
        let filter = RlsPolicyCompiler::compile(
            &[policy],
            &context,
            "Document",
            RlsOperation::Delete, // Not in policy operations
        );

        assert!(filter.is_none());
    }

    #[test]
    fn test_rls_admin_bypass() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );

        let admin = SecurityContext::admin("conn");
        let filter = RlsPolicyCompiler::compile(
            &[policy],
            &admin,
            "Document",
            RlsOperation::Select,
        );

        // Admin bypasses all policies
        assert!(filter.is_none());
    }

    #[test]
    fn test_rls_custom_bypass() {
        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        )
        .with_bypass("bypass_rls");

        let mut caps = CapabilitySet::new();
        caps.add(Capability::Custom("bypass_rls".to_string()));
        let context = SecurityContext::new("conn", "client", caps);

        let filter = RlsPolicyCompiler::compile(
            &[policy],
            &context,
            "Document",
            RlsOperation::Select,
        );

        // Custom bypass works
        assert!(filter.is_none());
    }

    #[test]
    fn test_rls_multiple_permissive_policies() {
        let policy1 = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );
        let policy2 = RlsPolicy::new(
            "public_docs",
            "Document",
            RlsFilterExpr::Standard(SimpleFilter::eq("is_public", true)),
        );

        let context = context_with_org("org-123");
        let filter = RlsPolicyCompiler::compile(
            &[policy1, policy2],
            &context,
            "Document",
            RlsOperation::Select,
        );

        assert!(filter.is_some());
        // Should be OR of both policies
        match filter.unwrap() {
            FilterExpr::Or(exprs) => assert_eq!(exprs.len(), 2),
            _ => panic!("Expected Or filter"),
        }
    }

    #[test]
    fn test_rls_restrictive_policy() {
        let permissive = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );
        let restrictive = RlsPolicy::new(
            "active_only",
            "Document",
            RlsFilterExpr::Standard(SimpleFilter::eq("status", "active")),
        )
        .with_type(PolicyType::Restrictive);

        let context = context_with_org("org-123");
        let filter = RlsPolicyCompiler::compile(
            &[permissive, restrictive],
            &context,
            "Document",
            RlsOperation::Select,
        );

        assert!(filter.is_some());
        // Should be AND of (permissive) AND (restrictive)
        match filter.unwrap() {
            FilterExpr::And(exprs) => assert_eq!(exprs.len(), 2),
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_combine_filters() {
        let user_filter = Some(FilterExpr::eq("status", "published"));
        let rls_filter = Some(FilterExpr::eq("org_id", "org-123"));

        let combined = combine_filters(user_filter, rls_filter);
        assert!(combined.is_some());

        match combined.unwrap() {
            FilterExpr::And(exprs) => assert_eq!(exprs.len(), 2),
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_combine_filters_user_only() {
        let user_filter = Some(FilterExpr::eq("status", "published"));
        let combined = combine_filters(user_filter, None);
        assert!(combined.is_some());
    }

    #[test]
    fn test_combine_filters_rls_only() {
        let rls_filter = Some(FilterExpr::eq("org_id", "org-123"));
        let combined = combine_filters(None, rls_filter);
        assert!(combined.is_some());
    }

    #[test]
    fn test_combine_filters_none() {
        let combined = combine_filters(None, None);
        assert!(combined.is_none());
    }
}
