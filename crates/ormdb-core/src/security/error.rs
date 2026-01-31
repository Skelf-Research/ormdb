//! Security-specific error types.

use thiserror::Error;

/// Security-related errors.
#[derive(Debug, Error)]
pub enum SecurityError {
    /// Permission denied for the requested operation.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Required capability was not granted.
    #[error("capability not granted: {0}")]
    CapabilityNotGranted(String),

    /// Row-level security policy violation.
    #[error("RLS policy violation: {0}")]
    RlsViolation(String),

    /// Field access denied due to sensitivity level.
    #[error("field access denied: {entity}.{field} requires {required}")]
    FieldAccessDenied {
        /// Entity containing the field.
        entity: String,
        /// Field name.
        field: String,
        /// Required sensitivity level or capability.
        required: String,
    },

    /// Query budget exceeded.
    #[error("query budget exceeded: {0}")]
    BudgetExceeded(String),

    /// Invalid security context.
    #[error("invalid security context: {0}")]
    InvalidContext(String),

    /// Policy compilation error.
    #[error("policy compilation error: {0}")]
    PolicyCompilationError(String),

    /// Audit logging error.
    #[error("audit error: {0}")]
    AuditError(String),

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Invalid capability string format.
    #[error("invalid capability format: {0}")]
    InvalidCapabilityFormat(String),

    /// Storage error during policy operations.
    #[error("storage error: {0}")]
    Storage(#[from] crate::error::Error),
}

/// Result type for security operations.
pub type SecurityResult<T> = Result<T, SecurityError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SecurityError::PermissionDenied("cannot read User".to_string());
        assert!(err.to_string().contains("cannot read User"));

        let err = SecurityError::FieldAccessDenied {
            entity: "User".to_string(),
            field: "ssn".to_string(),
            required: "SensitiveFieldAccess".to_string(),
        };
        assert!(err.to_string().contains("User.ssn"));
        assert!(err.to_string().contains("SensitiveFieldAccess"));
    }

    #[test]
    fn test_security_result() {
        let ok: SecurityResult<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: SecurityResult<i32> = Err(SecurityError::BudgetExceeded("too many rows".into()));
        assert!(err.is_err());
    }
}
