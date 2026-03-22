use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use ormdb_core::catalog::{
    Cardinality, EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle,
};
use ormdb_core::query::{decode_entity, encode_entity};
use ormdb_core::storage::{key::current_timestamp, Record, VersionedKey};
use ormdb_lang::ast::{
    ComparisonOp, FilterCondition, Literal, MutationClause, MutationKind, ObjectLiteral,
    QueryClause, QueryKind, SortDirection, Statement,
};
use ormdb_proto::Value;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
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
        Ok(parsed) => execute_statement(session, parsed),
        Err(e) => TerminalResponse::Error {
            message: format!("Parse error: {:?}", e),
        },
    }
}

/// Execute a parsed statement
fn execute_statement(session: &Session, stmt: Statement) -> TerminalResponse {
    match stmt {
        Statement::Query(query) => {
            let entity_name = &query.entity.value;

            // Check if entity exists in schema
            let catalog = session.database.catalog();
            // Check if entity exists in schema
            match catalog.get_entity(entity_name) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return TerminalResponse::Error {
                        message: format!("Entity '{}' not found. Use .schema to see defined entities.", entity_name),
                    };
                }
                Err(e) => {
                    return TerminalResponse::Error {
                        message: format!("Catalog error: {}", e),
                    };
                }
            };

            let storage = session.database.storage();

            // Extract query clauses
            let where_clause = query.clauses.iter().find_map(|c| match c {
                QueryClause::Where(w) => Some(w),
                _ => None,
            });
            let order_by = query.clauses.iter().find_map(|c| match c {
                QueryClause::OrderBy(o) => Some(o),
                _ => None,
            });
            let limit = query.clauses.iter().find_map(|c| match c {
                QueryClause::Limit(l) => Some(l.value as usize),
                _ => None,
            });
            let offset = query.clauses.iter().find_map(|c| match c {
                QueryClause::Offset(o) => Some(o.value as usize),
                _ => None,
            });
            let include_clauses: Vec<_> = query.clauses.iter().filter_map(|c| match c {
                QueryClause::Include(i) => Some(i),
                _ => None,
            }).collect();

            // Get schema for include processing
            let schema = catalog.current_schema().ok().flatten();

            match query.kind {
                QueryKind::FindMany => {
                    // Scan and filter entities
                    let mut results: Vec<([u8; 16], u64, Record)> = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            if let Some(where_clause) = &where_clause {
                                let fields = decode_entity(&record.data).unwrap_or_default();
                                // Add id field for filtering
                                let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                                all_fields.extend(fields);
                                evaluate_filter(&all_fields, &where_clause.condition)
                            } else {
                                true
                            }
                        })
                        .collect();

                    // Apply ORDER BY
                    if let Some(order) = order_by {
                        sort_by_field(&mut results, &order.field.value, &order.direction);
                    }

                    // Apply OFFSET and LIMIT
                    let total_before_pagination = results.len();
                    let results: Vec<_> = results.into_iter()
                        .skip(offset.unwrap_or(0))
                        .take(limit.unwrap_or(usize::MAX))
                        .collect();

                    if results.is_empty() {
                        return TerminalResponse::Output {
                            text: format!("[] (0 records)\n"),
                            format: "text".to_string(),
                        };
                    }

                    let mut output = String::from("[\n");
                    for (entity_id, _version, record) in &results {
                        if let Ok(fields) = decode_entity(&record.data) {
                            output.push_str("  {\n");
                            output.push_str(&format!("    \"id\": \"{}\",\n", format_uuid(entity_id)));
                            for (name, value) in &fields {
                                output.push_str(&format!("    \"{}\": {},\n", name, format_value(value)));
                            }

                            // Process includes
                            for include in &include_clauses {
                                let relation_name = &include.path.value;
                                if let Some(ref schema) = schema {
                                    if let Some(relation) = schema.relations.get(relation_name) {
                                        // Check if this relation applies to our entity
                                        // Relation can be from our entity or to our entity
                                        let (target_entity, our_field, target_field, is_reverse) =
                                            if relation.from_entity == *entity_name {
                                                // Forward relation: our entity -> target
                                                (&relation.to_entity, &relation.from_field, &relation.to_field, false)
                                            } else if relation.to_entity == *entity_name {
                                                // Reverse relation: target -> our entity
                                                (&relation.from_entity, &relation.to_field, &relation.from_field, true)
                                            } else {
                                                // Relation doesn't involve this entity
                                                continue;
                                            };

                                        // Get the foreign key value from our record
                                        let our_value = if our_field == "id" {
                                            Some(Value::Uuid(*entity_id))
                                        } else {
                                            fields.iter().find(|(n, _)| n == our_field).map(|(_, v)| v.clone())
                                        };

                                        if let Some(fk_value) = our_value {
                                            // Find matching records in target entity
                                            let related: Vec<_> = storage
                                                .scan_entity_type(target_entity)
                                                .filter_map(|r| r.ok())
                                                .filter(|(target_id, _, target_record)| {
                                                    let target_fields = decode_entity(&target_record.data).unwrap_or_default();
                                                    let target_value = if target_field == "id" {
                                                        Some(Value::Uuid(*target_id))
                                                    } else {
                                                        target_fields.iter().find(|(n, _)| n == target_field).map(|(_, v)| v.clone())
                                                    };
                                                    target_value.as_ref().map(|tv| compare_values(tv, &fk_value) == Some(Ordering::Equal)).unwrap_or(false)
                                                })
                                                .collect();

                                            // Format the related records
                                            if is_reverse {
                                                // One-to-many: array of related entities
                                                output.push_str(&format!("    \"{}\": [\n", relation_name));
                                                for (rel_id, _, rel_record) in &related {
                                                    if let Ok(rel_fields) = decode_entity(&rel_record.data) {
                                                        output.push_str("      {\n");
                                                        output.push_str(&format!("        \"id\": \"{}\",\n", format_uuid(rel_id)));
                                                        for (n, v) in rel_fields {
                                                            output.push_str(&format!("        \"{}\": {},\n", n, format_value(&v)));
                                                        }
                                                        output.push_str("      },\n");
                                                    }
                                                }
                                                output.push_str("    ],\n");
                                            } else {
                                                // Forward relation: single entity or null
                                                if let Some((rel_id, _, rel_record)) = related.first() {
                                                    if let Ok(rel_fields) = decode_entity(&rel_record.data) {
                                                        output.push_str(&format!("    \"{}\": {{\n", relation_name));
                                                        output.push_str(&format!("      \"id\": \"{}\",\n", format_uuid(rel_id)));
                                                        for (n, v) in rel_fields {
                                                            output.push_str(&format!("      \"{}\": {},\n", n, format_value(&v)));
                                                        }
                                                        output.push_str("    },\n");
                                                    }
                                                } else {
                                                    output.push_str(&format!("    \"{}\": null,\n", relation_name));
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            output.push_str("  },\n");
                        }
                    }

                    // Show pagination info if applicable
                    if offset.is_some() || limit.is_some() {
                        output.push_str(&format!("] ({} of {} records)\n", results.len(), total_before_pagination));
                    } else {
                        output.push_str(&format!("] ({} records)\n", results.len()));
                    }

                    TerminalResponse::Output {
                        text: output,
                        format: "text".to_string(),
                    }
                }
                QueryKind::FindFirst => {
                    let result = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            if let Some(where_clause) = &where_clause {
                                let fields = decode_entity(&record.data).unwrap_or_default();
                                let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                                all_fields.extend(fields);
                                evaluate_filter(&all_fields, &where_clause.condition)
                            } else {
                                true
                            }
                        })
                        .next();

                    match result {
                        Some((entity_id, _version, record)) => {
                            if let Ok(fields) = decode_entity(&record.data) {
                                let mut output = String::from("{\n");
                                output.push_str(&format!("  \"id\": \"{}\",\n", format_uuid(&entity_id)));
                                for (name, value) in fields {
                                    output.push_str(&format!("  \"{}\": {},\n", name, format_value(&value)));
                                }
                                output.push_str("}\n");
                                TerminalResponse::Output {
                                    text: output,
                                    format: "text".to_string(),
                                }
                            } else {
                                TerminalResponse::Error {
                                    message: "Failed to decode record".to_string(),
                                }
                            }
                        }
                        None => TerminalResponse::Output {
                            text: "null\n".to_string(),
                            format: "text".to_string(),
                        },
                    }
                }
                QueryKind::FindUnique => {
                    // findUnique requires WHERE clause
                    let Some(where_clause) = &where_clause else {
                        return TerminalResponse::Error {
                            message: "findUnique requires a WHERE clause".to_string(),
                        };
                    };

                    let results: Vec<_> = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            let fields = decode_entity(&record.data).unwrap_or_default();
                            let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                            all_fields.extend(fields);
                            evaluate_filter(&all_fields, &where_clause.condition)
                        })
                        .collect();

                    match results.len() {
                        0 => TerminalResponse::Output {
                            text: "null\n".to_string(),
                            format: "text".to_string(),
                        },
                        1 => {
                            let (entity_id, _version, record) = &results[0];
                            if let Ok(fields) = decode_entity(&record.data) {
                                let mut output = String::from("{\n");
                                output.push_str(&format!("  \"id\": \"{}\",\n", format_uuid(entity_id)));
                                for (name, value) in fields {
                                    output.push_str(&format!("  \"{}\": {},\n", name, format_value(&value)));
                                }
                                output.push_str("}\n");
                                TerminalResponse::Output {
                                    text: output,
                                    format: "text".to_string(),
                                }
                            } else {
                                TerminalResponse::Error {
                                    message: "Failed to decode record".to_string(),
                                }
                            }
                        }
                        n => TerminalResponse::Error {
                            message: format!("findUnique expected 0 or 1 result, found {}", n),
                        },
                    }
                }
                QueryKind::Count => {
                    let count = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            if let Some(where_clause) = &where_clause {
                                let fields = decode_entity(&record.data).unwrap_or_default();
                                let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                                all_fields.extend(fields);
                                evaluate_filter(&all_fields, &where_clause.condition)
                            } else {
                                true
                            }
                        })
                        .count();

                    TerminalResponse::Output {
                        text: format!("{}\n", count),
                        format: "text".to_string(),
                    }
                }
            }
        }
        Statement::Mutation(mutation) => {
            let entity_name = &mutation.entity.value;

            // Check if entity exists in schema
            let catalog = session.database.catalog();
            // Check if entity exists in schema
            match catalog.get_entity(entity_name) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return TerminalResponse::Error {
                        message: format!("Entity '{}' not found. Use .entity to define it first.", entity_name),
                    };
                }
                Err(e) => {
                    return TerminalResponse::Error {
                        message: format!("Catalog error: {}", e),
                    };
                }
            };

            match mutation.kind {
                MutationKind::Create { data } => {
                    // Convert AST object literal to fields
                    let fields = object_literal_to_fields(&data);

                    // Generate a new UUID for the entity
                    let entity_id = uuid::Uuid::new_v4();
                    let entity_id_bytes: [u8; 16] = *entity_id.as_bytes();

                    // Encode the entity data
                    let encoded = match encode_entity(&fields) {
                        Ok(e) => e,
                        Err(e) => {
                            return TerminalResponse::Error {
                                message: format!("Failed to encode entity: {}", e),
                            };
                        }
                    };

                    // Create the record
                    let record = Record::new(encoded);
                    let version_ts = current_timestamp();
                    let key = VersionedKey::new(entity_id_bytes, version_ts);

                    // Store the record with type indexing
                    let storage = session.database.storage();
                    if let Err(e) = storage.put_typed(entity_name, key, record) {
                        return TerminalResponse::Error {
                            message: format!("Failed to store record: {}", e),
                        };
                    }

                    // Return the created entity
                    let mut output = String::from("Created:\n{\n");
                    output.push_str(&format!("  \"id\": \"{}\",\n", entity_id));
                    for (name, value) in &fields {
                        output.push_str(&format!("  \"{}\": {},\n", name, format_value(value)));
                    }
                    output.push_str("}\n");

                    TerminalResponse::Output {
                        text: output,
                        format: "text".to_string(),
                    }
                }
                MutationKind::Update { clauses } => {
                    // Extract WHERE and SET clauses
                    let where_clause = clauses.iter().find_map(|c| match c {
                        MutationClause::Where(w) => Some(w),
                        _ => None,
                    });
                    let set_clause = clauses.iter().find_map(|c| match c {
                        MutationClause::Set(s) => Some(s),
                        _ => None,
                    });

                    let Some(set_data) = set_clause else {
                        return TerminalResponse::Error {
                            message: "update requires a SET clause with data to update".to_string(),
                        };
                    };

                    let storage = session.database.storage();
                    let set_fields = object_literal_to_fields(set_data);

                    // Find matching records
                    let matches: Vec<_> = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            if let Some(where_clause) = &where_clause {
                                let fields = decode_entity(&record.data).unwrap_or_default();
                                let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                                all_fields.extend(fields);
                                evaluate_filter(&all_fields, &where_clause.condition)
                            } else {
                                true // Update all if no WHERE clause (dangerous but valid)
                            }
                        })
                        .collect();

                    if matches.is_empty() {
                        return TerminalResponse::Output {
                            text: "Updated: 0 records\n".to_string(),
                            format: "text".to_string(),
                        };
                    }

                    let mut updated_count = 0;
                    for (entity_id, _version, record) in matches {
                        // Decode existing fields
                        let mut fields = decode_entity(&record.data).unwrap_or_default();

                        // Merge SET fields into existing fields
                        for (name, value) in &set_fields {
                            // Update existing or add new field
                            if let Some(pos) = fields.iter().position(|(n, _)| n == name) {
                                fields[pos] = (name.clone(), value.clone());
                            } else {
                                fields.push((name.clone(), value.clone()));
                            }
                        }

                        // Encode the updated entity
                        let encoded = match encode_entity(&fields) {
                            Ok(e) => e,
                            Err(e) => {
                                return TerminalResponse::Error {
                                    message: format!("Failed to encode entity: {}", e),
                                };
                            }
                        };

                        // Create new version
                        let new_record = Record::new(encoded);
                        let version_ts = current_timestamp();
                        let key = VersionedKey::new(entity_id, version_ts);

                        if let Err(e) = storage.put_typed(entity_name, key, new_record) {
                            return TerminalResponse::Error {
                                message: format!("Failed to update record: {}", e),
                            };
                        }
                        updated_count += 1;
                    }

                    TerminalResponse::Output {
                        text: format!("Updated: {} record{}\n", updated_count, if updated_count == 1 { "" } else { "s" }),
                        format: "text".to_string(),
                    }
                }
                MutationKind::Delete { clauses } => {
                    // Extract WHERE clause
                    let where_clause = clauses.iter().find_map(|c| match c {
                        MutationClause::Where(w) => Some(w),
                        _ => None,
                    });

                    let storage = session.database.storage();

                    // Find matching records
                    let matches: Vec<_> = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .filter(|(entity_id, _, record)| {
                            if let Some(where_clause) = &where_clause {
                                let fields = decode_entity(&record.data).unwrap_or_default();
                                let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                                all_fields.extend(fields);
                                evaluate_filter(&all_fields, &where_clause.condition)
                            } else {
                                true // Delete all if no WHERE clause (dangerous but valid)
                            }
                        })
                        .collect();

                    if matches.is_empty() {
                        return TerminalResponse::Output {
                            text: "Deleted: 0 records\n".to_string(),
                            format: "text".to_string(),
                        };
                    }

                    let mut deleted_count = 0;
                    for (entity_id, _version, _record) in matches {
                        if let Err(e) = storage.delete_typed(entity_name, &entity_id) {
                            return TerminalResponse::Error {
                                message: format!("Failed to delete record: {}", e),
                            };
                        }
                        deleted_count += 1;
                    }

                    TerminalResponse::Output {
                        text: format!("Deleted: {} record{}\n", deleted_count, if deleted_count == 1 { "" } else { "s" }),
                        format: "text".to_string(),
                    }
                }
                MutationKind::Upsert { clauses } => {
                    // Extract WHERE and SET clauses
                    let where_clause = clauses.iter().find_map(|c| match c {
                        MutationClause::Where(w) => Some(w),
                        _ => None,
                    });
                    let set_clause = clauses.iter().find_map(|c| match c {
                        MutationClause::Set(s) => Some(s),
                        _ => None,
                    });

                    let Some(set_data) = set_clause else {
                        return TerminalResponse::Error {
                            message: "upsert requires a SET clause with data".to_string(),
                        };
                    };

                    let Some(where_clause) = &where_clause else {
                        return TerminalResponse::Error {
                            message: "upsert requires a WHERE clause to find existing record".to_string(),
                        };
                    };

                    let storage = session.database.storage();
                    let set_fields = object_literal_to_fields(set_data);

                    // Find existing matching record
                    let existing: Option<([u8; 16], u64, Record)> = storage
                        .scan_entity_type(entity_name)
                        .filter_map(|r| r.ok())
                        .find(|(entity_id, _, record)| {
                            let fields = decode_entity(&record.data).unwrap_or_default();
                            let mut all_fields = vec![("id".to_string(), Value::Uuid(*entity_id))];
                            all_fields.extend(fields);
                            evaluate_filter(&all_fields, &where_clause.condition)
                        });

                    match existing {
                        Some((entity_id, _version, record)) => {
                            // Update existing record
                            let mut fields = decode_entity(&record.data).unwrap_or_default();
                            for (name, value) in &set_fields {
                                if let Some(pos) = fields.iter().position(|(n, _)| n == name) {
                                    fields[pos] = (name.clone(), value.clone());
                                } else {
                                    fields.push((name.clone(), value.clone()));
                                }
                            }

                            let encoded = match encode_entity(&fields) {
                                Ok(e) => e,
                                Err(e) => {
                                    return TerminalResponse::Error {
                                        message: format!("Failed to encode entity: {}", e),
                                    };
                                }
                            };

                            let new_record = Record::new(encoded);
                            let version_ts = current_timestamp();
                            let key = VersionedKey::new(entity_id, version_ts);

                            if let Err(e) = storage.put_typed(entity_name, key, new_record) {
                                return TerminalResponse::Error {
                                    message: format!("Failed to update record: {}", e),
                                };
                            }

                            let mut output = String::from("Updated (upsert):\n{\n");
                            output.push_str(&format!("  \"id\": \"{}\",\n", format_uuid(&entity_id)));
                            for (name, value) in &fields {
                                output.push_str(&format!("  \"{}\": {},\n", name, format_value(value)));
                            }
                            output.push_str("}\n");

                            TerminalResponse::Output {
                                text: output,
                                format: "text".to_string(),
                            }
                        }
                        None => {
                            // Create new record
                            let entity_id = uuid::Uuid::new_v4();
                            let entity_id_bytes: [u8; 16] = *entity_id.as_bytes();

                            let encoded = match encode_entity(&set_fields) {
                                Ok(e) => e,
                                Err(e) => {
                                    return TerminalResponse::Error {
                                        message: format!("Failed to encode entity: {}", e),
                                    };
                                }
                            };

                            let record = Record::new(encoded);
                            let version_ts = current_timestamp();
                            let key = VersionedKey::new(entity_id_bytes, version_ts);

                            if let Err(e) = storage.put_typed(entity_name, key, record) {
                                return TerminalResponse::Error {
                                    message: format!("Failed to create record: {}", e),
                                };
                            }

                            let mut output = String::from("Created (upsert):\n{\n");
                            output.push_str(&format!("  \"id\": \"{}\",\n", entity_id));
                            for (name, value) in &set_fields {
                                output.push_str(&format!("  \"{}\": {},\n", name, format_value(value)));
                            }
                            output.push_str("}\n");

                            TerminalResponse::Output {
                                text: output,
                                format: "text".to_string(),
                            }
                        }
                    }
                }
            }
        }
        Statement::SchemaCommand(_) => {
            TerminalResponse::Error {
                message: "Use dot commands for schema operations (e.g., .schema, .entity)".to_string(),
            }
        }
    }
}

