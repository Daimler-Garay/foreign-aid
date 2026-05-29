use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, sqlx::FromRow)]
pub struct Player {
    pub id: Uuid,
    pub display_name: String,
    pub active: bool,
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Serialize, Debug, Clone, sqlx::FromRow)]
pub struct CreatePlayer {
    pub display_name: String,
    pub active: bool,
}
