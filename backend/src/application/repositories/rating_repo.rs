use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::{application::repositories::RepositoryResult, domain::models::players::PlayerRating};

pub const RATING_MUTATION_ADVISORY_LOCK_ID: i64 = 74_000_001;

pub async fn acquire_rating_mutation_lock<'e, E>(executor: E) -> RepositoryResult<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(RATING_MUTATION_ADVISORY_LOCK_ID)
        .execute(executor)
        .await?;

    Ok(())
}

pub async fn load_active_player_ratings_for_update<'e, E>(
    executor: E,
    player_ids: &[Uuid],
) -> RepositoryResult<Vec<PlayerRating>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, PlayerRating>(
        r#"
        SELECT
            pr.player_id,
            pr.rating,
            pr.uncertainty,
            pr.games_played,
            pr.wins,
            pr.losses,
            pr.total_placement,
            pr.last_played_at,
            pr.updated_at
        FROM player_ratings pr
        JOIN players p ON p.id = pr.player_id
        WHERE pr.player_id = ANY($1)
          AND p.active = TRUE
        FOR UPDATE OF pr
        "#,
    )
    .bind(player_ids)
    .fetch_all(executor)
    .await
}

pub async fn update_player_rating_after_match<'e, E>(
    executor: E,
    player_id: Uuid,
    new_rating: f64,
    new_uncertainty: f64,
    placement: i32,
    played_at: DateTime<Utc>,
) -> RepositoryResult<PlayerRating>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, PlayerRating>(
        r#"
        UPDATE player_ratings
        SET rating = $2,
            uncertainty = $3,
            games_played = games_played + 1,
            wins = wins + CASE WHEN $4 = 1 THEN 1 ELSE 0 END,
            losses = losses + CASE WHEN $4 = 1 THEN 0 ELSE 1 END,
            total_placement = total_placement + $4,
            last_played_at = GREATEST(COALESCE(last_played_at, $5), $5)
        WHERE player_id = $1
        RETURNING player_id, rating, uncertainty, games_played, wins, losses,
                  total_placement, last_played_at, updated_at
        "#,
    )
    .bind(player_id)
    .bind(new_rating)
    .bind(new_uncertainty)
    .bind(placement)
    .bind(played_at)
    .fetch_one(executor)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::repositories::player_repo,
        db::{Database, DatabaseOptions, options::PostgresOptions},
    };

    fn test_options() -> DatabaseOptions {
        DatabaseOptions {
            postgres: PostgresOptions {
                database_url: None,
                db: "foreign_aid".to_string(),
                host: "localhost".to_string(),
                port: 5433,
                user: "admin".to_string(),
                password: "admin".to_string(),
                max_connections: 5,
            },
        }
    }

    #[tokio::test]
    async fn can_lock_load_and_update_player_rating() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let player_id = Uuid::new_v4();
        player_repo::insert_player(pool, player_id, "Alice", None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");

        let mut tx = pool.begin().await.expect("transaction should begin");
        acquire_rating_mutation_lock(&mut *tx)
            .await
            .expect("lock should acquire");
        let ratings = load_active_player_ratings_for_update(&mut *tx, &[player_id])
            .await
            .expect("ratings should load");
        assert_eq!(ratings.len(), 1);

        let updated =
            update_player_rating_after_match(&mut *tx, player_id, 26.0, 8.0, 1, Utc::now())
                .await
                .expect("rating should update");
        tx.commit().await.expect("transaction should commit");

        assert_eq!(updated.player_id, player_id);
        assert_eq!(updated.games_played, 1);
        assert_eq!(updated.wins, 1);
        assert_eq!(updated.losses, 0);
        assert_eq!(updated.total_placement, 1);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
