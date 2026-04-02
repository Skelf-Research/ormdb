//! Capability-based access control.
//!
//! Capabilities define what operations a client can perform.

use super::error::{SecurityError, SecurityResult};
use std::collections::HashSet;

/// Sensitivity level for field access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensitiveLevel {
    /// Internal fields (requires authenticated context).
    Internal,
    /// Sensitive fields (e.g., PII).
    Sensitive,
    /// Highly restricted fields.
    Restricted,
}

impl std::fmt::Display for SensitiveLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensitiveLevel::Internal => write!(f, "internal"),
            SensitiveLevel::Sensitive => write!(f, "sensitive"),
            SensitiveLevel::Restricted => write!(f, "restricted"),
        }
    }
}

/// Scope of entity access.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityScope {
    /// Access to all entities.
    All,
    /// Access to a specific entity type.
    Entity(String),
    /// Access to entities matching a pattern (e.g., "User*").
    Pattern(String),
}

impl EntityScope {
    /// Check if this scope matches the given entity name.
    pub fn matches(&self, entity: &str) -> bool {
        match self {
            EntityScope::All => true,
            EntityScope::Entity(name) => name == entity,
            EntityScope::Pattern(pattern) => {
                if pattern.ends_with('*') {
                    let prefix = &pattern[..pattern.len() - 1];
                    entity.starts_with(prefix)
                } else if pattern.starts_with('*') {
                    let suffix = &pattern[1..];
                    entity.ends_with(suffix)
                } else {
                    entity == pattern
                }
            }
        }
    }
}

/// Capability identifiers for access control.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read access to entities.
    Read(EntityScope),
    /// Write (insert/update) access to entities.
    Write(EntityScope),
    /// Delete access to entities.
    Delete(EntityScope),
    /// Administrative access (schema changes, migrations).
    Admin,
    /// Access to fields with specific sensitivity level.
    SensitiveFieldAccess(SensitiveLevel),
    /// Custom capability with arbitrary name.
    Custom(String),
}

impl Capability {
    /// Parse a capability from a string.
    ///
    /// Format: `operation:scope` where:
    /// - operation: `read`, `write`, `delete`, `admin`, `sensitive`, or custom
    /// - scope: `*` (all), entity name, or pattern
    ///
    /// Examples:
    /// - `read:*` - read all entities
    /// - `read:User` - read User entity
    /// - `write:Post*` - write entities starting with "Post"
    /// - `admin` - admin access
    /// - `sensitive:internal` - access to internal fields
    /// - `custom:audit` - custom audit capability
    pub fn parse(s: &str) -> SecurityResult<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();

        match parts[0] {
            "admin" => Ok(Capability::Admin),
            "read" => {
                let scope = Self::parse_scope(parts.get(1).copied())?;
                Ok(Capability::Read(scope))
            }
            "write" => {
                let scope = Self::parse_scope(parts.get(1).copied())?;
                Ok(Capability::Write(scope))
            }
            "delete" => {
                let scope = Self::parse_scope(parts.get(1).copied())?;
                Ok(Capability::Delete(scope))
            }
            "sensitive" => {
                let level = match parts.get(1).copied() {
                    Some("internal") | None => SensitiveLevel::Internal,
                    Some("sensitive") => SensitiveLevel::Sensitive,
                    Some("restricted") => SensitiveLevel::Restricted,
                    Some(other) => {
                        return Err(SecurityError::InvalidCapabilityFormat(format!(
                            "unknown sensitive level: {}",
                            other
                        )))
                    }
                };
                Ok(Capability::SensitiveFieldAccess(level))
            }
            "custom" => {
                let name = parts
                    .get(1)
                    .ok_or_else(|| {
                        SecurityError::InvalidCapabilityFormat(
                            "custom capability requires a name".to_string(),
                        )
                    })?
                    .to_string();
                Ok(Capability::Custom(name))
            }
            other => Err(SecurityError::InvalidCapabilityFormat(format!(
                "unknown capability type: {}",
                other
            ))),
        }
    }

    fn parse_scope(scope: Option<&str>) -> SecurityResult<EntityScope> {
        match scope {
            None | Some("*") => Ok(EntityScope::All),
            Some(s) if s.contains('*') => Ok(EntityScope::Pattern(s.to_string())),
            Some(s) => Ok(EntityScope::Entity(s.to_string())),
        }
    }

    /// Convert capability to string representation.
    pub fn to_string_repr(&self) -> String {
        match self {
            Capability::Admin => "admin".to_string(),
            Capability::Read(scope) => format!("read:{}", Self::scope_to_string(scope)),
            Capability::Write(scope) => format!("write:{}", Self::scope_to_string(scope)),
            Capability::Delete(scope) => format!("delete:{}", Self::scope_to_string(scope)),
            Capability::SensitiveFieldAccess(level) => format!("sensitive:{}", level),
            Capability::Custom(name) => format!("custom:{}", name),
        }
    }

    fn scope_to_string(scope: &EntityScope) -> String {
        match scope {
            EntityScope::All => "*".to_string(),
            EntityScope::Entity(name) => name.clone(),
            EntityScope::Pattern(pattern) => pattern.clone(),
        }
    }
}

