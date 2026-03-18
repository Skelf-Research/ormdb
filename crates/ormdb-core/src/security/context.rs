//! Security context that flows through all operations.
//!
//! The security context carries identity, capabilities, and attributes
//! needed for access control decisions.

use super::budget::{CapabilityLevel, SecurityBudget};
use super::capability::{CapabilityAuthenticator, CapabilitySet, SensitiveLevel};
use super::error::{SecurityError, SecurityResult};
use crate::storage::key::current_timestamp;
use ormdb_proto::Value;
use std::collections::HashMap;

/// Connection-scoped security context.
///
/// This struct flows through all operations and provides the information
/// needed for:
/// - Capability-based access control
/// - Row-level security (RLS) evaluation
/// - Field-level masking decisions
/// - Query budget enforcement
/// - Audit logging
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Unique connection/session identifier.
    pub connection_id: String,
    /// Client identifier from handshake.
    pub client_id: String,
    /// Granted capabilities for this connection.
    pub capabilities: CapabilitySet,
    /// User attributes for RLS evaluation (e.g., user_id, org_id, role).
    pub attributes: HashMap<String, Value>,
    /// Query budget limits for this context.
    pub budget: SecurityBudget,
    /// Timestamp when context was created.
    pub created_at: u64,
}

impl SecurityContext {
    /// Create a new security context.
    pub fn new(connection_id: impl Into<String>, client_id: impl Into<String>, capabilities: CapabilitySet) -> Self {
        let level = if capabilities.has_admin() {
            CapabilityLevel::Admin
        } else if capabilities.is_empty() {
            CapabilityLevel::Anonymous
        } else {
            CapabilityLevel::Authenticated
        };

        Self {
            connection_id: connection_id.into(),
            client_id: client_id.into(),
            capabilities,
            attributes: HashMap::new(),
            budget: SecurityBudget::for_level(level),
            created_at: current_timestamp(),
        }
    }

    /// Create an anonymous context with minimal permissions.
    pub fn anonymous() -> Self {
        Self {
            connection_id: "anonymous".to_string(),
            client_id: "anonymous".to_string(),
            capabilities: CapabilitySet::new(),
            attributes: HashMap::new(),
            budget: SecurityBudget::for_level(CapabilityLevel::Anonymous),
            created_at: current_timestamp(),
        }
    }

    /// Create a context with full admin access.
    pub fn admin(connection_id: impl Into<String>) -> Self {
        let mut caps = CapabilitySet::new();
        caps.add(super::capability::Capability::Admin);

        Self {
            connection_id: connection_id.into(),
            client_id: "admin".to_string(),
            capabilities: caps,
            attributes: HashMap::new(),
            budget: SecurityBudget::for_level(CapabilityLevel::Admin),
            created_at: current_timestamp(),
        }
    }

    /// Create a context from handshake capabilities.
    pub fn from_handshake(
        connection_id: impl Into<String>,
        client_id: impl Into<String>,
        requested_capabilities: &[String],
        authenticator: &dyn CapabilityAuthenticator,
    ) -> SecurityResult<Self> {
        let capabilities = authenticator.authenticate(requested_capabilities)?;
        Ok(Self::new(connection_id, client_id, capabilities))
    }

    /// Set a user attribute for RLS evaluation.
    pub fn with_attribute(mut self, name: impl Into<String>, value: Value) -> Self {
        self.attributes.insert(name.into(), value);
        self
    }

    /// Set multiple attributes.
    pub fn with_attributes(mut self, attributes: HashMap<String, Value>) -> Self {
        self.attributes.extend(attributes);
        self
    }

