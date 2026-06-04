use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Pending,
    Confirmed,
    Voided,
}

impl MatchStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Voided => "voided",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct MatchResult {
    pub id: Uuid,
    pub played_at: DateTime<Utc>,
    pub submitted_by_user_id: Option<Uuid>,
    pub status: String,
    pub notes: Option<String>,
    pub rating_algorithm: String,
    pub rating_algorithm_version: i32,
    pub corrected_from_match_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct MatchPlayer {
    pub match_id: Uuid,
    pub player_id: Uuid,
    pub placement: i32,
    pub old_rating: f64,
    pub old_uncertainty: f64,
    pub new_rating: f64,
    pub new_uncertainty: f64,
    pub rating_delta: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct MatchParticipantRow {
    pub match_id: Uuid,
    pub player_id: Uuid,
    pub display_name: String,
    pub placement: i32,
    pub old_rating: f64,
    pub old_uncertainty: f64,
    pub new_rating: f64,
    pub new_uncertainty: f64,
    pub rating_delta: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMatchRequest {
    pub played_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub placements: Vec<PlacementRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlacementRequest {
    pub player_id: Uuid,
    pub placement: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchSubmissionResponse {
    pub match_id: Uuid,
    pub status: String,
    pub rating_changes: Vec<RatingChangeResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RatingChangeResponse {
    pub player_id: Uuid,
    pub placement: i32,
    pub old_display_rating: i32,
    pub new_display_rating: i32,
    pub display_delta: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchSummaryResponse {
    pub id: Uuid,
    pub played_at: DateTime<Utc>,
    pub submitted_by_user_id: Option<Uuid>,
    pub status: String,
    pub notes: Option<String>,
    pub rating_algorithm: String,
    pub rating_algorithm_version: i32,
    pub corrected_from_match_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchDetailResponse {
    pub id: Uuid,
    pub played_at: DateTime<Utc>,
    pub submitted_by_user_id: Option<Uuid>,
    pub status: String,
    pub notes: Option<String>,
    pub rating_algorithm: String,
    pub rating_algorithm_version: i32,
    pub corrected_from_match_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub participants: Vec<MatchParticipantResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchParticipantResponse {
    pub player_id: Uuid,
    pub display_name: String,
    pub placement: i32,
    pub old_rating: f64,
    pub old_uncertainty: f64,
    pub new_rating: f64,
    pub new_uncertainty: f64,
    pub rating_delta: f64,
    pub old_display_rating: i32,
    pub new_display_rating: i32,
    pub display_delta: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoidMatchResponse {
    pub match_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrectMatchResponse {
    pub original_match_id: Uuid,
    pub corrected_match_id: Uuid,
    pub status: String,
}
