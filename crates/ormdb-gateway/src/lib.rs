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

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use ormdb_client::{ConnectionPool, Error as ClientError};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Application state shared across all routes.
#[derive(Clone)]
pub struct AppState {
    /// ORMDB connection pool.
    pub pool: Arc<ConnectionPool>,
    /// Gateway configuration.
    pub config: GatewayConfig,
}

impl AppState {
    /// Create new application state.
    pub fn new(pool: ConnectionPool, config: GatewayConfig) -> Self {
        Self {
            pool: Arc::new(pool),
            config,
        }
    }

    /// Execute a read-only request with retry and timeout handling.
    pub async fn execute_read<T, F, Fut>(&self, mut op: F) -> Result<T, AppError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        self.execute_with_retry(self.config.request_retries, &mut op)
            .await
    }

    /// Execute a write request without retries, but with timeout handling.
    pub async fn execute_write<T, F, Fut>(&self, mut op: F) -> Result<T, AppError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        self.execute_with_retry(0, &mut op).await
    }

    async fn execute_with_retry<T, F, Fut>(
        &self,
        retries: usize,
        op: &mut F,
    ) -> Result<T, AppError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        let mut attempt = 0;
        loop {
            let result = tokio::time::timeout(self.config.request_timeout, op()).await;
            let result = match result {
                Ok(res) => res,
                Err(_) => Err(ClientError::Timeout),
            };

            match result {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if attempt >= retries || !Self::should_retry(&err) {
                        return Err(AppError::from(err));
                    }
                    attempt += 1;
                    let backoff = self
                        .config
                        .request_retry_backoff
                        .saturating_mul(attempt as u32);
                    if backoff > Duration::from_millis(0) {
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }
    }

    fn should_retry(err: &ClientError) -> bool {
        match err {
            ClientError::Timeout | ClientError::Connection(_) => true,
            ClientError::Pool(message) => message.contains("timeout waiting for connection"),
            _ => false,
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
