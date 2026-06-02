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