/// Convert an ObjectLiteral to a list of field name/value pairs
fn object_literal_to_fields(obj: &ObjectLiteral) -> Vec<(String, Value)> {
    obj.fields
        .iter()
        .map(|field| {
            let value = literal_to_value(&field.value.value);
            (field.name.value.clone(), value)
        })
        .collect()
}

/// Convert an AST Literal to a Value
fn literal_to_value(lit: &Literal) -> Value {
    match lit {
        Literal::Null => Value::Null,
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Int(i) => Value::Int64(*i),
        Literal::Float(f) => Value::Float64(*f),
        Literal::String(s) => Value::String(s.clone()),
    }
}

/// Format a UUID from bytes
fn format_uuid(bytes: &[u8; 16]) -> String {
    uuid::Uuid::from_bytes(*bytes).to_string()
}

/// Format a Value for display
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int32(i) => i.to_string(),
        Value::Int64(i) => i.to_string(),
        Value::Float32(f) => f.to_string(),
        Value::Float64(f) => f.to_string(),
        Value::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
        Value::Uuid(u) => format!("\"{}\"", uuid::Uuid::from_bytes(*u)),
        Value::Timestamp(t) => format!("{}", t),
        Value::BoolArray(arr) => format!("{:?}", arr),
        Value::Int32Array(arr) => format!("{:?}", arr),
        Value::Int64Array(arr) => format!("{:?}", arr),
        Value::Float32Array(arr) => format!("{:?}", arr),
        Value::Float64Array(arr) => format!("{:?}", arr),
        Value::StringArray(arr) => format!("{:?}", arr),
        Value::UuidArray(arr) => {
            let uuids: Vec<_> = arr.iter().map(|u| uuid::Uuid::from_bytes(*u).to_string()).collect();
            format!("{:?}", uuids)
        }
    }
}

