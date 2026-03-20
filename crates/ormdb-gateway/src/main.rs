//! ORMDB HTTP/JSON Gateway binary.

use clap::Parser;
use ormdb_client::{ClientConfig, ConnectionPool, PoolConfig};
use ormdb_gateway::{create_router, AppState, Args, GatewayConfig};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Parse command line args
    let args = Args::parse();
    let config = GatewayConfig::from(&args);

    info!(
        listen = %config.listen_addr,
        ormdb = %config.ormdb_addr,
        "Starting ORMDB Gateway"
    );

    if config.pool_min_connections > config.pool_max_connections {
        anyhow::bail!("pool_min_connections cannot exceed pool_max_connections");
    }

    // Connect to ORMDB server with a connection pool
    let client_config = ClientConfig::new(&config.ormdb_addr)
        .with_timeout(config.client_timeout);
    let pool_config = PoolConfig::new(&config.ormdb_addr)
        .with_min_connections(config.pool_min_connections)
        .with_max_connections(config.pool_max_connections)
        .with_acquire_timeout(config.pool_acquire_timeout)
        .with_idle_timeout(config.pool_idle_timeout)
        .with_client_config(client_config);
    let pool = ConnectionPool::new(pool_config).await?;
    info!(
        min_connections = config.pool_min_connections,
        max_connections = config.pool_max_connections,
        request_timeout_ms = config.request_timeout.as_millis(),
        request_retries = config.request_retries,
        "Connected to ORMDB server"
    );

    // Create application state
    let state = AppState::new(pool, config.clone());

    // Create router
    let app = create_router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!("Gateway listening on {}", config.listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
