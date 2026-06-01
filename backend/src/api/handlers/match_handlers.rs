use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::types::Uuid;
use thiserror::Error;

use crate::{
    api::{
        error::{ApiError, ApiErrorCode, ApiErrorEntry, ApiErrorKind},
        version::{self, ApiVersion},
    },
    application::{repository::matches_repo, state::SharedState},
    domain::models::matches::{CreateMatchRequest, Match},
};

pub async fn create_match_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
    Json(matches): Json<CreateMatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::trace!("api version: {}", api_version);

    // guard
    todo!()
}
