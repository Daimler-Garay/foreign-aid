use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, sqlx::FromRow)]
pub struct Player {
    pub id: Uuid,
    pub display_name: String,
    pub active: bool,
    pub rating: f32,
    pub rating_deviation: f32,
    pub volatility: f32,
    pub games_played: i32,
    pub wins: i32,
    pub losses: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
