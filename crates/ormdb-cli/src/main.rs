//! ORMDB Command-Line Interface
//!
//! A unified CLI for ORMDB that supports both embedded and client modes.
//!
//! # Usage
//!
//! ```bash
//! # Embedded mode (local database file)
//! ormdb ./my_data                      # Open local database
//! ormdb :memory:                       # In-memory database
//! ormdb ./my_data -c "User.findMany()" # Command mode
//!
//! # Client mode (remote server)
//! ormdb tcp://localhost:9000           # Connect to server
//! ormdb ipc:///tmp/ormdb.sock          # Connect via IPC
//! ```

mod backend;
mod commands;
mod completer;
mod executor;
mod formatter;
mod mode;
mod repl;

use clap::Parser;
use formatter::OutputFormat;
use mode::ConnectionMode;
use std::path::PathBuf;

/// ORMDB Command-Line Interface
#[derive(Parser, Debug)]
#[command(name = "ormdb")]
#[command(version, about = "ORMDB Command-Line Interface - Embedded & Client")]
pub struct Args {
    /// Database path or server URL
    ///
    /// - File path (./data, /path/to/db): Open in embedded mode
    /// - :memory: : Open in-memory embedded database
    /// - tcp://host:port: Connect to server
    /// - ipc:///path: Connect via IPC
    #[arg(default_value = ":memory:")]
    pub target: String,

    /// Legacy: Server address (deprecated, use positional argument)
    #[arg(short = 'H', long, hide = true)]
    pub host: Option<String>,

    /// Execute a single query and exit
    #[arg(short = 'c', long)]
    pub command: Option<String>,

    /// Execute queries from file
    #[arg(short = 'f', long)]
    pub file: Option<PathBuf>,

    /// Output format
    #[arg(long, default_value = "table", value_enum)]
    pub format: OutputFormat,

    /// Connection timeout in seconds (client mode only)
    #[arg(long, default_value_t = 30)]
    pub timeout: u64,

    /// Maximum message size in MB (client mode only)
    #[arg(long, default_value_t = 64)]
    pub max_message_mb: usize,
}

impl Args {
    /// Get the effective target, considering the legacy --host flag.
    fn effective_target(&self) -> &str {
        // Legacy: -H flag takes precedence if provided
        self.host.as_deref().unwrap_or(&self.target)
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ormdb_cli=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    let result = run(args).await;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let target = args.effective_target();
    let mode = ConnectionMode::detect(target);

    // Print mode info
    match &mode {
        ConnectionMode::Embedded { path: None } => {
            tracing::info!("Opening in-memory database");
        }
        ConnectionMode::Embedded { path: Some(p) } => {
            tracing::info!("Opening embedded database: {}", p.display());
        }
        ConnectionMode::Client { url } => {
            tracing::info!("Connecting to server: {}", url);
        }
    }

    // Determine which mode to run in
    if let Some(command) = &args.command {
        // Command mode: execute single query and exit
        run_command_mode(&mode, command, args.format, args.timeout).await
    } else if let Some(file) = &args.file {
        // Script mode: execute queries from file
        run_script_mode(&mode, file, args.format, args.timeout).await
    } else {
        // REPL mode: interactive shell
        run_repl_mode(mode, args.format, args.timeout).await
    }
}

/// Execute a single command and exit.
async fn run_command_mode(
    mode: &ConnectionMode,
    command: &str,
    format: OutputFormat,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = backend::create_backend(mode.clone(), timeout).await?;
    let formatter = formatter::create_formatter(format);

    match executor::execute(&backend, command, &*formatter).await {
        Ok(output) => {
            println!("{}", output);
            backend.close().await?;
            Ok(())
        }
        Err(e) => {
            backend.close().await?;
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Execute queries from a file.
async fn run_script_mode(
    mode: &ConnectionMode,
    file: &PathBuf,
    format: OutputFormat,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let backend = backend::create_backend(mode.clone(), timeout).await?;
    let formatter = formatter::create_formatter(format);

    // Split by lines, filter empty lines and comments
    let statements: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect();

    for statement in statements {
        match executor::execute(&backend, statement, &*formatter).await {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                eprintln!("Error executing '{}': {}", statement, e);
                // Continue with next statement
            }
        }
    }

    backend.close().await?;
    Ok(())
}

/// Run the interactive REPL.
async fn run_repl_mode(
    mode: ConnectionMode,
    format: OutputFormat,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    repl::run(mode, format, timeout).await
}
