//! API Key based authentication.
//!
//! Authenticates clients using API keys that map to specific capabilities.
//!
//! # Configuration
//!
//! Set `ORMDB_API_KEYS` environment variable with format:
//! ```text
//! key1:cap1,cap2;key2:cap3,cap4
//! ```
//!
//! # Example
//!
//! ```text
//! ORMDB_API_KEYS="prod-key:read:*,write:User;admin-key:admin"
//! ```

use std::collections::HashMap;
use std::sync::RwLock;

use ormdb_core::security::{CapabilityAuthenticator, CapabilitySet, SecurityError, SecurityResult};

/// API Key authenticator that validates keys against a configured store.
pub struct ApiKeyAuthenticator {
    /// Map of API key -> list of capability strings
    keys: RwLock<HashMap<String, Vec<String>>>,
}

impl ApiKeyAuthenticator {
    /// Create a new empty authenticator.
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
        }
    }

    /// Register an API key with its associated capabilities.
    pub fn register_key(&self, key: impl Into<String>, capabilities: Vec<String>) {
        let mut keys = self.keys.write().unwrap();
        keys.insert(key.into(), capabilities);
    }

    /// Load API keys from environment variable.
    ///
    /// Format: `key1:cap1,cap2;key2:cap3,cap4`
    ///
    /// # Example
    ///
    /// ```text
    /// ORMDB_API_KEYS="prod-read:read:*;prod-write:read:*,write:*;admin:admin"
    /// ```
    pub fn from_env(env_var: &str) -> Self {
        let auth = Self::new();

        if let Ok(keys_str) = std::env::var(env_var) {
            for key_spec in keys_str.split(';') {
                let key_spec = key_spec.trim();
                if key_spec.is_empty() {
                    continue;
                }

                // Split on first colon: key:capabilities
                if let Some(colon_pos) = key_spec.find(':') {
                    let key = key_spec[..colon_pos].trim().to_string();
                    let caps_str = &key_spec[colon_pos + 1..];

                    // Parse capabilities (comma-separated)
                    let capabilities: Vec<String> = caps_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !key.is_empty() && !capabilities.is_empty() {
                        auth.register_key(key, capabilities);
                    }
                }
            }
        }

        auth
    }

    /// Load from the default environment variable `ORMDB_API_KEYS`.
    pub fn from_default_env() -> Self {
        Self::from_env("ORMDB_API_KEYS")
    }

    /// Check if a key is registered.
    pub fn has_key(&self, key: &str) -> bool {
        let keys = self.keys.read().unwrap();
        keys.contains_key(key)
    }

    /// Get the number of registered keys.
    pub fn key_count(&self) -> usize {
        let keys = self.keys.read().unwrap();
        keys.len()
    }
}

impl Default for ApiKeyAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityAuthenticator for ApiKeyAuthenticator {
    fn authenticate(&self, requested: &[String]) -> SecurityResult<CapabilitySet> {
        // The first element of requested should be the API key
        if requested.is_empty() {
            // No credentials provided - return empty capabilities (anonymous)
            return Ok(CapabilitySet::new());
        }

        let api_key = &requested[0];
        let keys = self.keys.read().unwrap();

        if let Some(capability_strings) = keys.get(api_key) {
            // Key found - parse and return the capabilities
            let refs: Vec<&str> = capability_strings.iter().map(|s| s.as_str()).collect();
            CapabilitySet::from_strings(&refs)
        } else {
            // Key not found - authentication failed
            Err(SecurityError::AuthenticationFailed(
                "invalid API key".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_authenticate() {
        let auth = ApiKeyAuthenticator::new();
        auth.register_key("test-key", vec!["read:*".to_string(), "write:User".to_string()]);

        let result = auth.authenticate(&["test-key".to_string()]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_read("Post"));
        assert!(caps.has_write("User"));
        assert!(!caps.has_write("Post"));
    }

    #[test]
    fn test_invalid_key() {
        let auth = ApiKeyAuthenticator::new();
        auth.register_key("valid-key", vec!["read:*".to_string()]);

        let result = auth.authenticate(&["invalid-key".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_credentials() {
        let auth = ApiKeyAuthenticator::new();
        auth.register_key("test-key", vec!["read:*".to_string()]);

        let result = auth.authenticate(&[]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(!caps.has_read("User")); // No capabilities for anonymous
    }

    #[test]
    fn test_admin_key() {
        let auth = ApiKeyAuthenticator::new();
        auth.register_key("admin-key", vec!["admin".to_string()]);

        let result = auth.authenticate(&["admin-key".to_string()]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(caps.has_admin());
    }

    #[test]
    fn test_from_env_format() {
        // Simulate environment parsing
        let auth = ApiKeyAuthenticator::new();

        // Parse format: "key1:cap1,cap2;key2:cap3"
        let env_value = "prod-read:read:*;prod-write:read:*,write:*;admin:admin";

        for key_spec in env_value.split(';') {
            if let Some(colon_pos) = key_spec.find(':') {
                let key = key_spec[..colon_pos].trim().to_string();
                let caps_str = &key_spec[colon_pos + 1..];
                let capabilities: Vec<String> = caps_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                auth.register_key(key, capabilities);
            }
        }

        assert_eq!(auth.key_count(), 3);
        assert!(auth.has_key("prod-read"));
        assert!(auth.has_key("prod-write"));
        assert!(auth.has_key("admin"));
    }
}
