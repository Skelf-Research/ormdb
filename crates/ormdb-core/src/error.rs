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
}
