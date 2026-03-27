use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use ormdb_core::catalog::FieldType;
use ormdb_core::query::QueryExecutor;
use ormdb_lang::{parse_and_compile, CompiledStatement, CompiledSchemaCommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

use crate::error::{Result, StudioError};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Session management
        .route("/api/session", post(create_session))
        .route("/api/session/:id", get(get_session).delete(delete_session))
        .route("/api/sessions", get(list_sessions))
        // Query execution
        .route("/api/session/:id/query", post(execute_query))
        .route("/api/session/:id/query/raw", post(execute_raw_query))
        .route("/api/session/:id/mutate", post(execute_mutation))
        // Schema
        .route("/api/session/:id/schema", get(get_schema))
        .route("/api/session/:id/schema/entities", get(list_entities))
        .route("/api/session/:id/schema/apply", post(apply_schema))
        // Metrics and monitoring
        .route("/api/session/:id/metrics", get(get_metrics))
        .route("/api/session/:id/explain", post(explain_query))
        .route("/api/session/:id/replication", get(get_replication_status))
        // Storage management
        .route("/api/session/:id/compact", post(compact_storage))
}

// ============================================================================
// Session Management
// ============================================================================

#[derive(Serialize)]
struct SessionResponse {
    success: bool,
    session: SessionInfo,
}

#[derive(Serialize)]
struct SessionInfo {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    age_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_activity_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    demo: Option<bool>,
}

#[derive(Deserialize, Default)]
struct CreateSessionParams {
    #[serde(default)]
    demo: bool,
}

async fn create_session(
    State(state): State<AppState>,
    Query(params): Query<CreateSessionParams>,
) -> Result<Json<SessionResponse>> {
    let session = if params.demo {
        state.sessions.create_demo_session()?
    } else {
        state.sessions.create_session()?
    };

    Ok(Json(SessionResponse {
        success: true,
        session: SessionInfo {
            id: session.id.clone(),
            age_secs: Some(0),
            last_activity_secs: Some(0),
            demo: if params.demo { Some(true) } else { None },
        },
    }))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>> {
    let session = state
        .sessions
        .get_session(&id)
        .ok_or_else(|| StudioError::SessionNotFound(id))?;

    Ok(Json(SessionResponse {
        success: true,
        session: SessionInfo {
            id: session.id.clone(),
            age_secs: Some(session.age().as_secs()),
            last_activity_secs: None,
            demo: None,
        },
    }))
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let deleted = state.sessions.delete_session(&id);

    if deleted {
        Ok(Json(json!({
            "success": true,
            "message": "Session deleted"
        })))
    } else {
        Err(StudioError::SessionNotFound(id))
    }
}

async fn list_sessions(State(state): State<AppState>) -> Json<Value> {
    let sessions = state.sessions.list_sessions();

    Json(json!({
        "success": true,
        "sessions": sessions,
        "count": sessions.len(),
        "max": state.config.max_sessions,
    }))
}

// ============================================================================
// Query Execution
// ============================================================================

#[derive(Deserialize)]
struct QueryRequest {
    entity: String,
    #[serde(default)]
    filter: Option<Value>,
    #[serde(default)]
    include: Option<Vec<String>>,
    #[serde(default)]
    order_by: Option<Vec<OrderBy>>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Deserialize)]
struct OrderBy {
    field: String,
    #[serde(default = "default_direction")]
    direction: String,
}

fn default_direction() -> String {
    "asc".to_string()
}

#[derive(Deserialize)]
struct RawQueryRequest {
    query: String,
}

#[derive(Deserialize)]
struct MutationRequest {
    #[serde(rename = "type")]
    mutation_type: String, // "create", "update", "delete"
    entity: String,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    filter: Option<Value>,
    #[serde(default)]
    id: Option<String>,
}

