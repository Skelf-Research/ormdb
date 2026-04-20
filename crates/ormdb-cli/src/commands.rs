//! REPL dot-command handling.

use crate::backend::Backend;
use crate::formatter::OutputFormat;
use crate::mode::ConnectionMode;

/// Result of executing a command.
pub enum CommandResult {
    /// Continue the REPL.
    Continue,
    /// Exit the REPL.
    Exit,
    /// Output to display.
    Output(String),
    /// Change the output format.
    SetFormat(OutputFormat),
    /// Connect to a new server (client mode).
    Connect(String),
    /// Disconnect from server.
    Disconnect,
    /// Show history.
    ShowHistory,
    /// Clear screen.
    Clear,
    /// Explain a query.
    Explain(String),
    /// Get server metrics.
    GetMetrics,
    /// Flush to disk (embedded mode).
    Flush,
    /// Compact the database (embedded mode).
    Compact,
    /// Create a backup (embedded mode).
    Backup(BackupOptions),
    /// Restore from backup (embedded mode).
    Restore(RestoreOptions),
    /// Show backup status (embedded mode).
    BackupStatus,
}

/// Options for backup command.
pub struct BackupOptions {
    /// S3 destination URL (s3://bucket/path).
    pub destination: String,
    /// Whether this is an incremental backup.
    pub incremental: bool,
}

/// Options for restore command.
pub struct RestoreOptions {
    /// S3 source URL (s3://bucket/path).
    pub source: String,
    /// Target LSN for point-in-time recovery.
    pub target_lsn: Option<u64>,
}

