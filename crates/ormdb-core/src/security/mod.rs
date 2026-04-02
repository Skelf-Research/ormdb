//! Security module for ORMDB.
//!
//! This module provides comprehensive security features:
//! - Capability-based access control per connection
//! - Row-level security (RLS) policy compilation
//! - Field-level masking for sensitive data
//! - Query shape limits and budgets
//! - Structured audit logging
//!
//! # Security Model
//!
//! Security is enforced through a `SecurityContext` that flows through all operations.
//! The context contains:
//! - Connection identity
//! - Granted capabilities
//! - User attributes for RLS evaluation
//! - Query budgets
//!
//! # Example
//!
//! ```ignore
//! use ormdb_core::security::{SecurityContext, CapabilitySet, Capability, EntityScope};
//!
//! // Create a security context with read access to User entity
//! let capabilities = CapabilitySet::from_capabilities(vec![
//!     Capability::Read(EntityScope::Entity("User".to_string())),
//! ]);
//!
//! let context = SecurityContext::new("conn-123", "client-456", capabilities);
//!
//! // Check if operation is allowed
//! if context.can_read("User") {
//!     // Execute query...
//! }
//! ```

pub mod audit;
pub mod budget;
pub mod capability;
pub mod context;
pub mod error;
pub mod field_security;
pub mod policy;
pub mod rls;

// Re-export main types

// Error types
pub use error::{SecurityError, SecurityResult};

// Context types
pub use context::SecurityContext;

// Capability types
pub use capability::{
    Capability, CapabilityAuthenticator, CapabilitySet, DefaultAuthenticator, DevAuthenticator,
    EntityScope, SensitiveLevel,
};

// Field security types
pub use field_security::{FieldMasker, FieldResult, FieldSecurity, FieldSensitivity, MaskingStrategy};

// RLS types
pub use rls::{combine_filters, PolicyType, RlsFilterExpr, RlsOperation, RlsPolicy, RlsPolicyCompiler};

// Budget types
pub use budget::{CapabilityLevel, SecurityBudget};

// Audit types
pub use audit::{
    AuditError, AuditEvent, AuditEventType, AuditLogger, FileAuditLogger, MemoryAuditLogger,
    MutationOp, NullAuditLogger, StderrAuditLogger,
};

// Policy storage
pub use policy::PolicyStore;