async fn execute_query(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Value>> {
    let _session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // TODO: Convert QueryRequest to GraphQuery and execute
    // For now, return a placeholder
    Ok(Json(json!({
        "success": true,
        "data": {
            "entity": request.entity,
            "rows": [],
            "count": 0,
        },
        "message": "Query execution not yet implemented"
    })))
}

async fn execute_raw_query(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<RawQueryRequest>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id.clone()))?;

    // Parse and compile the query using ormdb-lang
    let compiled = parse_and_compile(&request.query)
        .map_err(|e| StudioError::ParseError(e.format_with_source(&request.query)))?;

    match compiled {
        CompiledStatement::Query(graph_query) => {
            // Execute the query with timing
            let start = Instant::now();
            let executor = QueryExecutor::with_metrics(
                session.database.storage(),
                session.database.catalog(),
                session.database.metrics().clone(),
            );

            let result = executor.execute(&graph_query)
                .map_err(|e| StudioError::Database(e.to_string()))?;
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

            // Convert QueryResult to JSON
            let data = query_result_to_json(&result);

            Ok(Json(json!({
                "success": true,
                "data": data,
                "has_more": result.has_more,
                "total_entities": result.total_entities(),
                "total_edges": result.total_edges(),
                "duration_ms": duration_ms
            })))
        }
        CompiledStatement::Aggregate(agg_query) => {
            // Execute aggregate query with timing
            let start = Instant::now();
            let storage = session.database.storage();
            let executor = ormdb_core::query::AggregateExecutor::new(
                storage,
                storage.columnar(),
            );

            let result = executor.execute(&agg_query)
                .map_err(|e| StudioError::Database(e.to_string()))?;
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

            // Convert aggregate result to JSON
            let values: Vec<Value> = result.values.iter().map(|v| {
                json!({
                    "function": format!("{:?}", v.function),
                    "field": v.field,
                    "value": value_to_json(&v.value)
                })
            }).collect();

            Ok(Json(json!({
                "success": true,
                "data": {
                    "entity": result.entity,
                    "aggregations": values
                },
                "duration_ms": duration_ms
            })))
        }
        CompiledStatement::Mutation(mutation) => {
            // For now, mutations require more infrastructure (transaction handling)
            // Return a helpful message
            Ok(Json(json!({
                "success": false,
                "error": {
                    "message": "Mutations are not yet supported in the Studio query interface. Use the terminal instead.",
                    "mutation_type": format!("{:?}", mutation)
                }
            })))
        }
        CompiledStatement::SchemaCommand(cmd) => {
            // Handle schema commands
            match cmd {
                CompiledSchemaCommand::ListEntities => {
                    let catalog = session.database.catalog();
                    let entities = catalog.list_entities()
                        .map_err(|e| StudioError::Database(e.to_string()))?;

                    Ok(Json(json!({
                        "success": true,
                        "data": {
                            "command": "list_entities",
                            "entities": entities
                        }
                    })))
                }
                CompiledSchemaCommand::DescribeEntity(entity_name) => {
                    let catalog = session.database.catalog();
                    let schema = catalog.current_schema()
                        .map_err(|e| StudioError::Database(e.to_string()))?;

                    if let Some(bundle) = schema {
                        if let Some(entity) = bundle.entities.get(&entity_name) {
                            let fields: Vec<Value> = entity.fields.iter().map(|f| {
                                json!({
                                    "name": f.name,
                                    "type": format_field_type(&f.field_type),
                                    "required": f.required,
                                    "primary_key": f.name == entity.identity_field
                                })
                            }).collect();

                            Ok(Json(json!({
                                "success": true,
                                "data": {
                                    "command": "describe_entity",
                                    "entity": entity_name,
                                    "fields": fields,
                                    "identity_field": entity.identity_field
                                }
                            })))
                        } else {
                            Err(StudioError::Database(format!("Entity '{}' not found", entity_name)))
                        }
                    } else {
                        Err(StudioError::Database("No schema defined".to_string()))
                    }
                }
                CompiledSchemaCommand::DescribeRelation(relation_name) => {
                    let catalog = session.database.catalog();
                    let schema = catalog.current_schema()
                        .map_err(|e| StudioError::Database(e.to_string()))?;

                    if let Some(bundle) = schema {
                        if let Some(relation) = bundle.relations.get(&relation_name) {
                            Ok(Json(json!({
                                "success": true,
                                "data": {
                                    "command": "describe_relation",
                                    "name": relation_name,
                                    "from_entity": relation.from_entity,
                                    "from_field": relation.from_field,
                                    "to_entity": relation.to_entity,
                                    "to_field": relation.to_field,
                                    "cardinality": format!("{:?}", relation.cardinality)
                                }
                            })))
                        } else {
                            Err(StudioError::Database(format!("Relation '{}' not found", relation_name)))
                        }
                    } else {
                        Err(StudioError::Database("No schema defined".to_string()))
                    }
                }
                CompiledSchemaCommand::Help => {
                    Ok(Json(json!({
                        "success": true,
                        "data": {
                            "command": "help",
                            "syntax": [
                                "Entity.findMany() - Query all entities",
                                "Entity.findMany().where(field == value) - Query with filter",
                                "Entity.findMany().include(relation) - Include related entities",
                                "Entity.findMany().orderBy(field.asc) - Order results",
                                "Entity.findMany().limit(10).offset(0) - Paginate results",
                                "Entity.count() - Count entities",
                                "Entity.count().where(field == value) - Count with filter",
                                ".schema - List all entities",
                                ".schema Entity - Describe an entity",
                                ".describe relation - Describe a relation",
                                ".help - Show this help"
                            ]
                        }
                    })))
                }
            }
        }
    }
}

