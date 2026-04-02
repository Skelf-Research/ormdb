//! Authentication module for ORMDB server.
//!
//! Provides pluggable authenticators for verifying client credentials
//! and determining capabilities.
//!
//! # Supported Authentication Methods
//!
//! - **API Key**: Simple key-based authentication via `ORMDB_API_KEYS` env var
//! - **Token**: Bearer token authentication via `ORMDB_TOKENS` env var
//! - **JWT**: JSON Web Token authentication with signature verification
//!
//! # Environment Variable Formats
//!
//! ```text
//! ORMDB_API_KEYS="key1:read:*,write:User;key2:read:*;admin-key:admin"
//! ORMDB_TOKENS="token1:read:*;token2:admin"
//! ORMDB_JWT_SECRET="your-secret-key"
//! ```

mod apikey_authenticator;
mod jwt_authenticator;
mod token_authenticator;

pub use apikey_authenticator::ApiKeyAuthenticator;
pub use jwt_authenticator::JwtAuthenticator;
pub use token_authenticator::TokenAuthenticator;

// Re-export the trait from ormdb-core
pub use ormdb_core::security::CapabilityAuthenticator;
