use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct Player {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub display_name: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct PlayerRating {
    pub player_id: Uuid,
    pub rating: f64,
    pub uncertainty: f64,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    pub total_placement: i32,
    pub last_played_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct PlayerWithRating {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub display_name: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub rating: f64,
    pub uncertainty: f64,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    pub total_placement: i32,
    pub last_played_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePlayerRequest {
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePlayerRequest {
    pub display_name: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerResponse {
    pub id: Uuid,
    pub display_name: String,
    pub active: bool,
    pub rating: PlayerRatingSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerRatingSummary {
    pub display_rating: i32,
    pub rank_score: i32,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
}
