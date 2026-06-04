use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardQuery {
    pub min_games: Option<i32>,
    pub include_inactive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LeaderboardRow {
    pub rank: Option<i64>,
    pub player_id: Uuid,
    pub display_name: String,
    pub display_rating: i32,
    pub rank_score: i32,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    pub win_rate: Option<f64>,
    pub average_placement: Option<f64>,
    pub last_played_at: Option<DateTime<Utc>>,
    pub active: bool,
}
