//! Client error types.

use thiserror::Error;

/// Client errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Connection failed.
    #[error("connection error: {0}")]
    Connection(String),

    /// Protocol error.
    #[error("protocol error: {0}")]
    Protocol(#[from] ormdb_proto::Error),

    /// Request timed out.
    #[error("request timed out")]
    Timeout,
}
