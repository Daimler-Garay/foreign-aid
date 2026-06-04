use std::collections::{HashMap, HashSet};

use axum::http::StatusCode;
use chrono::Utc;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    api::error::ApiError,
    application::{
        repositories::{
            audit_repo::{self, NewAuditLogEntry},
            match_repo::{self, NewMatch, NewMatchPlayer},
            rating_repo, recalculation_repo,
        },
        services::{
            match_service::{RATING_ALGORITHM, RATING_ALGORITHM_VERSION},
            rating_service::{
                DEFAULT_RATING, DEFAULT_UNCERTAINTY, RatingInput, rate_ranked_free_for_all,
            },
        },
        state::SharedState,
    },
    domain::{
        models::{
            auth::AuthenticatedUser,
            matches::{
                CorrectMatchResponse, CreateMatchRequest, MatchStatus, PlacementRequest,
                VoidMatchResponse,
            },
            recalculation::{RecalculateRatingsRequest, RecalculateRatingsResponse},
        },
        validation::{
            ValidationError,
            match_validation::{validate_match_submission, validate_placements},
        },
    },
};

pub async fn recalculate_ratings(
    state: &SharedState,
    actor: &AuthenticatedUser,
    request: RecalculateRatingsRequest,
) -> Result<RecalculateRatingsResponse, ApiError> {
    let reason = request.reason.trim();
    if reason.is_empty() {
        return Err(validation_error("reason must not be blank"));
    }

    let run_id = Uuid::new_v4();
    recalculation_repo::insert_recalculation_run(&state.db_pool, run_id, Some(actor.id), reason)
        .await?;

    let result = run_recalculation_transaction(state, actor, run_id).await;
    if let Err(error) = result {
        let _ = recalculation_repo::finish_recalculation_run(
            &state.db_pool,
            run_id,
            crate::domain::models::recalculation::RecalculationStatus::Failed,
            Utc::now(),
            Some("rating recalculation failed"),
        )
        .await;
        return Err(error);
    }

    Ok(RecalculateRatingsResponse {
        run_id,
        status: "succeeded".to_owned(),
    })
}

pub async fn void_match(
    state: &SharedState,
    actor: &AuthenticatedUser,
    match_id: Uuid,
) -> Result<VoidMatchResponse, ApiError> {
    let mut tx = state.db_pool.begin().await?;
    rating_repo::acquire_rating_mutation_lock(&mut *tx).await?;

    let existing = match_repo::find_match_by_id_for_update(&mut *tx, match_id)
        .await?
        .ok_or_else(|| ApiError::not_found("match_not_found", "Match was not found."))?;
    if existing.status != MatchStatus::Confirmed.as_str() {
        return Err(conflict_error("Only confirmed matches can be voided."));
    }

    let voided = match_repo::set_match_status(&mut *tx, match_id, MatchStatus::Voided)
        .await?
        .expect("locked existing match should update");

    replay_confirmed_matches(&mut tx).await?;
    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "match.voided".to_owned(),
            entity_type: "match".to_owned(),
            entity_id: Some(match_id),
            old_value: Some(json!({ "status": existing.status })),
            new_value: Some(json!({ "status": voided.status })),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(VoidMatchResponse {
        match_id,
        status: MatchStatus::Voided.as_str().to_owned(),
    })
}

