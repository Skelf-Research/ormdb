//! Protocol error types.

use thiserror::Error;

/// Protocol-level errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Serialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Deserialization failed.
    #[error("deserialization error: {0}")]
    Deserialization(String),

    /// Protocol version mismatch.
    #[error("protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    /// Invalid message format.
    #[error("invalid message: {0}")]
    InvalidMessage(String),
}
