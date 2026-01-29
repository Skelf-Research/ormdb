//! ORMDB Server - Standalone database server.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // Initialize tracing.
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

    // TODO: Initialize storage, transport, and start serving.
    tracing::info!("ORMDB server initialized (no-op in Phase 0)");
}
