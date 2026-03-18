//! ORMDB HTTP/JSON Gateway binary.

use clap::Parser;
use ormdb_client::Client;
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

    // Connect to ORMDB server
    let client = Client::connect_to(&config.ormdb_addr).await?;
    info!("Connected to ORMDB server");

    // Create application state
    let state = AppState::new(client, config.clone());

    // Create router
    let app = create_router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!("Gateway listening on {}", config.listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
