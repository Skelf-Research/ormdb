//! Server error types.

use thiserror::Error;

/// Server errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Database error.
    #[error("database error: {0}")]
    Database(String),

    /// Storage error.
    #[error("storage error: {0}")]
    Storage(#[from] ormdb_core::error::Error),

    /// Protocol error.
    #[error("protocol error: {0}")]
    Protocol(#[from] ormdb_proto::Error),

    /// Transport error.
    #[error("transport error: {0}")]
    Transport(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
