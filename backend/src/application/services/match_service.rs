use std::collections::{HashMap, HashSet};

use axum::http::StatusCode;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    api::error::ApiError,
    application::{
        repositories::{
            audit_repo::{self, NewAuditLogEntry},
            match_repo::{self, NewMatch, NewMatchPlayer},
            rating_repo,
        },
        services::rating_service::{RatingInput, display_rating, rate_ranked_free_for_all},
        state::SharedState,
    },
    domain::{
        models::{
            auth::AuthenticatedUser,
            matches::{CreateMatchRequest, MatchSubmissionResponse, RatingChangeResponse},
            players::PlayerRating,
        },
        validation::{
            ValidationError,
            match_validation::{validate_match_submission, validate_placements},
        },
    },
};

pub const RATING_ALGORITHM: &str = "weng_lin";
pub const RATING_ALGORITHM_VERSION: i32 = 1;

pub async fn submit_match(
    state: &SharedState,
    actor: &AuthenticatedUser,
    request: CreateMatchRequest,
) -> Result<MatchSubmissionResponse, ApiError> {
    validate_placements(&request.placements).map_err(validation_error)?;

    let player_ids: Vec<Uuid> = request
        .placements
        .iter()
        .map(|placement| placement.player_id)
        .collect();
    let mut tx = state.db_pool.begin().await?;
    rating_repo::acquire_rating_mutation_lock(&mut *tx).await?;

    let current_ratings =
        rating_repo::load_active_player_ratings_for_update(&mut *tx, &player_ids).await?;
    let active_player_ids: HashSet<Uuid> = current_ratings
        .iter()
        .map(|rating| rating.player_id)
        .collect();
    validate_match_submission(
        request.played_at,
        &request.placements,
        &active_player_ids,
        Utc::now(),
    )
    .map_err(validation_error)?;

    let current_ratings_by_player_id: HashMap<Uuid, PlayerRating> = current_ratings
        .into_iter()
        .map(|rating| (rating.player_id, rating))
        .collect();
    let rating_inputs = request
        .placements
        .iter()
        .map(|placement| {
            let rating = current_ratings_by_player_id
                .get(&placement.player_id)
                .expect("active player validation should ensure rating exists");
            RatingInput {
                player_id: placement.player_id,
                rating: rating.rating,
                uncertainty: rating.uncertainty,
                placement: placement.placement,
            }
        })
        .collect::<Vec<_>>();
    let rating_updates = rate_ranked_free_for_all(&rating_inputs).map_err(rating_error)?;

    let match_id = Uuid::new_v4();
    let inserted_match = match_repo::insert_confirmed_match(
        &mut *tx,
        NewMatch {
            id: match_id,
            played_at: request.played_at,
            submitted_by_user_id: Some(actor.id),
            notes: request.notes.clone(),
            rating_algorithm: RATING_ALGORITHM.to_owned(),
            rating_algorithm_version: RATING_ALGORITHM_VERSION,
        },
    )
    .await?;

    for update in &rating_updates {
        match_repo::insert_match_player(
            &mut *tx,
            NewMatchPlayer {
                match_id,
                player_id: update.player_id,
                placement: update.placement,
                old_rating: update.old_rating,
                old_uncertainty: update.old_uncertainty,
                new_rating: update.new_rating,
                new_uncertainty: update.new_uncertainty,
                rating_delta: update.rating_delta,
            },
        )
        .await?;
        rating_repo::update_player_rating_after_match(
            &mut *tx,
            update.player_id,
            update.new_rating,
            update.new_uncertainty,
            update.placement,
            request.played_at,
        )
        .await?;
    }

    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "match.created".to_owned(),
            entity_type: "match".to_owned(),
            entity_id: Some(match_id),
            old_value: None,
            new_value: Some(json!({
                "id": match_id,
                "played_at": request.played_at,
                "placements": request.placements,
                "rating_algorithm": RATING_ALGORITHM,
                "rating_algorithm_version": RATING_ALGORITHM_VERSION,
            })),
        },
    )
    .await?;

    tx.commit().await?;

    let mut rating_changes = rating_updates
        .iter()
        .map(|update| {
            let old_display_rating = display_rating(update.old_rating);
            let new_display_rating = display_rating(update.new_rating);

            RatingChangeResponse {
                player_id: update.player_id,
                placement: update.placement,
                old_display_rating,
                new_display_rating,
                display_delta: new_display_rating - old_display_rating,
            }
        })
        .collect::<Vec<_>>();
    rating_changes.sort_by_key(|change| change.placement);

    Ok(MatchSubmissionResponse {
        match_id: inserted_match.id,
        status: inserted_match.status,
        rating_changes,
    })
}

