//! REPL dot-command handling.

use crate::formatter::OutputFormat;
use ormdb_client::Client;

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
    /// Connect to a new server.
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
}

/// Parse and execute a dot-command.
pub async fn handle_command(
    line: &str,
    client: &Option<Client>,
    format: OutputFormat,
) -> CommandResult {
    let line = line.trim();
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim());

    match command.as_str() {
        ".exit" | ".quit" | ".q" => CommandResult::Exit,

        ".help" | ".h" | ".?" => CommandResult::Output(get_help()),

        ".clear" | ".cls" => CommandResult::Clear,

        ".status" => {
            if let Some(client) = client {
                let version = client.schema_version();
                CommandResult::Output(format!(
                    "Connected (schema version: {})",
                    version
                ))
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        ".connect" => {
            if let Some(addr) = arg {
                CommandResult::Connect(addr.to_string())
            } else {
                CommandResult::Output("Usage: .connect <address>".to_string())
            }
        }

        ".disconnect" => CommandResult::Disconnect,

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
            if let Some(client) = client {
                // If no argument, list entities
                // If argument, describe that entity
                if let Some(entity) = arg {
                    CommandResult::Output(format!(
                        "Describe entity '{}' (schema deserialization not yet implemented)",
                        entity
                    ))
                } else {
                    match client.get_schema().await {
                        Ok((version, bytes)) => {
                            CommandResult::Output(format!(
                                "Schema version {} ({} bytes)\n\
                                 (Entity listing requires schema deserialization)",
                                version,
                                bytes.len()
                            ))
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
                if client.is_some() {
                    CommandResult::Explain(query_str.to_string())
                } else {
                    CommandResult::Output("Not connected".to_string())
                }
            } else {
                CommandResult::Output("Usage: .explain <query>".to_string())
            }
        }

        ".metrics" => {
            if client.is_some() {
                CommandResult::GetMetrics
            } else {
                CommandResult::Output("Not connected".to_string())
            }
        }

        _ => CommandResult::Output(format!("Unknown command: {}", command)),
    }
}

/// Check if a line is a dot-command.
pub fn is_command(line: &str) -> bool {
    line.trim().starts_with('.')
}

/// Get help text for REPL commands.
fn get_help() -> String {
    r#"REPL Commands
=============

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

Query Language
==============
Type .help language or use the .help command in a query for full syntax.

Examples:
  User.findMany()
  User.findMany().where(status == "active").limit(10)
  User.create({ name: "Alice", email: "alice@example.com" })
"#
    .to_string()
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
