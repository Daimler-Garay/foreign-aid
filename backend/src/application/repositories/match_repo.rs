use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::{
    application::repositories::RepositoryResult,
    domain::models::matches::{MatchPlayer, MatchResult, MatchStatus},
};

pub struct NewMatch {
    pub id: Uuid,
    pub played_at: DateTime<Utc>,
    pub submitted_by_user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub rating_algorithm: String,
    pub rating_algorithm_version: i32,
}

pub struct NewMatchPlayer {
    pub match_id: Uuid,
    pub player_id: Uuid,
    pub placement: i32,
    pub old_rating: f64,
    pub old_uncertainty: f64,
    pub new_rating: f64,
    pub new_uncertainty: f64,
    pub rating_delta: f64,
}

pub async fn insert_confirmed_match<'e, E>(
    executor: E,
    new_match: NewMatch,
) -> RepositoryResult<MatchResult>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, MatchResult>(
        r#"
        INSERT INTO matches (
            id,
            played_at,
            submitted_by_user_id,
            status,
            notes,
            rating_algorithm,
            rating_algorithm_version
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, played_at, submitted_by_user_id, status, notes,
                  rating_algorithm, rating_algorithm_version,
                  corrected_from_match_id, created_at, updated_at
        "#,
    )
    .bind(new_match.id)
    .bind(new_match.played_at)
    .bind(new_match.submitted_by_user_id)
    .bind(MatchStatus::Confirmed.as_str())
    .bind(new_match.notes)
    .bind(new_match.rating_algorithm)
    .bind(new_match.rating_algorithm_version)
    .fetch_one(executor)
    .await
}

pub async fn insert_match_player<'e, E>(
    executor: E,
    participant: NewMatchPlayer,
) -> RepositoryResult<MatchPlayer>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, MatchPlayer>(
        r#"
        INSERT INTO match_players (
            match_id,
            player_id,
            placement,
            old_rating,
            old_uncertainty,
            new_rating,
            new_uncertainty,
            rating_delta
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING match_id, player_id, placement, old_rating, old_uncertainty,
                  new_rating, new_uncertainty, rating_delta, created_at
        "#,
    )
    .bind(participant.match_id)
    .bind(participant.player_id)
    .bind(participant.placement)
    .bind(participant.old_rating)
    .bind(participant.old_uncertainty)
    .bind(participant.new_rating)
    .bind(participant.new_uncertainty)
    .bind(participant.rating_delta)
    .fetch_one(executor)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::repositories::{player_repo, user_repo},
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::auth::UserRole,
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
    async fn can_insert_confirmed_match_and_participant_in_transaction() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();
        let player_id = Uuid::new_v4();
        user_repo::insert_user(pool, user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("user should insert");
        player_repo::insert_player(pool, player_id, "Alice", None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");

        let mut tx = pool.begin().await.expect("transaction should begin");
        let match_id = Uuid::new_v4();
        let inserted_match = insert_confirmed_match(
            &mut *tx,
            NewMatch {
                id: match_id,
                played_at: Utc::now(),
                submitted_by_user_id: Some(user_id),
                notes: Some("test".to_owned()),
                rating_algorithm: "weng_lin".to_owned(),
                rating_algorithm_version: 1,
            },
        )
        .await
        .expect("match should insert");
        let participant = insert_match_player(
            &mut *tx,
            NewMatchPlayer {
                match_id,
                player_id,
                placement: 1,
                old_rating: 25.0,
                old_uncertainty: 25.0 / 3.0,
                new_rating: 26.0,
                new_uncertainty: 8.0,
                rating_delta: 1.0,
            },
        )
        .await
        .expect("participant should insert");
        tx.commit().await.expect("transaction should commit");

        assert_eq!(inserted_match.id, match_id);
        assert_eq!(inserted_match.status, "confirmed");
        assert_eq!(participant.player_id, player_id);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