/// Convert a QueryResult to JSON for the API response.
fn query_result_to_json(result: &ormdb_proto::QueryResult) -> Value {
    let mut entities: Vec<Value> = Vec::new();

    for block in &result.entities {
        // Convert columnar data to row-oriented JSON
        let rows: Vec<Value> = (0..block.ids.len())
            .map(|i| {
                let mut row = serde_json::Map::new();

                // Add id as hex string
                let id = &block.ids[i];
                row.insert("id".to_string(), json!(format_uuid(id)));

                // Add all field values
                for col in &block.columns {
                    row.insert(col.name.clone(), value_to_json(&col.values[i]));
                }

                Value::Object(row)
            })
            .collect();

        entities.push(json!({
            "entity": block.entity,
            "rows": rows,
            "count": block.len()
        }));
    }

    // Convert edges
    let edges: Vec<Value> = result.edges.iter().map(|block| {
        let edge_list: Vec<Value> = block.edges.iter().map(|e| {
            json!({
                "from": format_uuid(&e.from_id),
                "to": format_uuid(&e.to_id)
            })
        }).collect();

        json!({
            "relation": block.relation,
            "edges": edge_list,
            "count": block.len()
        })
    }).collect();

    json!({
        "entities": entities,
        "edges": edges
    })
}

/// Convert a proto Value to JSON.
fn value_to_json(v: &ormdb_proto::Value) -> Value {
    use ormdb_proto::Value as ProtoValue;
    match v {
        ProtoValue::Null => Value::Null,
        ProtoValue::Bool(b) => json!(b),
        ProtoValue::Int32(i) => json!(i),
        ProtoValue::Int64(i) => json!(i),
        ProtoValue::Float32(f) => json!(f),
        ProtoValue::Float64(f) => json!(f),
        ProtoValue::String(s) => json!(s),
        ProtoValue::Bytes(b) => {
            // Encode bytes as hex string
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            json!(hex)
        }
        ProtoValue::Timestamp(ts) => json!(ts),
        ProtoValue::Uuid(id) => json!(format_uuid(id)),
        // Array types
        ProtoValue::BoolArray(arr) => json!(arr),
        ProtoValue::Int32Array(arr) => json!(arr),
        ProtoValue::Int64Array(arr) => json!(arr),
        ProtoValue::Float32Array(arr) => json!(arr),
        ProtoValue::Float64Array(arr) => json!(arr),
        ProtoValue::StringArray(arr) => json!(arr),
        ProtoValue::UuidArray(arr) => {
            let uuids: Vec<String> = arr.iter().map(|id| format_uuid(id)).collect();
            json!(uuids)
        }
    }
}

/// Format a UUID as a hyphenated string.
fn format_uuid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

async fn execute_mutation(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<MutationRequest>,
) -> Result<Json<Value>> {
    let _session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // TODO: Execute mutation
    Ok(Json(json!({
        "success": true,
        "data": {
            "type": request.mutation_type,
            "entity": request.entity,
            "affected": 0,
        },
        "message": "Mutation execution not yet implemented"
    })))
}

// ============================================================================
// Schema
// ============================================================================

async fn get_schema(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    let catalog = session.database.catalog();
    let schema = catalog
        .current_schema()
        .map_err(|e| StudioError::Database(e.to_string()))?;

    match schema {
        Some(bundle) => {
            let entities: Vec<Value> = bundle
                .entities
                .iter()
                .map(|(name, entity)| {
                    let fields: Vec<Value> = entity
                        .fields
                        .iter()
                        .map(|f| {
                            json!({
                                "name": f.name,
                                "type": format_field_type(&f.field_type),
                                "nullable": !f.required,
                                "primaryKey": f.name == entity.identity_field,
                            })
                        })
                        .collect();

                    json!({
                        "name": name,
                        "fields": fields,
                        "relations": [],
                    })
                })
                .collect();

            Ok(Json(json!({
                "success": true,
                "schema": {
                    "version": bundle.version,
                    "entities": entities,
                }
            })))
        }
        None => Ok(Json(json!({
            "success": true,
            "schema": {
                "version": 0,
                "entities": [],
            }
        }))),
    }
}

fn format_field_type(ft: &FieldType) -> String {
    use ormdb_core::catalog::ScalarType;

    match ft {
        FieldType::Scalar(s) | FieldType::OptionalScalar(s) => match s {
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
        },
        FieldType::ArrayScalar(s) => format!("{}[]", format_field_type(&FieldType::Scalar(s.clone()))),
        FieldType::Enum { name, .. } | FieldType::OptionalEnum { name, .. } => name.clone(),
        FieldType::Embedded { entity } | FieldType::OptionalEmbedded { entity } => entity.clone(),
        FieldType::ArrayEmbedded { entity } => format!("{}[]", entity),
    }
}

