//! Interactive REPL implementation.

use crate::backend::{self, Backend, ClientBackend};
use crate::commands::{self, CommandResult};
use crate::completer::OrmdbHelper;
use crate::executor;
use crate::formatter::{self, OutputFormat};
use crate::mode::ConnectionMode;
use rustyline::error::ReadlineError;
use rustyline::history::{DefaultHistory, History};
use rustyline::{Config, Editor};
use std::path::PathBuf;

/// Get the history file path.
fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ormdb_history")
}

/// Run the interactive REPL.
pub async fn run(
    mode: ConnectionMode,
    initial_format: OutputFormat,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create backend
    let mut backend: Option<Backend> =
        match backend::create_backend(mode.clone(), timeout).await {
            Ok(b) => {
                let msg = match b.mode() {
                    ConnectionMode::Embedded { path: None } => {
                        "Opened in-memory database".to_string()
                    }
                    ConnectionMode::Embedded { path: Some(p) } => {
                        format!("Opened database: {} (schema v{})", p.display(), b.schema_version())
                    }
                    ConnectionMode::Client { url } => {
                        format!("Connected to {} (schema v{})", url, b.schema_version())
                    }
                };
                println!("{}", msg);
                Some(b)
            }
            Err(e) => {
                match &mode {
                    ConnectionMode::Client { url } => {
                        println!("Warning: Could not connect to {}: {}", url, e);
                        println!("Use .connect <address> to connect.\n");
                    }
                    ConnectionMode::Embedded { .. } => {
                        println!("Error: Could not open database: {}", e);
                        return Err(e.into());
                    }
                }
                None
            }
        };

    let mut format = initial_format;

    // Set up rustyline
    let rl_config = Config::builder()
        .history_ignore_space(true)
        .auto_add_history(true)
        .build();

    let helper = OrmdbHelper::new();
    let mut rl: Editor<OrmdbHelper, DefaultHistory> = Editor::with_config(rl_config)?;
    rl.set_helper(Some(helper));

    // Load history
    let hist_path = history_path();
    if hist_path.exists() {
        let _ = rl.load_history(&hist_path);
    }

    // Print welcome message
    let mode_name = if mode.is_embedded() {
        "embedded"
    } else {
        "client"
    };
    println!(
        "ORMDB CLI ({} mode) - Type .help for commands, .exit to quit\n",
        mode_name
    );

    // Main REPL loop
    loop {
        let prompt = if let Some(ref b) = backend {
            if b.mode().is_embedded() {
                "ormdb> "
            } else {
                "ormdb> "
            }
        } else {
            "ormdb (disconnected)> "
        };

        match rl.readline(prompt) {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                // Handle dot-commands
                if commands::is_command(line) {
                    match commands::handle_command(line, &backend, format).await {
                        CommandResult::Continue => {}
                        CommandResult::Exit => {
                            println!("Goodbye!");
                            break;
                        }
                        CommandResult::Output(msg) => {
                            println!("{}", msg);
                        }
                        CommandResult::SetFormat(fmt) => {
                            format = fmt;
                            println!("Output format set to {}", format);
                        }
                        CommandResult::Connect(addr) => {
                            // Disconnect existing client
                            if let Some(b) = backend.take() {
                                let _ = b.close().await;
                            }

                            // Connect to new address
                            match ClientBackend::connect(&addr, timeout).await {
                                Ok(b) => {
                                    println!(
                                        "Connected to {} (schema v{})",
                                        addr,
                                        b.schema_version()
                                    );
                                    backend = Some(Backend::Client(b));
                                }
                                Err(e) => {
                                    println!("Failed to connect: {}", e);
                                }
                            }
                        }
                        CommandResult::Disconnect => {
                            if let Some(b) = backend.take() {
                                let _ = b.close().await;
                                println!("Disconnected");
                            } else {
                                println!("Not connected");
                            }
                        }
                        CommandResult::ShowHistory => {
                            let history = rl.history();
                            let len = history.len();
                            let start = if len > 20 { len - 20 } else { 0 };
                            for (i, entry) in history.iter().skip(start).enumerate() {
                                println!("{:4}  {}", start + i + 1, entry);
                            }
                        }
                        CommandResult::Clear => {
                            // ANSI clear screen
                            print!("\x1B[2J\x1B[1;1H");
                        }
                        CommandResult::Explain(query_str) => {
                            if let Some(ref b) = backend {
                                match executor::explain(b, &query_str).await {
                                    Ok(result) => {
                                        println!("{}", executor::format_explain(&result));
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                        CommandResult::GetMetrics => {
                            if let Some(ref b) = backend {
                                match executor::get_metrics(b).await {
                                    Ok(result) => {
                                        println!("{}", executor::format_metrics(&result));
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                        CommandResult::Flush => {
                            if let Some(ref b) = backend {
                                match b.flush().await {
                                    Ok(()) => {
                                        println!("Flushed to disk");
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                        CommandResult::Compact => {
                            if let Some(ref b) = backend {
                                match b.compact().await {
                                    Ok(msg) => {
                                        println!("{}", msg);
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                        CommandResult::Backup(options) => {
                            if let Some(ref b) = backend {
                                match b.backup(&options.destination, options.incremental).await {
                                    Ok(msg) => {
                                        println!("{}", msg);
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                        CommandResult::Restore(options) => {
                            println!("Restore is not yet implemented in the REPL.");
                            println!(
                                "To restore, use: ormdb --restore {} {}",
                                options.source,
                                options.target_lsn.map(|l| format!("--lsn {}", l)).unwrap_or_default()
                            );
                        }
                        CommandResult::BackupStatus => {
                            if let Some(ref b) = backend {
                                match b.backup_status().await {
                                    Ok(msg) => {
                                        println!("{}", msg);
                                    }
                                    Err(e) => {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }

                // Execute query/mutation
                if let Some(ref b) = backend {
                    let formatter = formatter::create_formatter(format);
                    match executor::execute(b, line, &*formatter).await {
                        Ok(output) => {
                            if !output.is_empty() {
                                println!("{}", output);
                            }
                        }
                        Err(e) => {
                            println!("{}", e);
                        }
                    }
                } else {
                    println!("Not connected. Use .connect <address> to connect.");
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&hist_path);

    // Close backend
    if let Some(b) = backend {
        let _ = b.close().await;
    }

    Ok(())
}
