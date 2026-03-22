use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use ormdb_core::catalog::FieldType;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
}

async fn create_session(State(state): State<AppState>) -> Result<Json<SessionResponse>> {
    let session = state.sessions.create_session()?;

    Ok(Json(SessionResponse {
        success: true,
        session: SessionInfo {
            id: session.id.clone(),
            age_secs: Some(0),
            last_activity_secs: Some(0),
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
    let _session = state
        .sessions
        .get_session(&session_id)
        .ok_or_else(|| StudioError::SessionNotFound(session_id.clone()))?;

    // Parse the query using ormdb-lang
    let query_str = request.query.clone();
    let parsed = ormdb_lang::parse(&query_str)
        .map_err(|e| StudioError::ParseError(format!("{:?}", e)))?;

    // TODO: Compile and execute the parsed query
    // For now, return the parse result
    Ok(Json(json!({
        "success": true,
        "parsed": format!("{:?}", parsed),
        "message": "Query parsed successfully (execution not yet implemented)"
    })))
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