/// Parse and execute a dot-command.
pub async fn handle_command(
    line: &str,
    backend: &Option<Backend>,
    format: OutputFormat,
) -> CommandResult {
    let line = line.trim();
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim());

    match command.as_str() {
        ".exit" | ".quit" | ".q" => CommandResult::Exit,

        ".help" | ".h" | ".?" => CommandResult::Output(get_help(backend)),

        ".clear" | ".cls" => CommandResult::Clear,

        ".status" => {
            if let Some(backend) = backend {
                let mode_str = match backend.mode() {
                    ConnectionMode::Embedded { path: None } => "Embedded (in-memory)".to_string(),
                    ConnectionMode::Embedded { path: Some(p) } => {
                        format!("Embedded ({})", p.display())
                    }
                    ConnectionMode::Client { url } => format!("Client ({})", url),
                };
                let version = backend.schema_version();
                CommandResult::Output(format!("{}\nSchema version: {}", mode_str, version))
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".connect" => {
            if let Some(backend) = backend {
                if backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Cannot use .connect in embedded mode. Restart with a server URL."
                            .to_string(),
                    );
                }
            }
            if let Some(addr) = arg {
                CommandResult::Connect(addr.to_string())
            } else {
                CommandResult::Output("Usage: .connect <address>".to_string())
            }
        }

        ".disconnect" => {
            if let Some(backend) = backend {
                if backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Cannot disconnect in embedded mode. Use .exit to close.".to_string(),
                    );
                }
            }
            CommandResult::Disconnect
        }

        ".format" => {
            if let Some(fmt) = arg {
                match fmt.to_lowercase().as_str() {
                    "table" => CommandResult::SetFormat(OutputFormat::Table),
                    "json" => CommandResult::SetFormat(OutputFormat::Json),
                    "csv" => CommandResult::SetFormat(OutputFormat::Csv),
                    _ => CommandResult::Output(format!(
                        "Unknown format '{}'. Use: table, json, csv",
                        fmt
                    )),
                }
            } else {
                CommandResult::Output(format!("Current format: {}", format))
            }
        }

        ".schema" => {
            if let Some(backend) = backend {
                // If no argument, list entities
                // If argument, describe that entity
                if let Some(entity) = arg {
                    CommandResult::Output(format!(
                        "Describe entity '{}' (schema deserialization not yet implemented)",
                        entity
                    ))
                } else {
                    match backend.get_schema().await {
                        Ok((version, bytes)) => {
                            if bytes.is_empty() {
                                CommandResult::Output(format!(
                                    "Schema version {} (embedded mode)",
                                    version
                                ))
                            } else {
                                CommandResult::Output(format!(
                                    "Schema version {} ({} bytes)\n\
                                     (Entity listing requires schema deserialization)",
                                    version,
                                    bytes.len()
                                ))
                            }
                        }
                        Err(e) => CommandResult::Output(format!("Error: {}", e)),
                    }
                }
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".history" => CommandResult::ShowHistory,

        ".explain" => {
            if let Some(query_str) = arg {
                if backend.is_some() {
                    CommandResult::Explain(query_str.to_string())
                } else {
                    CommandResult::Output("Not connected".to_string())
                }
            } else {
                CommandResult::Output("Usage: .explain <query>".to_string())
            }
        }

        ".metrics" => {
            if let Some(backend) = backend {
                if backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Metrics are only available in client mode.".to_string(),
                    );
                }
                CommandResult::GetMetrics
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        // Embedded-only commands
        ".flush" => {
            if let Some(backend) = backend {
                if !backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Flush is only available in embedded mode.".to_string(),
                    );
                }
                CommandResult::Flush
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".compact" => {
            if let Some(backend) = backend {
                if !backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Compact is only available in embedded mode.".to_string(),
                    );
                }
                CommandResult::Compact
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".backup" => {
            if let Some(backend) = backend {
                if !backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Backup is only available in embedded mode.".to_string(),
                    );
                }
                if let Some(dest) = arg {
                    let incremental = line.contains("--incremental");
                    // Remove --incremental flag from destination if present
                    let destination = dest
                        .replace("--incremental", "")
                        .trim()
                        .to_string();
                    if destination.starts_with("s3://") {
                        CommandResult::Backup(BackupOptions {
                            destination,
                            incremental,
                        })
                    } else {
                        CommandResult::Output(
                            "Destination must be an S3 URL (s3://bucket/path)".to_string(),
                        )
                    }
                } else {
                    CommandResult::Output(
                        "Usage: .backup <s3://bucket/path> [--incremental]".to_string(),
                    )
                }
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".restore" => {
            if let Some(backend) = backend {
                if !backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Restore is only available in embedded mode.".to_string(),
                    );
                }
                if let Some(source) = arg {
                    // Parse optional --lsn flag
                    let target_lsn = extract_lsn_flag(line);
                    let source_url = source
                        .split_whitespace()
                        .next()
                        .unwrap_or(source)
                        .to_string();
                    if source_url.starts_with("s3://") {
                        CommandResult::Restore(RestoreOptions {
                            source: source_url,
                            target_lsn,
                        })
                    } else {
                        CommandResult::Output(
                            "Source must be an S3 URL (s3://bucket/path)".to_string(),
                        )
                    }
                } else {
                    CommandResult::Output(
                        "Usage: .restore <s3://bucket/path> [--lsn <N>]".to_string(),
                    )
                }
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".backup-status" => {
            if let Some(backend) = backend {
                if !backend.mode().is_embedded() {
                    return CommandResult::Output(
                        "Backup-status is only available in embedded mode.".to_string(),
                    );
                }
                CommandResult::BackupStatus
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        _ => CommandResult::Output(format!("Unknown command: {}", command)),
    }
}

/// Extract --lsn flag value from command line.
fn extract_lsn_flag(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "--lsn" {
            if let Some(lsn_str) = parts.get(i + 1) {
                return lsn_str.parse().ok();
            }
        }
    }
    None
}

/// Check if a line is a dot-command.
pub fn is_command(line: &str) -> bool {
    line.trim().starts_with('.')
}

/// Get help text for REPL commands.
fn get_help(backend: &Option<Backend>) -> String {
    let is_embedded = backend
        .as_ref()
        .map(|b| b.mode().is_embedded())
        .unwrap_or(false);

    let mut help = String::from(
        r#"REPL Commands
=============

"#,
    );

    if is_embedded {
        help.push_str(
            r#"EMBEDDED MODE COMMANDS
----------------------
.status               Show database status and schema version
.schema               Show schema version
.schema <Entity>      Describe a specific entity
.flush                Flush pending writes to disk
.compact              Run compaction to reclaim space
.explain <query>      Show query execution plan
.format [type]        Get or set output format (table, json, csv)
.history              Show query history
.clear                Clear the screen
.help                 Show this help message
.exit / .quit         Exit the REPL

BACKUP COMMANDS (embedded mode)
-------------------------------
.backup <s3://...>    Create full backup to S3
  --incremental       Create incremental backup since last
.restore <s3://...>   Restore from backup
  --lsn <N>           Restore to specific LSN (PITR)
.backup-status        Show backup history and status

"#,
        );
    } else {
        help.push_str(
            r#"CLIENT MODE COMMANDS
--------------------
.connect <address>    Connect to an ORMDB server
.disconnect           Disconnect from the current server
.status               Show connection status and schema version
.schema               List all entities in the schema
.schema <Entity>      Describe a specific entity
.explain <query>      Show query execution plan without running it
.metrics              Show server performance metrics
.format [type]        Get or set output format (table, json, csv)
.history              Show query history
.clear                Clear the screen
.help                 Show this help message
.exit / .quit         Exit the REPL

"#,
        );
    }

    help.push_str(
        r#"Query Language
==============
Type .help language or use the .help command in a query for full syntax.

Examples:
  User.findMany()
  User.findMany().where(status == "active").limit(10)
  User.create({ name: "Alice", email: "alice@example.com" })
"#,
    );

    help
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_command() {
        assert!(is_command(".exit"));
        assert!(is_command(".help"));
        assert!(is_command("  .status"));
        assert!(!is_command("User.findMany()"));
        assert!(!is_command("hello"));
    }
}
