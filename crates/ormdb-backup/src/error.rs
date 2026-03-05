//! Error types for backup operations.

use thiserror::Error;

/// Result type for backup operations.
pub type Result<T> = std::result::Result<T, BackupError>;

/// Errors that can occur during backup operations.
#[derive(Debug, Error)]
pub enum BackupError {
    /// Storage I/O error.
    #[error("storage error: {0}")]
    Storage(#[from] object_store::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Compression error.
    #[error("compression error: {0}")]
    Compression(#[from] std::io::Error),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] ormdb_core::Error),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// No backup found.
    #[error("no backup found")]
    NoBackupFound,

    /// Backup chain is broken.
    #[error("backup chain broken: missing LSN range {0}-{1}")]
    BrokenChain(u64, u64),

    /// Checksum mismatch.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Invalid manifest.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// Restore target exists.
    #[error("restore target already exists: {0}")]
    TargetExists(String),

    /// LSN not found in backup chain.
    #[error("LSN {0} not found in backup chain")]
    LsnNotFound(u64),
}

impl From<rkyv::rancor::Error> for BackupError {
    fn from(e: rkyv::rancor::Error) -> Self {
        BackupError::Serialization(e.to_string())
    }
}

impl From<serde_json::Error> for BackupError {
    fn from(e: serde_json::Error) -> Self {
        BackupError::Serialization(e.to_string())
    }
}

impl From<sled::Error> for BackupError {
    fn from(e: sled::Error) -> Self {
        BackupError::Database(ormdb_core::Error::Storage(e))
    }
}