/// A set of capabilities with efficient lookup.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create an empty capability set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a capability set from a list of capabilities.
    pub fn from_capabilities(caps: Vec<Capability>) -> Self {
        Self {
            capabilities: caps.into_iter().collect(),
        }
    }

    /// Parse capabilities from string representations.
    pub fn from_strings(strings: &[&str]) -> SecurityResult<Self> {
        let capabilities: SecurityResult<HashSet<Capability>> =
            strings.iter().map(|s| Capability::parse(s)).collect();
        Ok(Self {
            capabilities: capabilities?,
        })
    }

    /// Add a capability to the set.
    pub fn add(&mut self, cap: Capability) {
        self.capabilities.insert(cap);
    }

    /// Check if the set contains a specific capability.
    pub fn contains(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Check if read access is granted for the entity.
    pub fn has_read(&self, entity: &str) -> bool {
        self.capabilities.iter().any(|cap| match cap {
            Capability::Read(scope) => scope.matches(entity),
            Capability::Admin => true,
            _ => false,
        })
    }

    /// Check if write access is granted for the entity.
    pub fn has_write(&self, entity: &str) -> bool {
        self.capabilities.iter().any(|cap| match cap {
            Capability::Write(scope) => scope.matches(entity),
            Capability::Admin => true,
            _ => false,
        })
    }

    /// Check if delete access is granted for the entity.
    pub fn has_delete(&self, entity: &str) -> bool {
        self.capabilities.iter().any(|cap| match cap {
            Capability::Delete(scope) => scope.matches(entity),
            Capability::Admin => true,
            _ => false,
        })
    }

    /// Check if admin access is granted.
    pub fn has_admin(&self) -> bool {
        self.capabilities.contains(&Capability::Admin)
    }

    /// Check if access to sensitive fields at the given level is granted.
    pub fn has_sensitive_access(&self, level: SensitiveLevel) -> bool {
        if self.has_admin() {
            return true;
        }
        self.capabilities.iter().any(|cap| match cap {
            Capability::SensitiveFieldAccess(cap_level) => {
                // Higher levels grant access to lower levels
                match (cap_level, level) {
                    (SensitiveLevel::Restricted, _) => true,
                    (SensitiveLevel::Sensitive, SensitiveLevel::Sensitive) => true,
                    (SensitiveLevel::Sensitive, SensitiveLevel::Internal) => true,
                    (SensitiveLevel::Internal, SensitiveLevel::Internal) => true,
                    _ => false,
                }
            }
            _ => false,
        })
    }

    /// Check if a custom capability is granted.
    pub fn has_custom(&self, name: &str) -> bool {
        self.capabilities
            .iter()
            .any(|cap| matches!(cap, Capability::Custom(n) if n == name))
    }

    /// Get all capabilities as strings.
    pub fn to_strings(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .map(|c| c.to_string_repr())
            .collect()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Get the number of capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }
}

/// Trait for authenticating and granting capabilities.
pub trait CapabilityAuthenticator: Send + Sync {
    /// Authenticate the requested capabilities and return the granted set.
    ///
    /// The implementation should validate the client's identity and
    /// return only the capabilities they are authorized to have.
    fn authenticate(&self, requested: &[String]) -> SecurityResult<CapabilitySet>;
}

/// Default authenticator that grants all requested capabilities.
///
/// This is useful for testing or when security is disabled.
#[derive(Debug, Clone, Default)]
pub struct DefaultAuthenticator;

impl CapabilityAuthenticator for DefaultAuthenticator {
    fn authenticate(&self, requested: &[String]) -> SecurityResult<CapabilitySet> {
        let refs: Vec<&str> = requested.iter().map(|s| s.as_str()).collect();
        CapabilitySet::from_strings(&refs)
    }
}

/// Authenticator that grants no capabilities (anonymous access only).
#[derive(Debug, Clone, Default)]
pub struct DenyAllAuthenticator;

impl CapabilityAuthenticator for DenyAllAuthenticator {
    fn authenticate(&self, _requested: &[String]) -> SecurityResult<CapabilitySet> {
        Ok(CapabilitySet::new())
    }
}