pub async fn correct_match(
    state: &SharedState,
    actor: &AuthenticatedUser,
    original_match_id: Uuid,
    request: CreateMatchRequest,
) -> Result<CorrectMatchResponse, ApiError> {
    validate_placements(&request.placements).map_err(validation_from_validation_error)?;

    let player_ids = request
        .placements
        .iter()
        .map(|placement| placement.player_id)
        .collect::<Vec<_>>();
    let mut tx = state.db_pool.begin().await?;
    rating_repo::acquire_rating_mutation_lock(&mut *tx).await?;

    let original = match_repo::find_match_by_id_for_update(&mut *tx, original_match_id)
        .await?
        .ok_or_else(|| ApiError::not_found("match_not_found", "Match was not found."))?;
    if original.status != MatchStatus::Confirmed.as_str() {
        return Err(conflict_error("Only confirmed matches can be corrected."));
    }

    let active_ratings =
        rating_repo::load_active_player_ratings_for_update(&mut *tx, &player_ids).await?;
    let active_player_ids = active_ratings
        .iter()
        .map(|rating| rating.player_id)
        .collect::<HashSet<_>>();
    validate_match_submission(
        request.played_at,
        &request.placements,
        &active_player_ids,
        Utc::now(),
    )
    .map_err(validation_from_validation_error)?;

    match_repo::set_match_status(&mut *tx, original_match_id, MatchStatus::Voided)
        .await?
        .expect("locked original match should update");

    let corrected_match_id = Uuid::new_v4();
    match_repo::insert_confirmed_match(
        &mut *tx,
        NewMatch {
            id: corrected_match_id,
            played_at: request.played_at,
            submitted_by_user_id: Some(actor.id),
            notes: request.notes.clone(),
            rating_algorithm: RATING_ALGORITHM.to_owned(),
            rating_algorithm_version: RATING_ALGORITHM_VERSION,
            corrected_from_match_id: Some(original_match_id),
        },
    )
    .await?;

    for placement in &request.placements {
        match_repo::insert_match_player(
            &mut *tx,
            NewMatchPlayer {
                match_id: corrected_match_id,
                player_id: placement.player_id,
                placement: placement.placement,
                old_rating: DEFAULT_RATING,
                old_uncertainty: DEFAULT_UNCERTAINTY,
                new_rating: DEFAULT_RATING,
                new_uncertainty: DEFAULT_UNCERTAINTY,
                rating_delta: 0.0,
            },
        )
        .await?;
    }

    replay_confirmed_matches(&mut tx).await?;
    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "match.corrected".to_owned(),
            entity_type: "match".to_owned(),
            entity_id: Some(corrected_match_id),
            old_value: Some(json!({ "id": original_match_id, "status": original.status })),
            new_value: Some(json!({
                "id": corrected_match_id,
                "corrected_from_match_id": original_match_id,
                "placements": request.placements,
            })),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(CorrectMatchResponse {
        original_match_id,
        corrected_match_id,
        status: MatchStatus::Confirmed.as_str().to_owned(),
    })
}

async fn run_recalculation_transaction(
    state: &SharedState,
    actor: &AuthenticatedUser,
    run_id: Uuid,
) -> Result<(), ApiError> {
    let mut tx = state.db_pool.begin().await?;
    rating_repo::acquire_rating_mutation_lock(&mut *tx).await?;
    replay_confirmed_matches(&mut tx).await?;
    recalculation_repo::finish_recalculation_run(
        &mut *tx,
        run_id,
        crate::domain::models::recalculation::RecalculationStatus::Succeeded,
        Utc::now(),
        None,
    )
    .await?;
    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "ratings.recalculated".to_owned(),
            entity_type: "rating_recalculation_run".to_owned(),
            entity_id: Some(run_id),
            old_value: None,
            new_value: Some(json!({ "status": "succeeded" })),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(())
}

