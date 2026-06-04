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
        version::ApiVersion,
    },
    application::{
        repository::{matches_repo, player_repo},
        state::SharedState,
    },
    domain::models::matches::{
        CreateMatchRequest, JoinMatchRequest, Match, MatchDetail, MatchPlayer, MatchPlayerDetail,
    },
};

pub async fn create_match_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
    Json(matches): Json<CreateMatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::trace!("api version: {}", api_version);
    let host = player_repo::get_player_by_display_name(&matches.display_name, &state)
        .await
        // map host not found
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                let host_error = MatchError::HostNotFound(matches.display_name.clone());
                (host_error.status_code(), ApiErrorEntry::from(host_error)).into()
            }
            _ => ApiError::from(e),
        })?;

    let create_match = matches_repo::create_match(host.id, &matches, &state).await?;

    Ok((StatusCode::CREATED, Json(create_match)))
}

pub async fn get_match_detail_by_id_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MatchDetail>, ApiError> {
    tracing::trace!("api version {}", api_version);
    tracing::trace!("match id: {}", id);
    let matches = matches_repo::get_match_detail_by_id(id, &state)
        .await
        // handle match_id not found
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                let match_error = MatchError::MatchNotFound(id);
                (match_error.status_code(), ApiErrorEntry::from(match_error)).into()
            }
            _ => ApiError::from(e),
        })?;

    Ok(Json(matches))
}

pub async fn get_match_history_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
) -> Result<Json<Vec<MatchDetail>>, ApiError> {
    tracing::trace!("api version: {}", api_version);
    let history = matches_repo::get_match_history(&state).await?;
    Ok(Json(history))
}

#[derive(Debug, Error)]
enum MatchError {
    #[error("host player not found: {0}")]
    HostNotFound(String),
    #[error("match id not found: {0}")]
    MatchNotFound(Uuid),
}

impl MatchError {
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::HostNotFound(_) => StatusCode::NOT_FOUND,
            Self::MatchNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

impl From<MatchError> for ApiErrorEntry {
    fn from(match_error: MatchError) -> Self {
        let message = match_error.to_string();

        match match_error {
            MatchError::HostNotFound(display_name) => Self::new(&message)
                .code(ApiErrorCode::PlayerNotFound)
                .kind(ApiErrorKind::ResourceNotFound)
                .description(&format!(
                    "host with the display name '{}' does not exist in our records",
                    display_name
                ))
                .reason("host player must exist")
                .trace_id()
                .help("the host specified doesn't exist"),
            MatchError::MatchNotFound(uuid) => Self::new(&message)
                .code(ApiErrorCode::MatchNotFound)
                .kind(ApiErrorKind::ResourceNotFound)
                .reason("must be an existing match")
                .description(&format!(
                    "match with ID '{}' does not exist in our records",
                    uuid
                ))
                .instance(&format!("/api/v1/matches/{}", uuid))
                .trace_id()
                .help("please check if the match identifier is correct"),
        }
    }
}
