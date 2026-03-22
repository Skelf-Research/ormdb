use clap::Parser;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ormdb_studio::{
    config::{Args, StudioConfig},
    create_router,
    session::cleanup_task,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging
    let log_filter = args.log_level.clone();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("ormdb_studio={},tower_http=info", log_filter).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create configuration
    let config: StudioConfig = args.into();
    let listen_addr = config.listen_addr();
    let base_url = config.base_url();
    let open_browser = config.open_browser;

    // Create application state
    let state = AppState::new(config.clone());

    // Start session cleanup background task
    let cleanup_manager = state.sessions.clone();
    tokio::spawn(async move {
        cleanup_task(cleanup_manager, Duration::from_secs(60)).await;
    });

    // Create router
    let app = create_router(state);

    // Bind to address
    let listener = TcpListener::bind(&listen_addr).await?;

    tracing::info!("ORMDB Studio starting on {}", base_url);
    tracing::info!("API available at {}/api/session", base_url);
    tracing::info!("Health check at {}/health", base_url);

    // Open browser if requested
    if open_browser {
        tracing::info!("Opening browser...");
        if let Err(e) = open::that(&base_url) {
            tracing::warn!("Failed to open browser: {}", e);
        }
    }

    println!();
    println!("  ╔═══════════════════════════════════════════════════════╗");
    println!("  ║                                                       ║");
    println!("  ║   ORMDB Studio is running!                            ║");
    println!("  ║                                                       ║");
    println!("  ║   Local:   {}   ║", format!("{:<38}", base_url));
    println!("  ║                                                       ║");
    println!("  ║   Press Ctrl+C to stop                                ║");
    println!("  ║                                                       ║");
    println!("  ╚═══════════════════════════════════════════════════════╝");
    println!();

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}
