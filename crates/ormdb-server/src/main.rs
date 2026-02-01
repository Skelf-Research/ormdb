//! ORMDB Server - Standalone database server.

use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ormdb_core::metrics::new_shared_registry;
use ormdb_server::{create_transport, Args, Database, RequestHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ormdb_server=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        protocol_version = ormdb_proto::PROTOCOL_VERSION,
        "starting ORMDB server"
    );

    // Parse command-line arguments
    let args = Args::parse();
    let config = args.into_config();

    tracing::info!(
        data_path = %config.data_path.display(),
        tcp_address = ?config.tcp_address,
        ipc_address = ?config.ipc_address,
        "configuration loaded"
    );

    // Open the database
    tracing::info!("opening database");
    let database = Database::open(&config.data_path)?;
    let schema_version = database.schema_version();
    tracing::info!(schema_version, "database opened");

    // Create request handler
    let metrics = new_shared_registry();
    let handler = Arc::new(RequestHandler::with_metrics(Arc::new(database), metrics));

    // Create transport
    let transport = create_transport(&config, handler)?;

    // Set up graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    // Spawn shutdown signal handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "failed to listen for ctrl+c");
            return;
        }
        tracing::info!("received shutdown signal");
        let _ = shutdown_tx_clone.send(());
    });

    // Run the transport
    tracing::info!("server ready, accepting connections");
    match transport.run_until_shutdown(shutdown_rx).await {
        Ok(()) => {
            tracing::info!("server shutdown complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "server error");
            return Err(e.into());
        }
    }

    Ok(())
}
