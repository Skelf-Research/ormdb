use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::session::Session;
use crate::state::AppState;

/// WebSocket upgrade handler for terminal connections
pub async fn ws_terminal(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let session = state.sessions.get_session(&session_id);
    ws.on_upgrade(move |socket| handle_terminal(socket, session))
}

/// Handle terminal WebSocket connection
async fn handle_terminal(mut socket: WebSocket, session: Option<Arc<Session>>) {
    let Some(session) = session else {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&TerminalResponse::Error {
                    message: "Session not found".to_string(),
                })
                .unwrap(),
            ))
            .await;
        return;
    };

    // Send welcome message
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&TerminalResponse::Output {
                text: format!(
                    "ORMDB Studio Terminal\nSession: {}\nType .help for available commands.\n",
                    session.id
                ),
                format: "text".to_string(),
            })
            .unwrap(),
        ))
        .await;

    // Send initial prompt
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&TerminalResponse::Prompt {
                text: "ormdb> ".to_string(),
            })
            .unwrap(),
        ))
        .await;

    // Handle incoming messages
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!("WebSocket error: {}", e);
                break;
            }
        };

        // Parse the request
        let request: TerminalRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::to_string(&TerminalResponse::Error {
                            message: format!("Invalid request: {}", e),
                        })
                        .unwrap(),
                    ))
                    .await;
                continue;
            }
        };

        // Update session activity
        session.touch();

        // Handle the request
        let response = match request {
            TerminalRequest::Execute { ref command } => {
                execute_command(&session, command).await
            }
            TerminalRequest::GetCompletions { ref prefix } => {
                get_completions(&session, prefix)
            }
        };

        // Send response
        let _ = socket
            .send(Message::Text(serde_json::to_string(&response).unwrap()))
            .await;

        // Send prompt after command execution
        if matches!(request, TerminalRequest::Execute { .. }) {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&TerminalResponse::Prompt {
                        text: "ormdb> ".to_string(),
                    })
                    .unwrap(),
                ))
                .await;
        }
    }
}

/// Terminal request types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum TerminalRequest {
    Execute { command: String },
    GetCompletions { prefix: String },
}

/// Terminal response types
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum TerminalResponse {
    Output { text: String, format: String },
    Prompt { text: String },
    Completions { items: Vec<String> },
    Error { message: String },
}

/// Execute a terminal command
async fn execute_command(session: &Session, command: &str) -> TerminalResponse {
    let command = command.trim();

    if command.is_empty() {
        return TerminalResponse::Output {
            text: String::new(),
            format: "text".to_string(),
        };
    }

    // Handle dot commands
    if command.starts_with('.') {
        return handle_dot_command(session, command);
    }

    // Try to parse and execute as ormdb-lang query
    match ormdb_lang::parse(command) {
        Ok(parsed) => {
            // TODO: Compile and execute the query
            TerminalResponse::Output {
                text: format!("Parsed: {:?}\n(Execution not yet implemented)", parsed),
                format: "text".to_string(),
            }
        }
        Err(e) => TerminalResponse::Error {
            message: format!("Parse error: {:?}", e),
        },
    }
}

/// Handle dot commands (.help, .schema, etc.)
fn handle_dot_command(session: &Session, command: &str) -> TerminalResponse {
    let parts: Vec<&str> = command.splitn(2, char::is_whitespace).collect();
    let cmd = parts.first().map(|s| *s).unwrap_or("");
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd {
        ".help" | ".h" => TerminalResponse::Output {
            text: HELP_TEXT.to_string(),
            format: "text".to_string(),
        },
        ".clear" | ".cls" => TerminalResponse::Output {
            text: "\x1b[2J\x1b[H".to_string(), // ANSI clear screen
            format: "ansi".to_string(),
        },
        ".schema" => {
            match session.database.catalog().current_schema() {
                Ok(Some(schema)) => {
                    let mut output = String::new();
                    output.push_str(&format!("Schema version: {}\n\n", schema.version));

                    if schema.entities.is_empty() {
                        output.push_str("No entities defined yet.\n");
                        output.push_str("Use .entity EntityName { field: Type, ... } to define entities.\n");
                    } else {
                        for (name, entity) in &schema.entities {
                            output.push_str(&format!("entity {} {{\n", name));
                            for field in &entity.fields {
                                let type_str = format_field_type(&field.field_type);
                                let opt = if !field.required { "?" } else { "" };
                                output.push_str(&format!("  {}: {}{}\n", field.name, type_str, opt));
                            }
                            output.push_str("}\n\n");
                        }
                    }
                    TerminalResponse::Output {
                        text: output,
                        format: "text".to_string(),
                    }
                }
                Ok(None) => TerminalResponse::Output {
                    text: "No schema defined yet.\nUse .entity EntityName { field: Type, ... } to define entities.\n".to_string(),
                    format: "text".to_string(),
                },
                Err(e) => TerminalResponse::Error {
                    message: format!("Failed to get schema: {}", e),
                },
            }
        }
        ".entity" => {
            if args.is_empty() {
                return TerminalResponse::Error {
                    message: "Usage: .entity EntityName { field1: Type, field2: Type, ... }".to_string(),
                };
            }
            handle_entity_command(session, args)
        }
        ".session" => TerminalResponse::Output {
            text: format!(
                "Session ID: {}\nAge: {:?}\n",
                session.id,
                session.age()
            ),
            format: "text".to_string(),
        },
        ".exit" | ".quit" | ".q" => TerminalResponse::Output {
            text: "Use the browser to close this session, or call DELETE /api/session/{id}\n"
                .to_string(),
            format: "text".to_string(),
        },
        _ => TerminalResponse::Error {
            message: format!("Unknown command: {}. Type .help for available commands.", cmd),
        },
    }
}