async fn list_entities(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    let catalog = session.database.catalog();
    let entities = catalog
        .list_entities()
        .map_err(|e| StudioError::Database(e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "entities": entities
    })))
}

// ============================================================================
// Schema Apply
// ============================================================================

#[derive(Deserialize)]
struct ApplySchemaRequest {
    schema: String,
}

async fn apply_schema(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<ApplySchemaRequest>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // Parse the schema using ormdb-lang
    let parsed = ormdb_lang::parse(&request.schema)
        .map_err(|e| StudioError::ParseError(format!("{:?}", e)))?;

    // TODO: Compile and apply the schema to the catalog
    // For now, just return success with the parsed result
    Ok(Json(json!({
        "success": true,
        "version": 1,
        "message": "Schema parsed successfully (application not yet implemented)",
        "parsed": format!("{:?}", parsed)
    })))
}

// ============================================================================
// Metrics
// ============================================================================

async fn get_metrics(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    let metrics = session.database.metrics();
    let queries_by_entity: Vec<Value> = metrics
        .queries_by_entity()
        .into_iter()
        .map(|(entity, count)| {
            json!({
                "entity": entity,
                "count": count
            })
        })
        .collect();

    // Get entity counts from catalog
    let catalog = session.database.catalog();
    let entity_list = catalog.list_entities().unwrap_or_default();
    let entity_counts: Vec<Value> = entity_list
        .iter()
        .map(|name| {
            // TODO: Get actual count from storage
            json!({
                "entity": name,
                "count": 0
            })
        })
        .collect();

    Ok(Json(json!({
        "success": true,
        "metrics": {
            "uptime_secs": metrics.uptime_secs(),
            "queries": {
                "total_count": metrics.query_count(),
                "avg_duration_us": metrics.avg_query_latency_us(),
                "p50_duration_us": metrics.p50_query_latency_us(),
                "p99_duration_us": metrics.p99_query_latency_us(),
                "max_duration_us": metrics.max_query_latency_us(),
                "by_entity": queries_by_entity
            },
            "mutations": {
                "total_count": metrics.mutation_count(),
                "inserts": metrics.insert_count(),
                "updates": metrics.update_count(),
                "deletes": metrics.delete_count(),
                "upserts": metrics.upsert_count(),
                "rows_affected": metrics.rows_affected()
            },
            "cache": {
                "hits": metrics.cache_hits(),
                "misses": metrics.cache_misses(),
                "hit_rate": metrics.cache_hit_rate(),
                "size": 0,  // TODO: Get actual cache size
                "capacity": 1000,  // TODO: Get actual capacity
                "evictions": metrics.cache_evictions()
            },
            "storage": {
                "entity_counts": entity_counts,
                "total_entities": entity_list.len()
            }
        }
    })))
}

// ============================================================================
// Explain Query
// ============================================================================

#[derive(Deserialize)]
struct ExplainRequest {
    query: String,
}

async fn explain_query(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<ExplainRequest>,
) -> Result<Json<Value>> {
    let _session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // Parse the query using ormdb-lang
    let parsed = ormdb_lang::parse(&request.query)
        .map_err(|e| StudioError::ParseError(format!("{:?}", e)))?;

    // TODO: Generate actual execution plan
    // For now, return a mock plan based on the parsed query
    Ok(Json(json!({
        "success": true,
        "explain": {
            "plan": format!("Execution plan for: {:?}", parsed),
            "cost": {
                "total_cost": 100.0,
                "estimated_rows": 10,
                "io_cost": 80.0,
                "cpu_cost": 20.0
            },
            "joins": [],
            "plan_cached": false
        }
    })))
}

// ============================================================================
// Replication Status
// ============================================================================

async fn get_replication_status(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>> {
    let _session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // Studio sessions are always standalone (no replication)
    Ok(Json(json!({
        "success": true,
        "replication": {
            "role": "standalone",
            "current_lsn": 0,
            "lag_entries": 0,
            "lag_ms": 0
        }
    })))
}

// ============================================================================
// Storage Compaction
// ============================================================================

async fn compact_storage(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>> {
    let session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id))?;

    // Trigger manual compaction
    let result = session.database.compact();

    Ok(Json(json!({
        "success": true,
        "compaction": {
            "versions_removed": result.versions_removed,
            "tombstones_removed": result.tombstones_removed,
            "bytes_reclaimed": result.bytes_reclaimed,
            "duration_ms": result.duration.as_millis() as u64,
            "entities_processed": result.entities_processed,
            "errors": result.errors,
            "did_cleanup": result.did_cleanup()
        }
    })))
}
