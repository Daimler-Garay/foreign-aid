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
    application::{
        repository::{matches_repo, player_repo},
        state::SharedState,
    },
    domain::models::matches::{CreateMatchRequest, JoinMatchRequest, Match, MatchPlayer},
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

#[derive(Debug, Error)]
enum MatchError {
    #[error("host player not found: {0}")]
    HostNotFound(String),
}

impl MatchError {
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::HostNotFound(_) => StatusCode::NOT_FOUND,
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
        }
    }
}
