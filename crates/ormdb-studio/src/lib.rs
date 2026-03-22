//! ORMDB Studio - Web-based database management
//!
//! This crate provides a browser-based interface for ORMDB with:
//! - Query REPL with syntax highlighting
//! - Terminal emulator (xterm.js)
//! - Visual query builder
//! - Session-isolated databases

pub mod config;
pub mod error;
pub mod routes;
pub mod session;
pub mod state;
pub mod ws;

use axum::{routing::get, Router};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::state::AppState;

/// Create the Axum router with all routes
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .merge(routes::health::routes())
        // REST API
        .merge(routes::api::routes())
        // WebSocket terminal
        .route("/ws/terminal/:session_id", get(ws::ws_terminal))
        // Static files (Vue.js frontend) - must be last (fallback)
        .fallback(routes::static_files::serve_static)
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