/// Format a field type for display
fn format_field_type(ft: &FieldType) -> String {
    match ft {
        FieldType::Scalar(s) => format_scalar_type(s),
        FieldType::OptionalScalar(s) => format_scalar_type(s),
        FieldType::ArrayScalar(s) => format!("{}[]", format_scalar_type(s)),
        FieldType::Enum { name, .. } => name.clone(),
        FieldType::OptionalEnum { name, .. } => name.clone(),
        FieldType::Embedded { entity } => entity.clone(),
        FieldType::OptionalEmbedded { entity } => entity.clone(),
        FieldType::ArrayEmbedded { entity } => format!("{}[]", entity),
    }
}

fn format_scalar_type(st: &ScalarType) -> String {
    match st {
        ScalarType::Bool => "Bool".to_string(),
        ScalarType::Int32 => "Int".to_string(),
        ScalarType::Int64 => "Int64".to_string(),
        ScalarType::Float32 => "Float".to_string(),
        ScalarType::Float64 => "Float64".to_string(),
        ScalarType::Decimal { precision, scale } => format!("Decimal({},{})", precision, scale),
        ScalarType::String => "String".to_string(),
        ScalarType::Bytes => "Bytes".to_string(),
        ScalarType::Timestamp => "Timestamp".to_string(),
        ScalarType::Uuid => "Uuid".to_string(),
    }
}

/// Handle the .entity command for schema definition
fn handle_entity_command(session: &Session, args: &str) -> TerminalResponse {
    // Parse: EntityName { field1: Type, field2: Type }
    let trimmed = args.trim();

    // Find entity name and body
    let brace_pos = match trimmed.find('{') {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: "Expected '{' after entity name".to_string(),
            };
        }
    };

    let entity_name = trimmed[..brace_pos].trim();
    if entity_name.is_empty() {
        return TerminalResponse::Error {
            message: "Entity name cannot be empty".to_string(),
        };
    }

    // Validate entity name (must be PascalCase identifier)
    if !entity_name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
        return TerminalResponse::Error {
            message: "Entity name must start with an uppercase letter".to_string(),
        };
    }

    // Find closing brace
    let close_brace = match trimmed.rfind('}') {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: "Expected '}' at end of entity definition".to_string(),
            };
        }
    };

    let body = trimmed[brace_pos + 1..close_brace].trim();

    // Parse fields
    let mut fields = Vec::new();
    let mut has_id_field = false;

    for field_def in body.split(',') {
        let field_def = field_def.trim();
        if field_def.is_empty() {
            continue;
        }

        let colon_pos = match field_def.find(':') {
            Some(pos) => pos,
            None => {
                return TerminalResponse::Error {
                    message: format!("Invalid field definition: '{}'. Expected 'name: Type'", field_def),
                };
            }
        };

        let field_name = field_def[..colon_pos].trim();
        let type_str = field_def[colon_pos + 1..].trim();

        // Check for optional marker (?)
        let (type_str, optional) = if type_str.ends_with('?') {
            (&type_str[..type_str.len() - 1], true)
        } else {
            (type_str, false)
        };

        let field_type = match parse_field_type(type_str) {
            Ok(ft) => {
                if optional {
                    make_optional(ft)
                } else {
                    ft
                }
            }
            Err(e) => {
                return TerminalResponse::Error {
                    message: format!("Invalid type '{}' for field '{}': {}", type_str, field_name, e),
                };
            }
        };

        if field_name == "id" {
            has_id_field = true;
        }

        let field = if optional {
            FieldDef::optional(field_name, field_type)
        } else {
            FieldDef::new(field_name, field_type)
        };
        fields.push(field);
    }

    // Ensure we have an id field
    if !has_id_field {
        // Add a Uuid id field automatically
        fields.insert(0, FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)));
    }

    // Create entity definition
    let entity = EntityDef::new(entity_name, "id").with_fields(fields);

    // Get or create schema bundle
    let catalog = session.database.catalog();
    let mut schema = catalog.current_schema().unwrap_or(None).unwrap_or_else(|| SchemaBundle::new(0));

    // Add/replace entity
    schema.entities.insert(entity_name.to_string(), entity);

    // Apply schema
    match catalog.apply_schema(schema) {
        Ok(version) => TerminalResponse::Output {
            text: format!("Entity '{}' created (schema version: {})\n", entity_name, version),
            format: "text".to_string(),
        },
        Err(e) => TerminalResponse::Error {
            message: format!("Failed to apply schema: {}", e),
        },
    }
}

