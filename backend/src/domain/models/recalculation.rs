use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecalculationStatus {
    Running,
    Succeeded,
    Failed,
}

impl RecalculationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct RatingRecalculationRun {
    pub id: Uuid,
    pub triggered_by_user_id: Option<Uuid>,
    pub reason: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecalculateRatingsRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecalculateRatingsResponse {
    pub run_id: Uuid,
    pub status: String,
}
