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
            matches::{
                CreateMatchRequest, MatchDetailResponse, MatchParticipantResponse, MatchResult,
                MatchSubmissionResponse, MatchSummaryResponse, RatingChangeResponse,
            },
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

pub async fn list_matches(state: &SharedState) -> Result<Vec<MatchSummaryResponse>, ApiError> {
    let matches = match_repo::list_matches(&state.db_pool).await?;

    Ok(matches.into_iter().map(match_summary_response).collect())
}

pub async fn get_match(
    state: &SharedState,
    match_id: Uuid,
) -> Result<MatchDetailResponse, ApiError> {
    let match_result = match_repo::find_match_by_id(&state.db_pool, match_id)
        .await?
        .ok_or_else(|| ApiError::not_found("match_not_found", "Match was not found."))?;
    let participants = match_repo::list_match_participants(&state.db_pool, match_id)
        .await?
        .into_iter()
        .map(|participant| {
            let old_display_rating = display_rating(participant.old_rating);
            let new_display_rating = display_rating(participant.new_rating);

            MatchParticipantResponse {
                player_id: participant.player_id,
                display_name: participant.display_name,
                placement: participant.placement,
                old_rating: participant.old_rating,
                old_uncertainty: participant.old_uncertainty,
                new_rating: participant.new_rating,
                new_uncertainty: participant.new_uncertainty,
                rating_delta: participant.rating_delta,
                old_display_rating,
                new_display_rating,
                display_delta: new_display_rating - old_display_rating,
            }
        })
        .collect();

    Ok(MatchDetailResponse {
        id: match_result.id,
        played_at: match_result.played_at,
        submitted_by_user_id: match_result.submitted_by_user_id,
        status: match_result.status,
        notes: match_result.notes,
        rating_algorithm: match_result.rating_algorithm,
        rating_algorithm_version: match_result.rating_algorithm_version,
        corrected_from_match_id: match_result.corrected_from_match_id,
        created_at: match_result.created_at,
        updated_at: match_result.updated_at,
        participants,
    })
}

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

fn match_summary_response(match_result: MatchResult) -> MatchSummaryResponse {
    MatchSummaryResponse {
        id: match_result.id,
        played_at: match_result.played_at,
        submitted_by_user_id: match_result.submitted_by_user_id,
        status: match_result.status,
        notes: match_result.notes,
        rating_algorithm: match_result.rating_algorithm,
        rating_algorithm_version: match_result.rating_algorithm_version,
        corrected_from_match_id: match_result.corrected_from_match_id,
        created_at: match_result.created_at,
        updated_at: match_result.updated_at,
    }
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
    async fn list_matches_returns_history_in_deterministic_order() {
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
        let older = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect("older match should submit");
        let mut newer_request = request(&[ben, alice]);
        newer_request.played_at += Duration::minutes(1);
        let newer = submit_match(&state, &actor, newer_request)
            .await
            .expect("newer match should submit");

        let matches = list_matches(&state).await.expect("matches should list");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].id, newer.match_id);
        assert_eq!(matches[1].id, older.match_id);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn get_match_returns_participants_and_rating_deltas() {
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
        let submitted = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect("match should submit");

        let detail = get_match(&state, submitted.match_id)
            .await
            .expect("match detail should load");

        assert_eq!(detail.id, submitted.match_id);
        assert_eq!(detail.participants.len(), 2);
        assert_eq!(detail.participants[0].player_id, alice);
        assert_eq!(detail.participants[0].display_name, "Alice");
        assert_eq!(detail.participants[0].placement, 1);
        assert!(detail.participants[0].display_delta > 0);
        assert_eq!(detail.participants[1].player_id, ben);
        assert!(detail.participants[1].display_delta < 0);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn get_match_rejects_missing_match() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let error = get_match(&state, Uuid::new_v4())
            .await
            .expect_err("missing match should fail");

        assert_eq!(error.status, StatusCode::NOT_FOUND.as_u16());

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
    async fn submit_match_rejects_duplicate_player_without_storing_match() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = create_player(db.pool(), "Alice").await;
        let mut duplicate_player_request = request(&[alice]);
        duplicate_player_request.placements.push(PlacementRequest {
            player_id: alice,
            placement: 2,
        });

        let error = submit_match(&state, &actor, duplicate_player_request)
            .await
            .expect_err("duplicate player should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST.as_u16());
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(db.pool())
            .await
            .expect("match count should load");
        let games_played: i32 =
            sqlx::query_scalar("SELECT games_played FROM player_ratings WHERE player_id = $1")
                .bind(alice)
                .fetch_one(db.pool())
                .await
                .expect("rating should load");
        assert_eq!(match_count, 0);
        assert_eq!(games_played, 0);

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

    #[tokio::test]
    async fn submit_match_rolls_back_when_rating_update_fails() {
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
        sqlx::query(
            r#"
            UPDATE player_ratings
            SET games_played = 2147483647,
                wins = 2147483647,
                losses = 0,
                total_placement = 2147483647
            WHERE player_id = $1
            "#,
        )
        .bind(alice)
        .execute(db.pool())
        .await
        .expect("rating should be moved to overflow edge");

        let error = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect_err("rating stat overflow should fail");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(db.pool())
            .await
            .expect("match count should load");
        let participant_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM match_players")
            .fetch_one(db.pool())
            .await
            .expect("participant count should load");
        let ben_games_played: i32 =
            sqlx::query_scalar("SELECT games_played FROM player_ratings WHERE player_id = $1")
                .bind(ben)
                .fetch_one(db.pool())
                .await
                .expect("rating should load");
        assert_eq!(match_count, 0);
        assert_eq!(participant_count, 0);
        assert_eq!(ben_games_played, 0);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn submit_match_rolls_back_when_audit_insert_fails() {
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
        sqlx::query(
            r#"
            CREATE FUNCTION test_fail_audit_insert()
            RETURNS trigger
            LANGUAGE plpgsql
            AS $$
            BEGIN
                RAISE EXCEPTION 'forced audit failure';
                RETURN NEW;
            END
            $$
            "#,
        )
        .execute(db.pool())
        .await
        .expect("audit failure function should create");
        sqlx::query(
            r#"
            CREATE TRIGGER test_fail_audit_insert
            BEFORE INSERT ON audit_log
            FOR EACH ROW EXECUTE FUNCTION test_fail_audit_insert()
            "#,
        )
        .execute(db.pool())
        .await
        .expect("audit failure trigger should create");

        let error = submit_match(&state, &actor, request(&[alice, ben]))
            .await
            .expect_err("audit insert should fail");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(db.pool())
            .await
            .expect("match count should load");
        let participant_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM match_players")
            .fetch_one(db.pool())
            .await
            .expect("participant count should load");
        let alice_games_played: i32 =
            sqlx::query_scalar("SELECT games_played FROM player_ratings WHERE player_id = $1")
                .bind(alice)
                .fetch_one(db.pool())
                .await
                .expect("rating should load");
        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(db.pool())
            .await
            .expect("audit count should load");
        assert_eq!(match_count, 0);
        assert_eq!(participant_count, 0);
        assert_eq!(alice_games_played, 0);
        assert_eq!(audit_count, 0);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
