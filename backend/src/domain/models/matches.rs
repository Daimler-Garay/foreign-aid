use crate::domain::models::players::Player;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, sqlx::FromRow)]
pub struct Match {
    pub id: Uuid,
    pub host_player_id: Uuid,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct MatchPlayer {
    pub match_id: Uuid,
    pub player_id: Uuid,
    pub placement: Option<i32>,
    pub joined_at: DateTime<Utc>,
    pub eliminated_at: Option<DateTime<Utc>>,
    pub old_rating: Option<f64>,
    pub old_rating_deviation: Option<f64>,
    pub new_rating: Option<f64>,
    pub new_rating_deviation: Option<f64>,
    pub rating_delta: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateMatchRequest {
    pub display_name: String,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JoinMatchRequest {
    pub display_name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MatchDetail {
    pub match_detail: Match,
    pub players: Vec<MatchPlayerDetail>,
}

#[derive(Deserialize, Serialize, Debug, Clone, sqlx::FromRow)]
pub struct MatchPlayerDetail {
    #[sqlx(flatten)]
    pub player: Player,
    #[sqlx(flatten)]
    pub match_player: MatchPlayer,
}

// the reason for this is the flatten macro can't resolve conflicting column names
// the idea is to use this and bypass it via aliases
#[derive(Debug, sqlx::FromRow)]
pub struct MatchHistoryRow {
    pub match_id: Uuid,
    pub host_player_id: Uuid,
    pub status: String,
    pub notes: Option<String>,
    pub match_created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub player_id: Uuid,
    pub display_name: String,
    pub active: bool,
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    pub player_created_at: DateTime<Utc>,
    pub player_updated_at: DateTime<Utc>,
    pub placement: Option<i32>,
    pub joined_at: DateTime<Utc>,
    pub eliminated_at: Option<DateTime<Utc>>,
    pub old_rating: Option<f64>,
    pub old_rating_deviation: Option<f64>,
    pub new_rating: Option<f64>,
    pub new_rating_deviation: Option<f64>,
    pub rating_delta: Option<f64>,
}
