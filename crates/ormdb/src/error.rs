//! Error types for the ormdb embedded library.

use thiserror::Error;

/// The main error type for ormdb operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Database I/O error.
    #[error("database error: {0}")]
    Database(String),

    /// Storage layer error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Schema-related error.
    #[error("schema error: {0}")]
    Schema(String),

    /// Query execution error.
    #[error("query error: {0}")]
    Query(String),

    /// Entity not found.
    #[error("entity not found: {0}")]
    NotFound(String),

    /// Invalid argument provided.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Transaction conflict (optimistic concurrency).
    #[error("transaction conflict: entity was modified by another transaction")]
    TransactionConflict {
        /// The entity ID that had a conflict.
        entity_id: [u8; 16],
        /// Expected version.
        expected_version: u64,
        /// Actual version found.
        actual_version: u64,
    },

    /// Constraint violation.
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    /// Unique constraint violation.
    #[error("unique constraint violated: duplicate value '{value}' for {entity}.{field}")]
    UniqueViolation {
        /// Entity name.
        entity: String,
        /// Field name.
        field: String,
        /// Duplicate value.
        value: String,
    },

    /// Foreign key constraint violation.
    #[error("foreign key violation: {entity}.{field} references non-existent {referenced_entity}")]
    ForeignKeyViolation {
        /// Entity name.
        entity: String,
        /// Field name.
        field: String,
        /// Referenced entity name.
        referenced_entity: String,
    },

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Internal error (should not normally occur).
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type alias for ormdb operations.
pub type Result<T> = std::result::Result<T, Error>;

// Conversion from ormdb_core::Error
impl From<ormdb_core::error::Error> for Error {
    fn from(err: ormdb_core::error::Error) -> Self {
        match err {
            ormdb_core::error::Error::Storage(e) => Error::Storage(e.to_string()),
            ormdb_core::error::Error::Protocol(e) => Error::Internal(e.to_string()),
            ormdb_core::error::Error::Serialization(msg) => Error::Serialization(msg),
            ormdb_core::error::Error::Deserialization(msg) => Error::Serialization(msg),
            ormdb_core::error::Error::InvalidKey => Error::Internal("invalid key format".into()),
            ormdb_core::error::Error::NotFound => Error::NotFound("entity not found".into()),
            ormdb_core::error::Error::Transaction(msg) => Error::Transaction(msg),
            ormdb_core::error::Error::InvalidData(msg) => Error::InvalidArgument(msg),
            ormdb_core::error::Error::TransactionConflict {
                entity_id,
                expected,
                actual,
            } => Error::TransactionConflict {
                entity_id,
                expected_version: expected,
                actual_version: actual,
            },
            ormdb_core::error::Error::ConstraintViolation(constraint_err) => {
                match constraint_err {
                    ormdb_core::error::ConstraintError::UniqueViolation {
                        entity,
                        fields,
                        value,
                        ..
                    } => Error::UniqueViolation {
                        entity,
                        field: fields.join(", "),
                        value,
                    },
                    ormdb_core::error::ConstraintError::ForeignKeyViolation {
                        entity,
                        field,
                        referenced_entity,
                        ..
                    } => Error::ForeignKeyViolation {
                        entity,
                        field,
                        referenced_entity,
                    },
                    ormdb_core::error::ConstraintError::CheckViolation { expression, .. } => {
                        Error::ConstraintViolation(format!("check constraint failed: {}", expression))
                    }
                    ormdb_core::error::ConstraintError::RestrictViolation {
                        entity,
                        referencing_entity,
                        count,
                        ..
                    } => Error::ConstraintViolation(format!(
                        "cannot delete {}: {} {} entities reference it",
                        entity, count, referencing_entity
                    )),
                }
            }
            ormdb_core::error::Error::CascadeError(cascade_err) => {
                Error::ConstraintViolation(cascade_err.to_string())
            }
        }
    }
}

// Conversion from sled::Error
impl From<sled::Error> for Error {
    fn from(err: sled::Error) -> Self {
        Error::Storage(err.to_string())
    }
}