    /// Set a custom budget.
    pub fn with_budget(mut self, budget: SecurityBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Get an attribute value for RLS evaluation.
    pub fn get_attribute(&self, name: &str) -> Option<&Value> {
        self.attributes.get(name)
    }

    /// Get an attribute as a string value.
    pub fn get_attribute_string(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).and_then(|v| match v {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// Check if the context can read the given entity type.
    pub fn can_read(&self, entity: &str) -> bool {
        self.capabilities.has_read(entity)
    }

    /// Check if the context can write to the given entity type.
    pub fn can_write(&self, entity: &str) -> bool {
        self.capabilities.has_write(entity)
    }

    /// Check if the context can delete from the given entity type.
    pub fn can_delete(&self, entity: &str) -> bool {
        self.capabilities.has_delete(entity)
    }

    /// Check if the context has admin access.
    pub fn is_admin(&self) -> bool {
        self.capabilities.has_admin()
    }

    /// Check if the context can access fields at the given sensitivity level.
    pub fn can_access_sensitive(&self, level: SensitiveLevel) -> bool {
        self.capabilities.has_sensitive_access(level)
    }

    /// Check if the context is authenticated (not anonymous).
    pub fn is_authenticated(&self) -> bool {
        !self.capabilities.is_empty()
    }

    /// Require read permission or return an error.
    pub fn require_read(&self, entity: &str) -> SecurityResult<()> {
        if self.can_read(entity) {
            Ok(())
        } else {
            Err(SecurityError::PermissionDenied(format!(
                "read access to '{}' is required",
                entity
            )))
        }
    }

    /// Require write permission or return an error.
    pub fn require_write(&self, entity: &str) -> SecurityResult<()> {
        if self.can_write(entity) {
            Ok(())
        } else {
            Err(SecurityError::PermissionDenied(format!(
                "write access to '{}' is required",
                entity
            )))
        }
    }

    /// Require delete permission or return an error.
    pub fn require_delete(&self, entity: &str) -> SecurityResult<()> {
        if self.can_delete(entity) {
            Ok(())
        } else {
            Err(SecurityError::PermissionDenied(format!(
                "delete access to '{}' is required",
                entity
            )))
        }
    }

    /// Require admin permission or return an error.
    pub fn require_admin(&self) -> SecurityResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(SecurityError::PermissionDenied(
                "admin access is required".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_context() {
        let ctx = SecurityContext::anonymous();
        assert!(!ctx.can_read("User"));
        assert!(!ctx.can_write("User"));
        assert!(!ctx.is_admin());
        assert!(!ctx.is_authenticated());
        assert_eq!(ctx.budget.max_depth, 2); // Anonymous limit
    }

    #[test]
    fn test_admin_context() {
        let ctx = SecurityContext::admin("conn-123");
        assert!(ctx.can_read("User"));
        assert!(ctx.can_write("User"));
        assert!(ctx.can_delete("User"));
        assert!(ctx.is_admin());
        assert!(ctx.is_authenticated());
    }

    #[test]
    fn test_context_with_attributes() {
        let ctx = SecurityContext::anonymous()
            .with_attribute("user_id", Value::String("user-123".into()))
            .with_attribute("org_id", Value::String("org-456".into()));

        assert_eq!(
            ctx.get_attribute_string("user_id"),
            Some("user-123")
        );
        assert_eq!(
            ctx.get_attribute_string("org_id"),
            Some("org-456")
        );
        assert_eq!(ctx.get_attribute("missing"), None);
    }

    #[test]
    fn test_context_from_handshake() {
        use super::super::capability::DefaultAuthenticator;

        let auth = DefaultAuthenticator;
        let ctx = SecurityContext::from_handshake(
            "conn-123",
            "client-456",
            &["read:User".to_string(), "write:Post".to_string()],
            &auth,
        )
        .unwrap();

        assert!(ctx.can_read("User"));
        assert!(ctx.can_write("Post"));
        assert!(!ctx.can_write("User"));
        assert!(!ctx.is_admin());
    }

    #[test]
    fn test_require_permissions() {
        let ctx = SecurityContext::from_handshake(
            "conn",
            "client",
            &["read:User".to_string()],
            &super::super::capability::DefaultAuthenticator,
        )
        .unwrap();

        assert!(ctx.require_read("User").is_ok());
        assert!(ctx.require_read("Post").is_err());
        assert!(ctx.require_write("User").is_err());
        assert!(ctx.require_admin().is_err());
    }

    #[test]
    fn test_custom_budget() {
        let budget = SecurityBudget::custom(3, 500, 2000);
        let ctx = SecurityContext::anonymous().with_budget(budget);

        assert_eq!(ctx.budget.max_depth, 3);
        assert_eq!(ctx.budget.max_entities, 500);
    }
}