async fn replay_confirmed_matches(tx: &mut Transaction<'_, Postgres>) -> Result<(), ApiError> {
    rating_repo::reset_all_player_ratings(&mut **tx, DEFAULT_RATING, DEFAULT_UNCERTAINTY).await?;

    let matches = match_repo::list_confirmed_matches_for_replay(&mut **tx).await?;
    for match_result in matches {
        let participants =
            match_repo::list_match_participants_for_update(&mut **tx, match_result.id).await?;
        let placements = participants
            .iter()
            .map(|participant| PlacementRequest {
                player_id: participant.player_id,
                placement: participant.placement,
            })
            .collect::<Vec<_>>();
        validate_placements(&placements).map_err(validation_from_validation_error)?;

        let player_ids = participants
            .iter()
            .map(|participant| participant.player_id)
            .collect::<Vec<_>>();
        let current_ratings =
            rating_repo::load_player_ratings_for_update(&mut **tx, &player_ids).await?;
        let current_ratings = current_ratings
            .into_iter()
            .map(|rating| (rating.player_id, rating))
            .collect::<HashMap<_, _>>();

        let rating_inputs = participants
            .iter()
            .map(|participant| {
                let rating = current_ratings
                    .get(&participant.player_id)
                    .ok_or_else(|| validation_error("match participant is missing rating state"))?;

                Ok(RatingInput {
                    player_id: participant.player_id,
                    rating: rating.rating,
                    uncertainty: rating.uncertainty,
                    placement: participant.placement,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        let rating_updates = rate_ranked_free_for_all(&rating_inputs).map_err(|error| {
            ApiError::new(StatusCode::BAD_REQUEST, "rating_error", error.to_string())
        })?;

        for update in rating_updates {
            match_repo::update_match_player_snapshot(
                &mut **tx,
                NewMatchPlayer {
                    match_id: match_result.id,
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
                &mut **tx,
                update.player_id,
                update.new_rating,
                update.new_uncertainty,
                update.placement,
                match_result.played_at,
            )
            .await?;
        }
    }

    Ok(())
}

fn validation_from_validation_error(error: ValidationError) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "validation_error",
        error.to_string(),
    )
}

fn validation_error(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "validation_error", message)
}

fn conflict_error(message: &str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, "conflict", message)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{
            config::Config,
            repositories::{player_repo, user_repo},
            services::match_service,
            state::AppState,
        },
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::auth::UserRole,
    };

    #[derive(Debug, PartialEq, sqlx::FromRow)]
    struct RatingSnapshot {
        player_id: Uuid,
        rating: f64,
        uncertainty: f64,
        games_played: i32,
        wins: i32,
        losses: i32,
        total_placement: i32,
    }

    #[derive(Debug, PartialEq, sqlx::FromRow)]
    struct MatchPlayerSnapshot {
        match_id: Uuid,
        player_id: Uuid,
        placement: i32,
        old_rating: f64,
        old_uncertainty: f64,
        new_rating: f64,
        new_uncertainty: f64,
        rating_delta: f64,
    }

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

    async fn player(pool: &crate::db::DatabasePool, display_name: &str) -> Uuid {
        let player_id = Uuid::new_v4();
        player_repo::insert_player(pool, player_id, display_name, None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");

        player_id
    }

    fn request_with_time(players: &[Uuid], minutes_ago: i64) -> CreateMatchRequest {
        CreateMatchRequest {
            played_at: Utc::now() - Duration::minutes(minutes_ago),
            notes: Some("replay test".to_owned()),
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

    async fn rating_snapshots(pool: &crate::db::DatabasePool) -> Vec<RatingSnapshot> {
        sqlx::query_as::<_, RatingSnapshot>(
            r#"
            SELECT player_id, rating, uncertainty, games_played, wins, losses, total_placement
            FROM player_ratings
            ORDER BY player_id ASC
            "#,
        )
        .fetch_all(pool)
        .await
        .expect("rating snapshots should load")
    }

    async fn match_player_snapshots(pool: &crate::db::DatabasePool) -> Vec<MatchPlayerSnapshot> {
        sqlx::query_as::<_, MatchPlayerSnapshot>(
            r#"
            SELECT match_id, player_id, placement, old_rating, old_uncertainty,
                   new_rating, new_uncertainty, rating_delta
            FROM match_players
            ORDER BY match_id ASC, player_id ASC
            "#,
        )
        .fetch_all(pool)
        .await
        .expect("match player snapshots should load")
    }

    async fn old_rating_for(
        pool: &crate::db::DatabasePool,
        match_id: Uuid,
        player_id: Uuid,
    ) -> f64 {
        sqlx::query_scalar(
            r#"
            SELECT old_rating
            FROM match_players
            WHERE match_id = $1
              AND player_id = $2
            "#,
        )
        .bind(match_id)
        .bind(player_id)
        .fetch_one(pool)
        .await
        .expect("old rating should load")
    }

    #[tokio::test]
    async fn replay_is_deterministic() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = player(db.pool(), "Alice").await;
        let ben = player(db.pool(), "Ben").await;
        let chloe = player(db.pool(), "Chloe").await;
        match_service::submit_match(&state, &actor, request_with_time(&[alice, ben], 10))
            .await
            .expect("first match should submit");
        match_service::submit_match(&state, &actor, request_with_time(&[chloe, alice], 5))
            .await
            .expect("second match should submit");

        recalculate_ratings(
            &state,
            &actor,
            RecalculateRatingsRequest {
                reason: "first replay".to_owned(),
            },
        )
        .await
        .expect("first replay should succeed");
        let ratings_after_first = rating_snapshots(db.pool()).await;
        let match_players_after_first = match_player_snapshots(db.pool()).await;

        recalculate_ratings(
            &state,
            &actor,
            RecalculateRatingsRequest {
                reason: "second replay".to_owned(),
            },
        )
        .await
        .expect("second replay should succeed");
        let ratings_after_second = rating_snapshots(db.pool()).await;
        let match_players_after_second = match_player_snapshots(db.pool()).await;

        assert_eq!(ratings_after_first, ratings_after_second);
        assert_eq!(match_players_after_first, match_players_after_second);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn voiding_match_replays_later_rating_snapshots() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = player(db.pool(), "Alice").await;
        let ben = player(db.pool(), "Ben").await;
        let chloe = player(db.pool(), "Chloe").await;
        let first =
            match_service::submit_match(&state, &actor, request_with_time(&[alice, ben], 10))
                .await
                .expect("first match should submit");
        let later =
            match_service::submit_match(&state, &actor, request_with_time(&[alice, chloe], 5))
                .await
                .expect("later match should submit");
        let old_snapshot = old_rating_for(db.pool(), later.match_id, alice).await;

        let response = void_match(&state, &actor, first.match_id)
            .await
            .expect("match should void");

        let new_snapshot = old_rating_for(db.pool(), later.match_id, alice).await;
        let alice_games: i32 =
            sqlx::query_scalar("SELECT games_played FROM player_ratings WHERE player_id = $1")
                .bind(alice)
                .fetch_one(db.pool())
                .await
                .expect("games should load");
        assert_eq!(response.status, "voided");
        assert_ne!(old_snapshot, new_snapshot);
        assert_eq!(new_snapshot, DEFAULT_RATING);
        assert_eq!(alice_games, 1);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn correcting_match_replays_later_rating_snapshots() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = player(db.pool(), "Alice").await;
        let ben = player(db.pool(), "Ben").await;
        let chloe = player(db.pool(), "Chloe").await;
        let first =
            match_service::submit_match(&state, &actor, request_with_time(&[alice, ben], 10))
                .await
                .expect("first match should submit");
        let later =
            match_service::submit_match(&state, &actor, request_with_time(&[alice, chloe], 5))
                .await
                .expect("later match should submit");
        let old_snapshot = old_rating_for(db.pool(), later.match_id, alice).await;

        let corrected = correct_match(
            &state,
            &actor,
            first.match_id,
            request_with_time(&[ben, alice], 10),
        )
        .await
        .expect("match should correct");

        let new_snapshot = old_rating_for(db.pool(), later.match_id, alice).await;
        let original_status: String =
            sqlx::query_scalar("SELECT status FROM matches WHERE id = $1")
                .bind(first.match_id)
                .fetch_one(db.pool())
                .await
                .expect("status should load");
        let corrected_from: Option<Uuid> =
            sqlx::query_scalar("SELECT corrected_from_match_id FROM matches WHERE id = $1")
                .bind(corrected.corrected_match_id)
                .fetch_one(db.pool())
                .await
                .expect("correction link should load");

        assert_ne!(old_snapshot, new_snapshot);
        assert_eq!(original_status, "voided");
        assert_eq!(corrected_from, Some(first.match_id));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn recalculation_tracks_successful_run() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        let response = recalculate_ratings(
            &state,
            &actor,
            RecalculateRatingsRequest {
                reason: "manual test".to_owned(),
            },
        )
        .await
        .expect("recalculation should succeed");

        let status: String =
            sqlx::query_scalar("SELECT status FROM rating_recalculation_runs WHERE id = $1")
                .bind(response.run_id)
                .fetch_one(db.pool())
                .await
                .expect("run status should load");
        assert_eq!(status, "succeeded");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
