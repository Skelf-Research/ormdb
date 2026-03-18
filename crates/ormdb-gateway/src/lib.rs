//! ORMDB HTTP/JSON Gateway.
//!
//! This crate provides an HTTP/JSON gateway for ORMDB, allowing non-Rust clients
//! to communicate with the database using standard REST APIs.

pub mod config;
pub mod error;
pub mod json;
pub mod routes;

pub use config::{Args, GatewayConfig};
pub use error::AppError;

use std::sync::Arc;

use axum::Router;
use ormdb_client::Client;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Application state shared across all routes.
#[derive(Clone)]
pub struct AppState {
    /// ORMDB client connection.
    pub client: Arc<Client>,
    /// Gateway configuration.
    pub config: GatewayConfig,
}

impl AppState {
    /// Create new application state.
    pub fn new(client: Client, config: GatewayConfig) -> Self {
        Self {
            client: Arc::new(client),
            config,
        }
    }
}

/// Create the router with all routes.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(routes::health::routes())
        .merge(routes::query::routes())
        .merge(routes::mutation::routes())
        .merge(routes::schema::routes())
        .merge(routes::replication::routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
