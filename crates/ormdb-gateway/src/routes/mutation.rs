//! Mutation endpoint.

use axum::{extract::State, routing::post, Json, Router};
use ormdb_proto::mutation::Mutation;

use crate::error::AppError;
use crate::json::MutationResponse;
use crate::AppState;

/// Mutation routes.
pub fn routes() -> Router<AppState> {
    Router::new().route("/mutate", post(handle_mutate))
}

/// Handle a mutation.
async fn handle_mutate(
    State(state): State<AppState>,
    Json(mutation): Json<Mutation>,
) -> Result<Json<MutationResponse>, AppError> {
    let result = state.client.mutate(mutation).await?;
    Ok(Json(result.into()))
}
