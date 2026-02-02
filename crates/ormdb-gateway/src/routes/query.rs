//! Query endpoint.

use axum::{extract::State, routing::post, Json, Router};
use ormdb_proto::query::GraphQuery;

use crate::error::AppError;
use crate::json::QueryResponse;
use crate::AppState;

/// Query routes.
pub fn routes() -> Router<AppState> {
    Router::new().route("/query", post(handle_query))
}

/// Handle a graph query.
async fn handle_query(
    State(state): State<AppState>,
    Json(query): Json<GraphQuery>,
) -> Result<Json<QueryResponse>, AppError> {
    let pool = state.pool.clone();
    let result = state.execute_read(|| pool.query(query.clone())).await?;
    Ok(Json(result.into()))
}