// ============================================================================
// Filter Evaluation
// ============================================================================

/// Evaluate a filter condition against a record's fields
fn evaluate_filter(record_fields: &[(String, Value)], condition: &FilterCondition) -> bool {
    match condition {
        FilterCondition::Comparison { field, op, value } => {
            let field_name = &field.value;
            let record_value = record_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);

            let Some(record_value) = record_value else {
                // Field not found in record - only matches if comparing to null
                return matches!(op, ComparisonOp::Eq) && matches!(value.value, Literal::Null);
            };

            let filter_value = literal_to_value(&value.value);
            evaluate_comparison(record_value, op, &filter_value)
        }
        FilterCondition::In { field, values, negated } => {
            let field_name = &field.value;
            let record_value = record_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);

            let Some(record_value) = record_value else {
                return *negated; // NOT IN matches when field is missing
            };

            let matches = values.iter().any(|v| {
                let filter_value = literal_to_value(&v.value);
                compare_values(record_value, &filter_value) == Some(Ordering::Equal)
            });

            if *negated { !matches } else { matches }
        }
        FilterCondition::IsNull { field, negated } => {
            let field_name = &field.value;
            let record_value = record_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);

            let is_null = record_value.map(|v| matches!(v, Value::Null)).unwrap_or(true);
            if *negated { !is_null } else { is_null }
        }
        FilterCondition::Like { field, pattern, negated } => {
            let field_name = &field.value;
            let record_value = record_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);

            let Some(Value::String(s)) = record_value else {
                return *negated;
            };

            let matches = matches_like_pattern(s, &pattern.value);
            if *negated { !matches } else { matches }
        }
        FilterCondition::And(conditions) => {
            conditions.iter().all(|c| evaluate_filter(record_fields, c))
        }
        FilterCondition::Or(conditions) => {
            conditions.iter().any(|c| evaluate_filter(record_fields, c))
        }
    }
}