fn validation_error(error: ValidationError) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "validation_error",
        error.to_string(),
    )
}

fn rating_error(error: impl std::fmt::Display) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "rating_error", error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Duration;

    use super::*;
    use crate::{
        application::{
            config::Config,
            repositories::{player_repo, user_repo},
            state::AppState,
        },
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::{auth::UserRole, matches::PlacementRequest},
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

    fn test_config() -> Config {
        Config {
            app_env: "test".to_owned(),
            app_host: "127.0.0.1".to_owned(),
            app_port: 0,
            database: test_options().postgres,
        }
    }

    async fn admin_actor(pool: &crate::db::DatabasePool) -> AuthenticatedUser {
        let user_id = Uuid::new_v4();
        user_repo::insert_user(pool, user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("admin should insert");

        AuthenticatedUser {
            id: user_id,
            username: "admin".to_owned(),
            role: "admin".to_owned(),
            active: true,
            player_id: None,
            session_id: Uuid::new_v4(),
        }
    }

    async fn create_player(pool: &crate::db::DatabasePool, display_name: &str) -> Uuid {
        let player_id = Uuid::new_v4();
        player_repo::insert_player(pool, player_id, display_name, None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");

        player_id
    }

    fn request(players: &[Uuid]) -> CreateMatchRequest {
        CreateMatchRequest {
            played_at: Utc::now() - Duration::minutes(1),
            notes: Some("test match".to_owned()),
            placements: players
                .iter()
                .enumerate()
                .map(|(index, player_id)| PlacementRequest {
                    player_id: *player_id,
                    placement: (index + 1) as i32,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn submit_match_stores_snapshots_updates_ratings_and_audit() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = create_player(db.pool(), "Alice").await;
        let ben = create_player(db.pool(), "Ben").await;
        let chloe = create_player(db.pool(), "Chloe").await;

        let response = submit_match(&state, &actor, request(&[alice, ben, chloe]))
            .await
            .expect("match should submit");

        assert_eq!(response.status, "confirmed");
        assert_eq!(response.rating_changes.len(), 3);
        assert!(response.rating_changes[0].display_delta > 0);
        assert!(response.rating_changes[2].display_delta < 0);

        let participant_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM match_players WHERE match_id = $1")
                .bind(response.match_id)
                .fetch_one(db.pool())
                .await
                .expect("participant count should load");
        assert_eq!(participant_count, 3);

        let winner_rating: PlayerRating =
            sqlx::query_as("SELECT * FROM player_ratings WHERE player_id = $1")
                .bind(alice)
                .fetch_one(db.pool())
                .await
                .expect("winner rating should load");
        assert_eq!(winner_rating.games_played, 1);
        assert_eq!(winner_rating.wins, 1);
        assert_eq!(winner_rating.losses, 0);

        let audit_action: String =
            sqlx::query_scalar("SELECT action FROM audit_log WHERE entity_id = $1")
                .bind(response.match_id)
                .fetch_one(db.pool())
                .await
                .expect("audit action should load");
        assert_eq!(audit_action, "match.created");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn submit_match_rejects_inactive_player_without_storing_match() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = create_player(db.pool(), "Alice").await;
        let ben = create_player(db.pool(), "Ben").await;
        player_repo::set_player_active(db.pool(), ben, false)
            .await
            .expect("player should deactivate");

        let error = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect_err("inactive player should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST.as_u16());
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(db.pool())
            .await
            .expect("match count should load");
        assert_eq!(match_count, 0);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn submit_match_rolls_back_when_match_insert_fails() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = AuthenticatedUser {
            id: Uuid::new_v4(),
            username: "missing-admin".to_owned(),
            role: "admin".to_owned(),
            active: true,
            player_id: None,
            session_id: Uuid::new_v4(),
        };
        let alice = create_player(db.pool(), "Alice").await;
        let ben = create_player(db.pool(), "Ben").await;

        let error = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect_err("missing submitting user should fail");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(db.pool())
            .await
            .expect("match count should load");
        let participant_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM match_players")
            .fetch_one(db.pool())
            .await
            .expect("participant count should load");
        let games_played: i32 =
            sqlx::query_scalar("SELECT games_played FROM player_ratings WHERE player_id = $1")
                .bind(alice)
                .fetch_one(db.pool())
                .await
                .expect("rating should load");
        assert_eq!(match_count, 0);
        assert_eq!(participant_count, 0);
        assert_eq!(games_played, 0);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
