//! Bearer token based authentication.
//!
//! Authenticates clients using bearer tokens that map to specific capabilities.
//! Similar to API key authentication but typically used with short-lived tokens.
//!
//! # Configuration
//!
//! Set `ORMDB_TOKENS` environment variable with format:
//! ```text
//! token1:cap1,cap2;token2:cap3,cap4
//! ```
//!
//! # Example
//!
//! ```text
//! ORMDB_TOKENS="session-abc123:read:*,write:User;service-token:admin"
//! ```

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use ormdb_core::security::{CapabilityAuthenticator, CapabilitySet, SecurityError, SecurityResult};

/// Token entry with optional expiration.
#[derive(Clone)]
struct TokenEntry {
    capabilities: Vec<String>,
    expires_at: Option<Instant>,
}

impl TokenEntry {
    fn new(capabilities: Vec<String>) -> Self {
        Self {
            capabilities,
            expires_at: None,
        }
    }

    fn with_expiry(capabilities: Vec<String>, ttl: Duration) -> Self {
        Self {
            capabilities,
            expires_at: Some(Instant::now() + ttl),
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| Instant::now() > exp).unwrap_or(false)
    }
}

/// Token authenticator that validates bearer tokens against a configured store.
pub struct TokenAuthenticator {
    /// Map of token -> entry with capabilities and optional expiry
    tokens: RwLock<HashMap<String, TokenEntry>>,
}

impl TokenAuthenticator {
    /// Create a new empty authenticator.
    pub fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
        }
    }

    /// Register a token with its associated capabilities (no expiry).
    pub fn register_token(&self, token: impl Into<String>, capabilities: Vec<String>) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(token.into(), TokenEntry::new(capabilities));
    }

    /// Register a token with expiration.
    pub fn register_token_with_ttl(
        &self,
        token: impl Into<String>,
        capabilities: Vec<String>,
        ttl: Duration,
    ) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(token.into(), TokenEntry::with_expiry(capabilities, ttl));
    }

    /// Revoke a token.
    pub fn revoke_token(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(token).is_some()
    }

    /// Load tokens from environment variable.
    ///
    /// Format: `token1:cap1,cap2;token2:cap3,cap4`
    ///
    /// Note: Tokens loaded from env have no expiry.
    pub fn from_env(env_var: &str) -> Self {
        let auth = Self::new();

        if let Ok(tokens_str) = std::env::var(env_var) {
            for token_spec in tokens_str.split(';') {
                let token_spec = token_spec.trim();
                if token_spec.is_empty() {
                    continue;
                }

                if let Some(colon_pos) = token_spec.find(':') {
                    let token = token_spec[..colon_pos].trim().to_string();
                    let caps_str = &token_spec[colon_pos + 1..];

                    let capabilities: Vec<String> = caps_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !token.is_empty() && !capabilities.is_empty() {
                        auth.register_token(token, capabilities);
                    }
                }
            }
        }

        auth
    }

    /// Load from the default environment variable `ORMDB_TOKENS`.
    pub fn from_default_env() -> Self {
        Self::from_env("ORMDB_TOKENS")
    }

    /// Check if a token is valid (exists and not expired).
    pub fn is_valid(&self, token: &str) -> bool {
        let tokens = self.tokens.read().unwrap();
        tokens.get(token).map(|e| !e.is_expired()).unwrap_or(false)
    }

    /// Get the number of registered tokens.
    pub fn token_count(&self) -> usize {
        let tokens = self.tokens.read().unwrap();
        tokens.len()
    }

    /// Remove expired tokens.
    pub fn cleanup_expired(&self) -> usize {
        let mut tokens = self.tokens.write().unwrap();
        let before = tokens.len();
        tokens.retain(|_, entry| !entry.is_expired());
        before - tokens.len()
    }
}

impl Default for TokenAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityAuthenticator for TokenAuthenticator {
    fn authenticate(&self, requested: &[String]) -> SecurityResult<CapabilitySet> {
        if requested.is_empty() {
            // No credentials provided - return empty capabilities (anonymous)
            return Ok(CapabilitySet::new());
        }

        let token = &requested[0];

        // Strip "Bearer " prefix if present
        let token = token
            .strip_prefix("Bearer ")
            .or_else(|| token.strip_prefix("bearer "))
            .unwrap_or(token);

        let tokens = self.tokens.read().unwrap();

        if let Some(entry) = tokens.get(token) {
            if entry.is_expired() {
                return Err(SecurityError::AuthenticationFailed(
                    "token expired".to_string(),
                ));
            }

            let refs: Vec<&str> = entry.capabilities.iter().map(|s| s.as_str()).collect();
            CapabilitySet::from_strings(&refs)
        } else {
            Err(SecurityError::AuthenticationFailed(
                "invalid token".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_authenticate() {
        let auth = TokenAuthenticator::new();
        auth.register_token("test-token", vec!["read:*".to_string()]);

        let result = auth.authenticate(&["test-token".to_string()]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(caps.has_read("User"));
    }

    #[test]
    fn test_bearer_prefix() {
        let auth = TokenAuthenticator::new();
        auth.register_token("test-token", vec!["read:*".to_string()]);

        // Should work with Bearer prefix
        let result = auth.authenticate(&["Bearer test-token".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_token_expiry() {
        let auth = TokenAuthenticator::new();

        // Register with very short TTL
        auth.register_token_with_ttl(
            "short-lived",
            vec!["read:*".to_string()],
            Duration::from_millis(1),
        );

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(10));

        let result = auth.authenticate(&["short-lived".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_token() {
        let auth = TokenAuthenticator::new();
        auth.register_token("revocable", vec!["read:*".to_string()]);

        // Token should work initially
        assert!(auth.is_valid("revocable"));

        // Revoke it
        assert!(auth.revoke_token("revocable"));

        // Should no longer work
        assert!(!auth.is_valid("revocable"));
        let result = auth.authenticate(&["revocable".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let auth = TokenAuthenticator::new();

        auth.register_token_with_ttl("expired1", vec!["read:*".to_string()], Duration::from_millis(1));
        auth.register_token_with_ttl("expired2", vec!["read:*".to_string()], Duration::from_millis(1));
        auth.register_token("persistent", vec!["read:*".to_string()]);

        std::thread::sleep(Duration::from_millis(10));

        let cleaned = auth.cleanup_expired();
        assert_eq!(cleaned, 2);
        assert_eq!(auth.token_count(), 1);
        assert!(auth.is_valid("persistent"));
    }
}
