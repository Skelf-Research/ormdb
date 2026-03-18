//! Interactive REPL implementation.

use crate::commands::{self, CommandResult};
use crate::completer::OrmdbHelper;
use crate::executor;
use crate::formatter::{self, OutputFormat};
use ormdb_client::{Client, ClientConfig};
use rustyline::error::ReadlineError;
use rustyline::history::{DefaultHistory, History};
use rustyline::{Config, Editor};
use std::path::PathBuf;
use std::time::Duration;

/// Get the history file path.
fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ormdb_history")
}

/// Run the interactive REPL.
pub async fn run(
    config: ClientConfig,
    initial_format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // Try to connect
    let mut client: Option<Client> = match Client::connect(config.clone()).await {
        Ok(c) => {
            println!(
                "Connected to {} (schema v{})",
                config.address,
                c.schema_version()
            );
            Some(c)
        }
        Err(e) => {
            println!("Warning: Could not connect to {}: {}", config.address, e);
            println!("Use .connect <address> to connect.\n");
            None
        }
    };

    let mut format = initial_format;
    let mut _host = config.address.clone();

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

    println!("ORMDB CLI - Type .help for commands, .exit to quit\n");

    // Main REPL loop
    loop {
        let prompt = if client.is_some() {
            "ormdb> "
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
                    match commands::handle_command(line, &client, format).await {
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
                            if let Some(c) = client.take() {
                                let _ = c.close().await;
                            }

                            // Connect to new address
                            let new_config = ClientConfig::new(&addr)
                                .with_timeout(Duration::from_secs(30));

                            match Client::connect(new_config).await {
                                Ok(c) => {
                                    println!(
                                        "Connected to {} (schema v{})",
                                        addr,
                                        c.schema_version()
                                    );
                                    _host = addr;
                                    client = Some(c);
                                }
                                Err(e) => {
                                    println!("Failed to connect: {}", e);
                                }
                            }
                        }
                        CommandResult::Disconnect => {
                            if let Some(c) = client.take() {
                                let _ = c.close().await;
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
                            if let Some(ref c) = client {
                                match executor::explain(c, &query_str).await {
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
                            if let Some(ref c) = client {
                                match executor::get_metrics(c).await {
                                    Ok(result) => {
                                        println!("{}", executor::format_metrics(&result));
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
                if let Some(ref c) = client {
                    let formatter = formatter::create_formatter(format);
                    match executor::execute(c, line, &*formatter).await {
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

    // Close client
    if let Some(c) = client {
        let _ = c.close().await;
    }

    Ok(())
}
