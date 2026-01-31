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

    /// Server returned an error.
    #[error("server error (code {code}): {message}")]
    Server {
        /// Error code from the server.
        code: u32,
        /// Error message from the server.
        message: String,
    },

    /// Pool error.
    #[error("pool error: {0}")]
    Pool(String),
}
