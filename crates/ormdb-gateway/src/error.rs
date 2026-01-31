//! Error handling for the gateway.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Application error type.
#[derive(Debug)]
pub enum AppError {
    /// Internal server error.
    Internal(String),
    /// Bad request.
    BadRequest(String),
    /// Not found.
    NotFound(String),
    /// Client error (communication with ORMDB).
    ClientError(String),
}

/// Error response body.
#[derive(Serialize)]
pub struct ErrorResponse {
    /// Error flag.
    pub error: bool,
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg),
            AppError::ClientError(msg) => (StatusCode::BAD_GATEWAY, "CLIENT_ERROR", msg),
        };

        let body = ErrorResponse {
            error: true,
            code: code.to_string(),
            message,
        };

        (status, Json(body)).into_response()
    }
}

impl From<ormdb_client::Error> for AppError {
    fn from(err: ormdb_client::Error) -> Self {
        AppError::ClientError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::BadRequest(format!("JSON error: {}", err))
    }
}
