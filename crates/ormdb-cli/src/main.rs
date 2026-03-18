//! ORMDB Command-Line Client
//!
//! An interactive CLI for querying and managing ORMDB databases.

mod commands;
mod completer;
mod executor;
mod formatter;
mod repl;

use clap::Parser;
use formatter::OutputFormat;
use std::path::PathBuf;
use std::time::Duration;

/// ORMDB Command-Line Client
#[derive(Parser, Debug)]
#[command(name = "ormdb-cli")]
#[command(version, about = "ORMDB Command-Line Client")]
pub struct Args {
    /// Server address (tcp:// or ipc://)
    #[arg(short = 'H', long, default_value = "tcp://127.0.0.1:9000")]
    pub host: String,

    /// Execute a single query and exit
    #[arg(short = 'c', long)]
    pub command: Option<String>,

    /// Execute queries from file
    #[arg(short = 'f', long)]
    pub file: Option<PathBuf>,

    /// Output format
    #[arg(long, default_value = "table", value_enum)]
    pub format: OutputFormat,

    /// Connection timeout in seconds
    #[arg(long, default_value_t = 30)]
    pub timeout: u64,

    /// Maximum message size in MB
    #[arg(long, default_value_t = 64)]
    pub max_message_mb: usize,
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
    let config = ormdb_client::ClientConfig::new(&args.host)
        .with_timeout(Duration::from_secs(args.timeout))
        .with_max_message_size(args.max_message_mb * 1024 * 1024);

    // Determine which mode to run in
    if let Some(command) = &args.command {
        // Command mode: execute single query and exit
        run_command_mode(&config, command, args.format).await
    } else if let Some(file) = &args.file {
        // Script mode: execute queries from file
        run_script_mode(&config, file, args.format).await
    } else {
        // REPL mode: interactive shell
        run_repl_mode(config, args.format).await
    }
}

/// Execute a single command and exit.
async fn run_command_mode(
    config: &ormdb_client::ClientConfig,
    command: &str,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = ormdb_client::Client::connect(config.clone()).await?;
    let formatter = formatter::create_formatter(format);

    match executor::execute(&client, command, &*formatter).await {
        Ok(output) => {
            println!("{}", output);
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Execute queries from a file.
async fn run_script_mode(
    config: &ormdb_client::ClientConfig,
    file: &PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let client = ormdb_client::Client::connect(config.clone()).await?;
    let formatter = formatter::create_formatter(format);

    // Split by lines, filter empty lines and comments
    let statements: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect();

    for statement in statements {
        match executor::execute(&client, statement, &*formatter).await {
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

    Ok(())
}

/// Run the interactive REPL.
async fn run_repl_mode(
    config: ormdb_client::ClientConfig,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    repl::run(config, format).await
}