/// Parse a type string into a FieldType
fn parse_field_type(type_str: &str) -> Result<FieldType, String> {
    let type_str = type_str.trim();

    // Check for array type
    if type_str.ends_with("[]") {
        let inner = &type_str[..type_str.len() - 2];
        let scalar = parse_scalar_type(inner)?;
        return Ok(FieldType::array_scalar(scalar));
    }

    // Parse scalar type
    let scalar = parse_scalar_type(type_str)?;
    Ok(FieldType::scalar(scalar))
}

/// Parse a scalar type string
fn parse_scalar_type(type_str: &str) -> Result<ScalarType, String> {
    match type_str.to_lowercase().as_str() {
        "string" | "str" | "text" => Ok(ScalarType::String),
        "int" | "int32" | "i32" | "integer" => Ok(ScalarType::Int32),
        "int64" | "i64" | "bigint" => Ok(ScalarType::Int64),
        "float" | "float32" | "f32" => Ok(ScalarType::Float32),
        "float64" | "f64" | "double" => Ok(ScalarType::Float64),
        "bool" | "boolean" => Ok(ScalarType::Bool),
        "uuid" | "id" => Ok(ScalarType::Uuid),
        "timestamp" | "datetime" | "date" => Ok(ScalarType::Timestamp),
        "bytes" | "binary" | "blob" => Ok(ScalarType::Bytes),
        _ => Err(format!("Unknown type: {}", type_str)),
    }
}

/// Convert a field type to its optional variant
fn make_optional(ft: FieldType) -> FieldType {
    match ft {
        FieldType::Scalar(s) => FieldType::OptionalScalar(s),
        FieldType::Enum { name, variants } => FieldType::OptionalEnum { name, variants },
        FieldType::Embedded { entity } => FieldType::OptionalEmbedded { entity },
        other => other, // Already optional or array
    }
}

/// Get command completions for a prefix
fn get_completions(_session: &Session, prefix: &str) -> TerminalResponse {
    let mut completions = Vec::new();

    // Dot commands
    let dot_commands = [
        ".help", ".clear", ".cls", ".schema", ".entity", ".session", ".exit", ".quit",
    ];

    // Entity methods (for future use)
    let _methods = [
        "findMany",
        "findFirst",
        "findUnique",
        "create",
        "update",
        "delete",
        "count",
    ];

    for cmd in dot_commands {
        if cmd.starts_with(prefix) {
            completions.push(cmd.to_string());
        }
    }

    // TODO: Add entity completions from schema

    TerminalResponse::Completions { items: completions }
}

const HELP_TEXT: &str = r#"
ORMDB Studio Terminal

Commands:
  .help, .h       Show this help message
  .clear, .cls    Clear the screen
  .schema         Show database schema
  .entity         Define a new entity (see below)
  .session        Show current session info
  .exit, .quit    Exit information

Schema Definition:
  .entity EntityName { field: Type, field2: Type? }

  Types: String, Int, Int64, Float, Float64, Bool, Uuid, Timestamp, Bytes
  Suffix with ? for optional fields
  Suffix with [] for arrays (e.g., String[])

  Examples:
    .entity User { name: String, email: String }
    .entity Post { title: String, content: String?, views: Int }
    .entity Tag { name: String, posts: String[] }

Query Language:
  Entity.findMany()              Fetch all records
  Entity.findFirst()             Fetch first record
  Entity.findUnique()            Fetch unique record
  Entity.create({ ... })         Create a record
  Entity.update({ ... })         Update records
  Entity.delete()                Delete records
  Entity.count()                 Count records

Filters:
  .where(field == value)         Equality filter
  .where(field > value)          Comparison filter
  .include(relation)             Include related data
  .orderBy(field, "asc")         Sort results
  .limit(n)                      Limit results
  .offset(n)                     Skip results

Examples:
  User.findMany()
  User.findMany().where(active == true).limit(10)
  Post.findMany().include(author).orderBy(createdAt, "desc")
  User.create({ name: "Alice", email: "alice@example.com" })

"#;
