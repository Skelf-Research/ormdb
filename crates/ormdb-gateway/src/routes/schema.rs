//! Schema endpoint.

use axum::body::Bytes;
use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::AppState;

/// Schema routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/schema", get(handle_get_schema))
        .route("/schema", post(handle_apply_schema))
}

/// Schema response.
#[derive(Debug, Serialize)]
pub struct SchemaResponse {
    /// Success flag.
    pub success: bool,
    /// Schema version.
    pub version: u64,
    /// Schema data (raw bytes for now, can be expanded).
    pub entities: Vec<EntitySchema>,
}

/// Entity schema.
#[derive(Debug, Serialize)]
pub struct EntitySchema {
    /// Entity name.
    pub name: String,
    /// Entity fields.
    pub fields: Vec<FieldSchema>,
    /// Entity relations.
    pub relations: Vec<RelationSchema>,
}

/// Field schema.
#[derive(Debug, Serialize)]
pub struct FieldSchema {
    /// Field name.
    pub name: String,
    /// Field type.
    #[serde(rename = "type")]
    pub field_type: String,
    /// Whether this is a primary key.
    pub primary: bool,
    /// Whether this field is optional.
    pub optional: bool,
}

/// Relation schema.
#[derive(Debug, Serialize)]
pub struct RelationSchema {
    /// Relation name.
    pub name: String,
    /// Target entity.
    pub target: String,
    /// Relation type (one_to_one, one_to_many, many_to_many).
    #[serde(rename = "type")]
    pub relation_type: String,
}

/// Schema apply request (JSON body).
#[derive(Debug, Deserialize)]
pub struct ApplySchemaRequest {
    /// Schema definition as a string (will be converted to bytes).
    pub schema: String,
}

/// Schema applied response.
#[derive(Debug, Serialize)]
pub struct SchemaAppliedResponse {
    /// Success flag.
    pub success: bool,
    /// New schema version.
    pub version: u64,
}

/// Handle get schema request.
async fn handle_get_schema(
    State(state): State<AppState>,
) -> Result<Json<SchemaResponse>, AppError> {
    let pool = state.pool.clone();
    let (version, _data) = state.execute_read(|| pool.get_schema()).await?;

    // For now, return an empty schema structure
    // In a full implementation, we would deserialize the schema data
    // and convert it to the JSON-friendly format
    Ok(Json(SchemaResponse {
        success: true,
        version,
        entities: vec![], // Would be populated from schema data
    }))
}

/// Handle apply schema request.
async fn handle_apply_schema(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<SchemaAppliedResponse>, AppError> {
    let pool = state.pool.clone();
    let schema_bytes = body.to_vec();
    let version = state.execute_write(|| pool.apply_schema(schema_bytes.clone())).await?;
    Ok(Json(SchemaAppliedResponse {
        success: true,
        version,
    }))
}
