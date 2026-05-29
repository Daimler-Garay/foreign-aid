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