/// Evaluate a comparison operation between two values
fn evaluate_comparison(left: &Value, op: &ComparisonOp, right: &Value) -> bool {
    match op {
        ComparisonOp::Eq => compare_values(left, right) == Some(Ordering::Equal),
        ComparisonOp::Ne => compare_values(left, right) != Some(Ordering::Equal),
        ComparisonOp::Lt => compare_values(left, right) == Some(Ordering::Less),
        ComparisonOp::Le => matches!(compare_values(left, right), Some(Ordering::Less | Ordering::Equal)),
        ComparisonOp::Gt => compare_values(left, right) == Some(Ordering::Greater),
        ComparisonOp::Ge => matches!(compare_values(left, right), Some(Ordering::Greater | Ordering::Equal)),
    }
}

/// Compare two Values, returning their ordering if comparable
fn compare_values(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        // Null comparisons
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        (Value::Null, _) => Some(Ordering::Less),
        (_, Value::Null) => Some(Ordering::Greater),

        // Boolean comparisons
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),

        // Integer comparisons (cross-type)
        (Value::Int32(a), Value::Int32(b)) => Some(a.cmp(b)),
        (Value::Int64(a), Value::Int64(b)) => Some(a.cmp(b)),
        (Value::Int32(a), Value::Int64(b)) => Some((*a as i64).cmp(b)),
        (Value::Int64(a), Value::Int32(b)) => Some(a.cmp(&(*b as i64))),

        // Float comparisons (cross-type)
        (Value::Float32(a), Value::Float32(b)) => a.partial_cmp(b),
        (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b),
        (Value::Float32(a), Value::Float64(b)) => (*a as f64).partial_cmp(b),
        (Value::Float64(a), Value::Float32(b)) => a.partial_cmp(&(*b as f64)),

        // Int to float comparisons
        (Value::Int32(a), Value::Float64(b)) => (*a as f64).partial_cmp(b),
        (Value::Int64(a), Value::Float64(b)) => (*a as f64).partial_cmp(b),
        (Value::Float64(a), Value::Int32(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Float64(a), Value::Int64(b)) => a.partial_cmp(&(*b as f64)),

        // String comparisons
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),

        // UUID comparisons
        (Value::Uuid(a), Value::Uuid(b)) => Some(a.cmp(b)),

        // Timestamp comparisons
        (Value::Timestamp(a), Value::Timestamp(b)) => Some(a.cmp(b)),

        // Bytes comparisons
        (Value::Bytes(a), Value::Bytes(b)) => Some(a.cmp(b)),

        // Incompatible types
        _ => None,
    }
}