/// Development/testing authenticator that grants full admin access.
///
/// **WARNING**: This authenticator grants full admin access regardless of
/// credentials. It should NEVER be used in production.
#[derive(Debug, Clone, Default)]
pub struct DevAuthenticator;

impl CapabilityAuthenticator for DevAuthenticator {
    fn authenticate(&self, _requested: &[String]) -> SecurityResult<CapabilitySet> {
        // Grant full admin access for development
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Admin);
        Ok(caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_scope_matches() {
        assert!(EntityScope::All.matches("User"));
        assert!(EntityScope::All.matches("Post"));

        assert!(EntityScope::Entity("User".to_string()).matches("User"));
        assert!(!EntityScope::Entity("User".to_string()).matches("Post"));

        assert!(EntityScope::Pattern("User*".to_string()).matches("User"));
        assert!(EntityScope::Pattern("User*".to_string()).matches("UserProfile"));
        assert!(!EntityScope::Pattern("User*".to_string()).matches("Post"));

        assert!(EntityScope::Pattern("*Post".to_string()).matches("Post"));
        assert!(EntityScope::Pattern("*Post".to_string()).matches("BlogPost"));
        assert!(!EntityScope::Pattern("*Post".to_string()).matches("User"));
    }

    #[test]
    fn test_capability_parse() {
        assert_eq!(Capability::parse("admin").unwrap(), Capability::Admin);

        assert_eq!(
            Capability::parse("read:*").unwrap(),
            Capability::Read(EntityScope::All)
        );

        assert_eq!(
            Capability::parse("read:User").unwrap(),
            Capability::Read(EntityScope::Entity("User".to_string()))
        );

        assert_eq!(
            Capability::parse("write:Post*").unwrap(),
            Capability::Write(EntityScope::Pattern("Post*".to_string()))
        );

        assert_eq!(
            Capability::parse("sensitive:internal").unwrap(),
            Capability::SensitiveFieldAccess(SensitiveLevel::Internal)
        );

        assert_eq!(
            Capability::parse("custom:audit").unwrap(),
            Capability::Custom("audit".to_string())
        );

        assert!(Capability::parse("unknown").is_err());
    }

    #[test]
    fn test_capability_set_read_access() {
        let caps = CapabilitySet::from_strings(&["read:User", "read:Post"]).unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_read("Post"));
        assert!(!caps.has_read("Admin"));
    }

    #[test]
    fn test_capability_set_wildcard() {
        let caps = CapabilitySet::from_strings(&["read:*"]).unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_read("Post"));
        assert!(caps.has_read("AnyEntity"));
    }

    #[test]
    fn test_capability_set_admin_grants_all() {
        let caps = CapabilitySet::from_strings(&["admin"]).unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_write("User"));
        assert!(caps.has_delete("User"));
        assert!(caps.has_admin());
    }

    #[test]
    fn test_capability_set_sensitive_levels() {
        let internal = CapabilitySet::from_strings(&["sensitive:internal"]).unwrap();
        assert!(internal.has_sensitive_access(SensitiveLevel::Internal));
        assert!(!internal.has_sensitive_access(SensitiveLevel::Sensitive));
        assert!(!internal.has_sensitive_access(SensitiveLevel::Restricted));

        let sensitive = CapabilitySet::from_strings(&["sensitive:sensitive"]).unwrap();
        assert!(sensitive.has_sensitive_access(SensitiveLevel::Internal));
        assert!(sensitive.has_sensitive_access(SensitiveLevel::Sensitive));
        assert!(!sensitive.has_sensitive_access(SensitiveLevel::Restricted));

        let restricted = CapabilitySet::from_strings(&["sensitive:restricted"]).unwrap();
        assert!(restricted.has_sensitive_access(SensitiveLevel::Internal));
        assert!(restricted.has_sensitive_access(SensitiveLevel::Sensitive));
        assert!(restricted.has_sensitive_access(SensitiveLevel::Restricted));
    }

    #[test]
    fn test_default_authenticator() {
        let auth = DefaultAuthenticator;
        let caps = auth
            .authenticate(&["read:User".to_string(), "write:Post".to_string()])
            .unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_write("Post"));
    }

    #[test]
    fn test_deny_all_authenticator() {
        let auth = DenyAllAuthenticator;
        let caps = auth
            .authenticate(&["read:User".to_string(), "admin".to_string()])
            .unwrap();
        assert!(!caps.has_read("User"));
        assert!(!caps.has_admin());
        assert!(caps.is_empty());
    }
}
