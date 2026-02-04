use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StudioError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Too many sessions (max: {0})")]
    TooManySessions(usize),

    #[error("Session expired")]
    SessionExpired,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Query parse error: {0}")]
    ParseError(String),

    #[error("Query execution error: {0}")]
    ExecutionError(String),

    #[error("Schema error: {0}")]
    SchemaError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for StudioError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            StudioError::SessionNotFound(_) => {
                (StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", self.to_string())
            }
            StudioError::TooManySessions(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "TOO_MANY_SESSIONS",
                self.to_string(),
            ),
            StudioError::SessionExpired => {
                (StatusCode::GONE, "SESSION_EXPIRED", self.to_string())
            }
            StudioError::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", self.to_string())
            }
            StudioError::ParseError(_) => {
                (StatusCode::BAD_REQUEST, "PARSE_ERROR", self.to_string())
            }
            StudioError::ExecutionError(_) => {
                (StatusCode::BAD_REQUEST, "EXECUTION_ERROR", self.to_string())
            }
            StudioError::SchemaError(_) => {
                (StatusCode::BAD_REQUEST, "SCHEMA_ERROR", self.to_string())
            }
            StudioError::InvalidRequest(_) => {
                (StatusCode::BAD_REQUEST, "INVALID_REQUEST", self.to_string())
            }
            StudioError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", self.to_string())
            }
            StudioError::Io(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "IO_ERROR", self.to_string())
            }
        };

        let body = Json(json!({
            "success": false,
            "error": {
                "code": error_code,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, StudioError>;
