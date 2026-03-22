use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "ormdb-studio",
        "version": env!("CARGO_PKG_VERSION"),
        "sessions": {
            "active": state.sessions.session_count(),
            "max": state.config.max_sessions,
        }
    }))
}
