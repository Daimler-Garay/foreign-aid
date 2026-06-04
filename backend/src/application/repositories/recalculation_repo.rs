use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::{
    application::repositories::RepositoryResult,
    db::DatabasePool,
    domain::models::recalculation::{RatingRecalculationRun, RecalculationStatus},
};

pub async fn insert_recalculation_run(
    pool: &DatabasePool,
    id: Uuid,
    triggered_by_user_id: Option<Uuid>,
    reason: &str,
) -> RepositoryResult<RatingRecalculationRun> {
    sqlx::query_as::<_, RatingRecalculationRun>(
        r#"
        INSERT INTO rating_recalculation_runs (
            id,
            triggered_by_user_id,
            reason,
            status
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id, triggered_by_user_id, reason, started_at, finished_at,
                  status, error_message
        "#,
    )
    .bind(id)
    .bind(triggered_by_user_id)
    .bind(reason)
    .bind(RecalculationStatus::Running.as_str())
    .fetch_one(pool)
    .await
}

pub async fn finish_recalculation_run<'e, E>(
    executor: E,
    id: Uuid,
    status: RecalculationStatus,
    finished_at: DateTime<Utc>,
    error_message: Option<&str>,
) -> RepositoryResult<RatingRecalculationRun>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, RatingRecalculationRun>(
        r#"
        UPDATE rating_recalculation_runs
        SET status = $2,
            finished_at = $3,
            error_message = $4
        WHERE id = $1
        RETURNING id, triggered_by_user_id, reason, started_at, finished_at,
                  status, error_message
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(finished_at)
    .bind(error_message)
    .fetch_one(executor)
    .await
}
