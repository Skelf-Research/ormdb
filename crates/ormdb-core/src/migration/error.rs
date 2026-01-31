//! Migration-specific error types.

use crate::catalog::FieldType;
use thiserror::Error;

/// Safety grade for a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SafetyGrade {
    /// Grade A: Online, no blocking writes.
    /// Examples: new optional fields, new entities, new optional relations.
    A,
    /// Grade B: Online with background backfill.
    /// Examples: new required fields with defaults, new indexes.
    B,
    /// Grade C: Requires expand/contract workflow.
    /// Examples: compatible type changes, field renames.
    C,
    /// Grade D: Destructive, requires explicit confirmation.
    /// Examples: field removal, entity removal, incompatible type changes.
    D,
}

impl std::fmt::Display for SafetyGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SafetyGrade::A => write!(f, "A (online, non-blocking)"),
            SafetyGrade::B => write!(f, "B (online with backfill)"),
            SafetyGrade::C => write!(f, "C (expand/contract)"),
            SafetyGrade::D => write!(f, "D (destructive)"),
        }
    }
}

/// Migration-specific errors.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// A migration is already in progress.
    #[error("migration already in progress: {migration_id:02x?}")]
    MigrationInProgress {
        /// The ID of the migration in progress.
        migration_id: [u8; 16],
    },

    /// Cannot perform the operation online due to safety grade.
    #[error("cannot perform {operation} online: grade {grade}, requires {requirement}")]
    UnsafeOperation {
        /// The operation being attempted.
        operation: String,
        /// The safety grade of the operation.
        grade: SafetyGrade,
        /// What is required to perform this operation.
        requirement: String,
    },

    /// Backfill operation failed.
    #[error("backfill failed for {entity}.{field}: {reason}")]
    BackfillFailed {
        /// The entity being backfilled.
        entity: String,
        /// The field being backfilled.
        field: String,
        /// The reason for failure.
        reason: String,
    },

    /// A migration step failed.
    #[error("step {step_index} failed: {message}")]
    StepFailed {
        /// The index of the failed step.
        step_index: usize,
        /// Error message.
        message: String,
    },

    /// Rollback failed.
    #[error("rollback failed: {reason}")]
    RollbackFailed {
        /// The reason rollback failed.
        reason: String,
    },

    /// Incompatible type change.
    #[error("incompatible type change: cannot convert {from_type:?} to {to_type:?}")]
    IncompatibleTypeChange {
        /// The source type.
        from_type: FieldType,
        /// The target type.
        to_type: FieldType,
    },

    /// Migration not found.
    #[error("migration not found: {migration_id:02x?}")]
    MigrationNotFound {
        /// The ID of the migration that was not found.
        migration_id: [u8; 16],
    },

    /// Migration state is corrupted.
    #[error("migration state corrupted: {message}")]
    StateCorrupted {
        /// Description of the corruption.
        message: String,
    },

    /// Schema validation failed.
    #[error("schema validation failed: {message}")]
    ValidationFailed {
        /// Description of the validation failure.
        message: String,
    },

    /// No changes detected between schemas.
    #[error("no changes detected between schema versions {from_version} and {to_version}")]
    NoChanges {
        /// Source schema version.
        from_version: u64,
        /// Target schema version.
        to_version: u64,
    },

    /// Storage error.
    #[error("storage error: {0}")]
    Storage(#[from] crate::error::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("deserialization error: {0}")]
    Deserialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_grade_ordering() {
        assert!(SafetyGrade::A < SafetyGrade::B);
        assert!(SafetyGrade::B < SafetyGrade::C);
        assert!(SafetyGrade::C < SafetyGrade::D);
    }

    #[test]
    fn test_safety_grade_display() {
        assert_eq!(SafetyGrade::A.to_string(), "A (online, non-blocking)");
        assert_eq!(SafetyGrade::B.to_string(), "B (online with backfill)");
        assert_eq!(SafetyGrade::C.to_string(), "C (expand/contract)");
        assert_eq!(SafetyGrade::D.to_string(), "D (destructive)");
    }

    #[test]
    fn test_error_display() {
        let err = MigrationError::BackfillFailed {
            entity: "User".to_string(),
            field: "email".to_string(),
            reason: "constraint violation".to_string(),
        };
        assert!(err.to_string().contains("User.email"));
    }
}
