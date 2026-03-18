//! Core error types.

use thiserror::Error;

/// Core database errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Storage layer error.
    #[error("storage error: {0}")]
    Storage(#[from] sled::Error),

    /// Protocol error.
    #[error("protocol error: {0}")]
    Protocol(#[from] ormdb_proto::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("deserialization error: {0}")]
    Deserialization(String),

    /// Key decoding error.
    #[error("invalid key format")]
    InvalidKey,

    /// Record not found.
    #[error("record not found")]
    NotFound,

    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Invalid data format.
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// Constraint violation error.
    #[error("constraint violation: {0}")]
    ConstraintViolation(#[from] ConstraintError),

    /// Transaction conflict (optimistic concurrency).
    #[error("transaction conflict on entity {entity_id:?}: expected version {expected}, found {actual}")]
    TransactionConflict {
        /// The entity ID that had a conflict.
        entity_id: [u8; 16],
        /// Expected version.
        expected: u64,
        /// Actual version found.
        actual: u64,
    },

    /// Cascade error during delete.
    #[error("cascade error: {0}")]
    CascadeError(#[from] CascadeError),
}

/// Constraint violation errors.
#[derive(Debug, Error, Clone)]
pub enum ConstraintError {
    /// Unique constraint violation.
    #[error("unique constraint '{constraint}' violated on {entity}.{fields:?}: duplicate value '{value}'")]
    UniqueViolation {
        /// Constraint name.
        constraint: String,
        /// Entity name.
        entity: String,
        /// Fields involved.
        fields: Vec<String>,
        /// Duplicate value (formatted).
        value: String,
    },

    /// Foreign key constraint violation.
    #[error("foreign key constraint '{constraint}' violated: {entity}.{field} references non-existent {referenced_entity}")]
    ForeignKeyViolation {
        /// Constraint name.
        constraint: String,
        /// Entity name.
        entity: String,
        /// Foreign key field.
        field: String,
        /// Referenced entity name.
        referenced_entity: String,
    },

    /// Check constraint violation.
    #[error("check constraint '{constraint}' violated on {entity}: expression '{expression}' is false")]
    CheckViolation {
        /// Constraint name.
        constraint: String,
        /// Entity name.
        entity: String,
        /// The check expression that failed.
        expression: String,
    },

    /// Restrict violation (cannot delete due to existing references).
    #[error("restrict constraint '{constraint}' violated: cannot delete {entity}, {count} {referencing_entity} entities reference it")]
    RestrictViolation {
        /// Constraint name.
        constraint: String,
        /// Entity being deleted.
        entity: String,
        /// Entity type that references this entity.
        referencing_entity: String,
        /// Number of referencing entities.
        count: usize,
    },
}

/// Cascade operation errors.
#[derive(Debug, Error, Clone)]
pub enum CascadeError {
    /// Cannot delete due to restrict behavior.
    #[error("restrict violation: cannot delete {entity}, {count} {referencing_entity} entities reference it")]
    RestrictViolation {
        /// Entity being deleted.
        entity: String,
        /// Entity type that references this entity.
        referencing_entity: String,
        /// Number of referencing entities.
        count: usize,
    },

    /// Circular cascade detected.
    #[error("circular cascade detected: {path:?}")]
    CircularCascade {
        /// The cascade path that formed a cycle.
        path: Vec<String>,
    },

    /// Maximum cascade depth exceeded.
    #[error("maximum cascade depth exceeded: {depth}")]
    MaxDepthExceeded {
        /// The depth at which the limit was exceeded.
        depth: usize,
    },
}
