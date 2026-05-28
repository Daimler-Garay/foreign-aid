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
    application::{repository::player_repo, state::SharedState},
    domain::models::players::{CreatePlayer, Player},
};

pub async fn add_player_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
    Json(player): Json<CreatePlayer>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::trace!("api version: {}", api_version);
    let player = player_repo::add_player(player, &state).await?;
    Ok((StatusCode::CREATED, Json(player)))
}

pub async fn list_player_handler(
    api_version: ApiVersion,
    State(state): State<SharedState>,
) -> Result<Json<Vec<Player>>, ApiError> {
    tracing::trace!("api version {}", api_version);
    let players = player_repo::list_players(&state).await?;
    Ok(Json(players))
}

pub async fn get_player_by_id_handler(
    State(state): State<SharedState>,
    Path((version, id)): Path<(String, Uuid)>,
) -> Result<Json<Player>, ApiError> {
    let api_version: ApiVersion = version::parse_version(&version)?;
    tracing::trace!("api version {}", api_version);
    tracing::trace!("id: {}", id);
    let player = player_repo::get_player_by_id(id, &state)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                let player_error = PlayerError::PlayerNotFound(id);
                (
                    player_error.status_code(),
                    ApiErrorEntry::from(player_error),
                )
                    .into()
            }
            _ => ApiError::from(e),
        })?;

    Ok(Json(player))
}

#[derive(Debug, Error)]
enum PlayerError {
    #[error("player not found: {0}")]
    PlayerNotFound(Uuid),
}

impl PlayerError {
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::PlayerNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

impl From<PlayerError> for ApiErrorEntry {
    fn from(player_error: PlayerError) -> Self {
        let message = player_error.to_string();
        match player_error {
            PlayerError::PlayerNotFound(player_id) => Self::new(&message)
                .code(ApiErrorCode::PlayerNotFound)
                .kind(ApiErrorKind::ResourceNotFound)
                .description(&format!(
                    "player with the ID '{}' does not exist in our records",
                    player_id
                ))
                .detail(serde_json::json!({"player_id": player_id}))
                .reason("must be an existing player")
                .instance(&format!("/api/v1/players/{}", player_id))
                .trace_id()
                .help(&format!("please check if the player ID is correct")),
        }
    }
}