/// Match a string against a SQL LIKE pattern
/// % matches any sequence of characters
/// _ matches any single character
fn matches_like_pattern(value: &str, pattern: &str) -> bool {
    // Simple regex-like matching without full regex dependency
    if pattern == "%" {
        return true;
    }

    if !pattern.contains('%') && !pattern.contains('_') {
        return value == pattern;
    }

    // Handle common patterns
    if pattern.starts_with('%') && pattern.ends_with('%') {
        let inner = &pattern[1..pattern.len()-1];
        if !inner.contains('%') && !inner.contains('_') {
            return value.contains(inner);
        }
    }

    if pattern.starts_with('%') && !pattern[1..].contains('%') {
        let suffix = &pattern[1..];
        return value.ends_with(suffix);
    }

    if pattern.ends_with('%') && !pattern[..pattern.len()-1].contains('%') {
        let prefix = &pattern[..pattern.len()-1];
        return value.starts_with(prefix);
    }

    // For complex patterns, do character-by-character matching
    matches_like_recursive(value.chars().collect::<Vec<_>>().as_slice(),
                          pattern.chars().collect::<Vec<_>>().as_slice())
}

fn matches_like_recursive(value: &[char], pattern: &[char]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }

    match pattern[0] {
        '%' => {
            // % matches zero or more characters
            for i in 0..=value.len() {
                if matches_like_recursive(&value[i..], &pattern[1..]) {
                    return true;
                }
            }
            false
        }
        '_' => {
            // _ matches exactly one character
            !value.is_empty() && matches_like_recursive(&value[1..], &pattern[1..])
        }
        c => {
            // Regular character must match exactly
            !value.is_empty() && value[0] == c && matches_like_recursive(&value[1..], &pattern[1..])
        }
    }
}

