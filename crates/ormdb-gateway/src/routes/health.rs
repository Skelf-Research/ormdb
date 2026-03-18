//! Health check endpoint.

use axum::{extract::State, routing::get, Json, Router};

use crate::json::HealthResponse;
use crate::AppState;

/// Health check routes.
pub fn routes() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

/// Health check handler.
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    // Try to ping the ORMDB server
    let ormdb_connected = state.client.ping().await.is_ok();

    Json(HealthResponse {
        status: if ormdb_connected { "healthy" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        ormdb_connected,
    })
}
