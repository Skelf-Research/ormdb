//! JWT (JSON Web Token) based authentication.
//!
//! Authenticates clients using JWTs that contain capability claims.
//!
//! # Configuration
//!
//! Set `ORMDB_JWT_SECRET` environment variable with the HMAC secret key,
//! or use `ORMDB_JWT_PUBLIC_KEY` for RSA/EC public key verification.
//!
//! # JWT Claims
//!
//! The JWT must contain:
//! - `sub`: Subject (user identifier)
//! - `capabilities`: Array of capability strings
//! - `exp`: Expiration timestamp (Unix timestamp)
//!
//! Optional claims:
//! - `iat`: Issued at timestamp
//! - `iss`: Issuer
//! - `aud`: Audience
//!
//! # Example JWT Payload
//!
//! ```json
//! {
//!   "sub": "user-123",
//!   "capabilities": ["read:*", "write:User"],
//!   "exp": 1735689600,
//!   "iat": 1735603200,
//!   "iss": "ormdb-auth"
//! }
//! ```

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use ormdb_core::security::{CapabilityAuthenticator, CapabilitySet, SecurityError, SecurityResult};

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user identifier).
    pub sub: String,

    /// List of capability strings.
    pub capabilities: Vec<String>,

    /// Expiration time (Unix timestamp).
    pub exp: u64,

    /// Issued at time (Unix timestamp).
    #[serde(default)]
    pub iat: Option<u64>,

    /// Issuer.
    #[serde(default)]
    pub iss: Option<String>,

    /// Audience.
    #[serde(default)]
    pub aud: Option<String>,

    /// Custom attributes that can be used for RLS.
    #[serde(default)]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// JWT authenticator configuration.
#[derive(Clone)]
pub struct JwtConfig {
    /// Algorithm to use for verification.
    pub algorithm: Algorithm,

    /// Whether to validate expiration.
    pub validate_exp: bool,

    /// Required issuer (if any).
    pub required_issuer: Option<String>,

    /// Required audience (if any).
    pub required_audience: Option<String>,

    /// Leeway in seconds for expiration check.
    pub leeway_secs: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::HS256,
            validate_exp: true,
            required_issuer: None,
            required_audience: None,
            leeway_secs: 60,
        }
    }
}

/// JWT authenticator that validates JWTs and extracts capabilities from claims.
pub struct JwtAuthenticator {
    decoding_key: DecodingKey,
    config: JwtConfig,
}

impl JwtAuthenticator {
    /// Create a new JWT authenticator with HMAC secret.
    pub fn with_secret(secret: &[u8]) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret),
            config: JwtConfig::default(),
        }
    }

    /// Create a new JWT authenticator with HMAC secret string.
    pub fn with_secret_str(secret: &str) -> Self {
        Self::with_secret(secret.as_bytes())
    }

    /// Create a new JWT authenticator with RSA public key (PEM format).
    pub fn with_rsa_pem(public_key_pem: &[u8]) -> Result<Self, String> {
        let key = DecodingKey::from_rsa_pem(public_key_pem)
            .map_err(|e| format!("invalid RSA public key: {}", e))?;

        Ok(Self {
            decoding_key: key,
            config: JwtConfig {
                algorithm: Algorithm::RS256,
                ..Default::default()
            },
        })
    }

    /// Create a new JWT authenticator with EC public key (PEM format).
    pub fn with_ec_pem(public_key_pem: &[u8]) -> Result<Self, String> {
        let key = DecodingKey::from_ec_pem(public_key_pem)
            .map_err(|e| format!("invalid EC public key: {}", e))?;

        Ok(Self {
            decoding_key: key,
            config: JwtConfig {
                algorithm: Algorithm::ES256,
                ..Default::default()
            },
        })
    }

    /// Load from environment variables.
    ///
    /// Checks in order:
    /// 1. `ORMDB_JWT_SECRET` - HMAC secret
    /// 2. `ORMDB_JWT_RSA_PUBLIC_KEY` - RSA public key (PEM)
    /// 3. `ORMDB_JWT_EC_PUBLIC_KEY` - EC public key (PEM)
    pub fn from_env() -> Result<Self, String> {
        if let Ok(secret) = std::env::var("ORMDB_JWT_SECRET") {
            return Ok(Self::with_secret_str(&secret));
        }

        if let Ok(rsa_key) = std::env::var("ORMDB_JWT_RSA_PUBLIC_KEY") {
            return Self::with_rsa_pem(rsa_key.as_bytes());
        }

        if let Ok(ec_key) = std::env::var("ORMDB_JWT_EC_PUBLIC_KEY") {
            return Self::with_ec_pem(ec_key.as_bytes());
        }

        Err("no JWT secret or public key configured (set ORMDB_JWT_SECRET)".to_string())
    }

    /// Set the required issuer for validation.
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.config.required_issuer = Some(issuer.into());
        self
    }

    /// Set the required audience for validation.
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.config.required_audience = Some(audience.into());
        self
    }

    /// Set the leeway for expiration check.
    pub fn with_leeway(mut self, secs: u64) -> Self {
        self.config.leeway_secs = secs;
        self
    }

    /// Disable expiration validation (not recommended for production).
    pub fn without_exp_validation(mut self) -> Self {
        self.config.validate_exp = false;
        self
    }

    /// Verify a JWT token and extract claims.
    pub fn verify_token(&self, token: &str) -> SecurityResult<JwtClaims> {
        let mut validation = Validation::new(self.config.algorithm);
        validation.leeway = self.config.leeway_secs;
        validation.validate_exp = self.config.validate_exp;

        if let Some(ref iss) = self.config.required_issuer {
            validation.set_issuer(&[iss]);
        }

        if let Some(ref aud) = self.config.required_audience {
            validation.set_audience(&[aud]);
        }

        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &validation).map_err(
            |e| {
                SecurityError::AuthenticationFailed(format!("JWT verification failed: {}", e))
            },
        )?;

        Ok(token_data.claims)
    }

    /// Get the subject (user ID) from a token.
    pub fn get_subject(&self, token: &str) -> SecurityResult<String> {
        let claims = self.verify_token(token)?;
        Ok(claims.sub)
    }
}

