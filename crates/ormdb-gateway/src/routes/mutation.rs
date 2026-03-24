//! Mutation endpoint.

use axum::{extract::State, routing::post, Json, Router};
use ormdb_proto::mutation::{Mutation, MutationBatch};

use crate::error::AppError;
use crate::json::MutationResponse;
use crate::AppState;

/// Mutation routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mutate", post(handle_mutate))
        .route("/mutate-batch", post(handle_mutate_batch))
}

/// Handle a mutation.
async fn handle_mutate(
    State(state): State<AppState>,
    Json(mutation): Json<Mutation>,
) -> Result<Json<MutationResponse>, AppError> {
    let pool = state.pool.clone();
    let result = state.execute_write(|| pool.mutate(mutation.clone())).await?;
    Ok(Json(result.into()))
}

/// Handle a batch of mutations atomically.
async fn handle_mutate_batch(
    State(state): State<AppState>,
    Json(batch): Json<MutationBatch>,
) -> Result<Json<MutationResponse>, AppError> {
    let pool = state.pool.clone();
    let result = state.execute_write(|| pool.mutate_batch(batch.clone())).await?;
    Ok(Json(result.into()))
}
