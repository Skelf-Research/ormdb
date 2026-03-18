//! Replication endpoints.

use axum::{extract::{Query, State}, routing::get, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::json::{
    ChangeLogEntryJson, ReplicationStatusResponse, StreamChangesResponseJson,
    uuid_to_hex,
};
use crate::AppState;

/// Replication routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/replication/status", get(handle_get_status))
        .route("/replication/changes", get(handle_stream_changes))
}

/// Handle get replication status.
async fn handle_get_status(
    State(state): State<AppState>,
) -> Result<Json<ReplicationStatusResponse>, AppError> {
    let status = state.client.get_replication_status().await?;
    Ok(Json(status.into()))
}

/// Query parameters for stream changes.
#[derive(Debug, Deserialize)]
pub struct StreamChangesParams {
    /// Starting LSN (inclusive).
    #[serde(default)]
    pub from_lsn: u64,
    /// Maximum number of entries to return.
    #[serde(default = "default_batch_size")]
    pub limit: u32,
    /// Optional entity filter (comma-separated).
    pub entities: Option<String>,
}

fn default_batch_size() -> u32 {
    1000
}

/// Handle stream changes request.
async fn handle_stream_changes(
    State(state): State<AppState>,
    Query(params): Query<StreamChangesParams>,
) -> Result<Json<StreamChangesResponseJson>, AppError> {
    let entity_filter = params.entities.map(|s| {
        s.split(',').map(|e| e.trim().to_string()).collect()
    });

    let response = state
        .client
        .stream_changes(params.from_lsn, params.limit, entity_filter)
        .await?;

    let entries: Vec<ChangeLogEntryJson> = response
        .entries
        .iter()
        .map(|e| {
            let change_type = match e.change_type {
                ormdb_proto::ChangeType::Insert => "insert",
                ormdb_proto::ChangeType::Update => "update",
                ormdb_proto::ChangeType::Delete => "delete",
            };

            ChangeLogEntryJson {
                lsn: e.lsn,
                timestamp: e.timestamp,
                entity_type: e.entity_type.clone(),
                entity_id: uuid_to_hex(&e.entity_id),
                change_type: change_type.to_string(),
                changed_fields: e.changed_fields.clone(),
                schema_version: e.schema_version,
            }
        })
        .collect();

    Ok(Json(StreamChangesResponseJson {
        success: true,
        entries,
        next_lsn: response.next_lsn,
        has_more: response.has_more,
    }))
}