impl CapabilityAuthenticator for JwtAuthenticator {
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

        let claims = self.verify_token(token)?;

        let refs: Vec<&str> = claims.capabilities.iter().map(|s| s.as_str()).collect();
        CapabilitySet::from_strings(&refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn create_test_token(secret: &str, claims: &JwtClaims) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    fn test_claims() -> JwtClaims {
        JwtClaims {
            sub: "test-user".to_string(),
            capabilities: vec!["read:*".to_string(), "write:User".to_string()],
            exp: (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs())
                + 3600, // 1 hour from now
            iat: None,
            iss: None,
            aud: None,
            attributes: None,
        }
    }

    #[test]
    fn test_verify_valid_token() {
        let secret = "test-secret-key-for-testing";
        let auth = JwtAuthenticator::with_secret_str(secret);

        let claims = test_claims();
        let token = create_test_token(secret, &claims);

        let result = auth.verify_token(&token);
        assert!(result.is_ok());

        let verified_claims = result.unwrap();
        assert_eq!(verified_claims.sub, "test-user");
        assert_eq!(verified_claims.capabilities.len(), 2);
    }

    #[test]
    fn test_authenticate_returns_capabilities() {
        let secret = "test-secret-key-for-testing";
        let auth = JwtAuthenticator::with_secret_str(secret);

        let claims = test_claims();
        let token = create_test_token(secret, &claims);

        let result = auth.authenticate(&[token]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(caps.has_read("User"));
        assert!(caps.has_read("Post"));
        assert!(caps.has_write("User"));
        assert!(!caps.has_write("Post"));
    }

    #[test]
    fn test_bearer_prefix() {
        let secret = "test-secret-key-for-testing";
        let auth = JwtAuthenticator::with_secret_str(secret);

        let claims = test_claims();
        let token = create_test_token(secret, &claims);

        let result = auth.authenticate(&[format!("Bearer {}", token)]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_token() {
        let auth = JwtAuthenticator::with_secret_str("correct-secret");

        // Token signed with different secret
        let claims = test_claims();
        let bad_token = create_test_token("wrong-secret", &claims);

        let result = auth.authenticate(&[bad_token]);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token() {
        let secret = "test-secret";
        let auth = JwtAuthenticator::with_secret_str(secret);

        let mut claims = test_claims();
        claims.exp = 1; // Long expired

        let token = create_test_token(secret, &claims);

        let result = auth.authenticate(&[token]);
        assert!(result.is_err());
    }

    #[test]
    fn test_issuer_validation() {
        let secret = "test-secret";
        let auth = JwtAuthenticator::with_secret_str(secret).with_issuer("trusted-issuer");

        // Token with correct issuer
        let mut claims = test_claims();
        claims.iss = Some("trusted-issuer".to_string());
        let good_token = create_test_token(secret, &claims);

        assert!(auth.authenticate(&[good_token]).is_ok());

        // Token with wrong issuer
        claims.iss = Some("untrusted-issuer".to_string());
        let bad_token = create_test_token(secret, &claims);

        assert!(auth.authenticate(&[bad_token]).is_err());
    }

    #[test]
    fn test_admin_capability() {
        let secret = "test-secret";
        let auth = JwtAuthenticator::with_secret_str(secret);

        let mut claims = test_claims();
        claims.capabilities = vec!["admin".to_string()];
        let token = create_test_token(secret, &claims);

        let result = auth.authenticate(&[token]);
        assert!(result.is_ok());

        let caps = result.unwrap();
        assert!(caps.has_admin());
    }
}