/// Sort records by a field
fn sort_by_field(
    records: &mut [([u8; 16], u64, Record)],
    field_name: &str,
    direction: &SortDirection,
) {
    records.sort_by(|(_, _, a), (_, _, b)| {
        let a_fields = decode_entity(&a.data).unwrap_or_default();
        let b_fields = decode_entity(&b.data).unwrap_or_default();

        let a_val = a_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);
        let b_val = b_fields.iter().find(|(n, _)| n == field_name).map(|(_, v)| v);

        let ordering = match (a_val, b_val) {
            (Some(a), Some(b)) => compare_values(a, b).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };

        match direction {
            SortDirection::Asc => ordering,
            SortDirection::Desc => ordering.reverse(),
        }
    });
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
        ".relation" => {
            if args.is_empty() {
                return TerminalResponse::Error {
                    message: "Usage: .relation name: From.field -> To.field [one-to-one|one-to-many]".to_string(),
                };
            }
            handle_relation_command(session, args)
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

/// Handle the .relation command for defining relations
/// Syntax: .relation name: From.field -> To.field [one-to-one|one-to-many]
fn handle_relation_command(session: &Session, args: &str) -> TerminalResponse {
    let trimmed = args.trim();

    // Parse: name: From.field -> To.field [cardinality]
    let colon_pos = match trimmed.find(':') {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: "Expected ':' after relation name. Usage: .relation name: From.field -> To.field".to_string(),
            };
        }
    };

    let relation_name = trimmed[..colon_pos].trim();
    if relation_name.is_empty() {
        return TerminalResponse::Error {
            message: "Relation name cannot be empty".to_string(),
        };
    }

    let rest = trimmed[colon_pos + 1..].trim();

    // Find the arrow
    let arrow_pos = match rest.find("->") {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: "Expected '->' between source and target. Usage: .relation name: From.field -> To.field".to_string(),
            };
        }
    };

    let from_part = rest[..arrow_pos].trim();
    let after_arrow = rest[arrow_pos + 2..].trim();

    // Parse From.field
    let from_dot = match from_part.find('.') {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: format!("Expected 'Entity.field' format, got: '{}'", from_part),
            };
        }
    };
    let from_entity = from_part[..from_dot].trim();
    let from_field = from_part[from_dot + 1..].trim();

    // Parse To.field and optional cardinality
    let parts: Vec<&str> = after_arrow.split_whitespace().collect();
    if parts.is_empty() {
        return TerminalResponse::Error {
            message: "Expected target entity after '->'".to_string(),
        };
    }

    let to_part = parts[0];
    let cardinality_str = parts.get(1).copied();

    let to_dot = match to_part.find('.') {
        Some(pos) => pos,
        None => {
            return TerminalResponse::Error {
                message: format!("Expected 'Entity.field' format, got: '{}'", to_part),
            };
        }
    };
    let to_entity = to_part[..to_dot].trim();
    let to_field = to_part[to_dot + 1..].trim();

    // Parse cardinality (default: one-to-many)
    let cardinality = match cardinality_str {
        Some("one-to-one") | Some("1:1") => Cardinality::OneToOne,
        Some("one-to-many") | Some("1:n") | Some("1:*") => Cardinality::OneToMany,
        Some("many-to-many") | Some("n:n") | Some("*:*") => Cardinality::ManyToMany,
        Some(other) => {
            return TerminalResponse::Error {
                message: format!("Unknown cardinality: '{}'. Use: one-to-one, one-to-many, or many-to-many", other),
            };
        }
        None => Cardinality::OneToMany, // Default
    };

    // Verify entities exist
    let catalog = session.database.catalog();
    let schema = match catalog.current_schema() {
        Ok(Some(s)) => s,
        Ok(None) => {
            return TerminalResponse::Error {
                message: "No schema defined. Create entities first with .entity".to_string(),
            };
        }
        Err(e) => {
            return TerminalResponse::Error {
                message: format!("Failed to get schema: {}", e),
            };
        }
    };

    if !schema.entities.contains_key(from_entity) {
        return TerminalResponse::Error {
            message: format!("Entity '{}' not found. Define it first with .entity", from_entity),
        };
    }
    if !schema.entities.contains_key(to_entity) {
        return TerminalResponse::Error {
            message: format!("Entity '{}' not found. Define it first with .entity", to_entity),
        };
    }

    // Create the relation
    let relation = match cardinality {
        Cardinality::OneToOne => {
            RelationDef::one_to_one(relation_name, from_entity, from_field, to_entity, to_field)
        }
        Cardinality::OneToMany => {
            RelationDef::one_to_many(relation_name, from_entity, from_field, to_entity, to_field)
        }
        Cardinality::ManyToMany => {
            // For many-to-many, we'd need an edge entity
            return TerminalResponse::Error {
                message: "Many-to-many relations require an edge entity (not yet supported in this command)".to_string(),
            };
        }
    };

    // Get fresh schema and add relation
    let mut schema = catalog.current_schema().unwrap_or(None).unwrap_or_else(|| SchemaBundle::new(0));
    schema.relations.insert(relation_name.to_string(), relation);

    // Apply schema
    match catalog.apply_schema(schema) {
        Ok(version) => TerminalResponse::Output {
            text: format!(
                "Relation '{}' created: {}.{} -> {}.{} ({:?}) (schema version: {})\n",
                relation_name, from_entity, from_field, to_entity, to_field, cardinality, version
            ),
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
        ".help", ".clear", ".cls", ".schema", ".entity", ".relation", ".session", ".exit", ".quit",
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
  .relation       Define a relation between entities
  .session        Show current session info
  .exit, .quit    Exit information

Schema Definition:
  .entity EntityName { field: Type, field2: Type? }

  Types: String, Int, Int64, Float, Float64, Bool, Uuid, Timestamp, Bytes
  Suffix with ? for optional fields
  Suffix with [] for arrays (e.g., String[])

  Examples:
    .entity User { name: String, email: String }
    .entity Post { title: String, content: String?, author_id: Uuid }
    .entity Tag { name: String, posts: String[] }

Relation Definition:
  .relation name: From.field -> To.field [cardinality]

  Cardinality: one-to-one, one-to-many (default)

  Examples:
    .relation posts: Post.author_id -> User.id
    .relation profile: Profile.user_id -> User.id one-to-one

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
