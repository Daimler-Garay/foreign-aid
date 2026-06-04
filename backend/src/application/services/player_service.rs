use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::{
    api::error::ApiError,
    application::{
        repositories::{
            audit_repo::{self, NewAuditLogEntry},
            player_repo,
        },
        services::rating_service::{conservative_rank_score, display_rating},
        state::SharedState,
    },
    domain::{
        models::{
            auth::AuthenticatedUser,
            players::{
                CreatePlayerRequest, ListPlayersQuery, Player, PlayerRating, PlayerRatingSummary,
                PlayerResponse, PlayerWithRating, UpdatePlayerRequest,
            },
        },
        validation::{ValidationError, player_validation::validate_display_name},
    },
};

pub async fn create_player(
    state: &SharedState,
    actor: &AuthenticatedUser,
    request: CreatePlayerRequest,
) -> Result<PlayerResponse, ApiError> {
    validate_display_name(&request.display_name).map_err(validation_error)?;
    let display_name = request.display_name.trim();
    let player_id = Uuid::new_v4();

    let mut tx = state.db_pool.begin().await?;

    let player = player_repo::insert_player(&mut *tx, player_id, display_name, None)
        .await
        .map_err(player_write_error)?;
    let rating = player_repo::insert_default_rating(&mut *tx, player.id).await?;

    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "player.created".to_owned(),
            entity_type: "player".to_owned(),
            entity_id: Some(player.id),
            old_value: None,
            new_value: Some(json!({
                "id": player.id,
                "display_name": player.display_name,
                "active": player.active,
            })),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(player_response(player, rating))
}

pub async fn list_players(
    state: &SharedState,
    query: ListPlayersQuery,
) -> Result<Vec<PlayerResponse>, ApiError> {
    let players = player_repo::list_players_with_ratings(
        &state.db_pool,
        query.include_inactive.unwrap_or(false),
    )
    .await?;

    Ok(players
        .into_iter()
        .map(player_with_rating_response)
        .collect())
}

pub async fn get_player(state: &SharedState, player_id: Uuid) -> Result<PlayerResponse, ApiError> {
    let player = player_repo::find_player_with_rating(&state.db_pool, player_id)
        .await?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player not found."))?;

    Ok(player_with_rating_response(player))
}

pub async fn update_player(
    state: &SharedState,
    actor: &AuthenticatedUser,
    player_id: Uuid,
    request: UpdatePlayerRequest,
) -> Result<PlayerResponse, ApiError> {
    if let Some(display_name) = &request.display_name {
        validate_display_name(display_name).map_err(validation_error)?;
    }
    let display_name = request.display_name.as_deref().map(str::trim);

    let mut tx = state.db_pool.begin().await?;
    let before = player_repo::find_player_with_rating_for_update(&mut *tx, player_id)
        .await?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player not found."))?;
    let updated = player_repo::update_player(&mut *tx, player_id, display_name, request.active)
        .await
        .map_err(player_write_error)?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player not found."))?;
    let rating = rating_from_player_with_rating(&before);

    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "player.updated".to_owned(),
            entity_type: "player".to_owned(),
            entity_id: Some(updated.id),
            old_value: Some(player_audit_value(&before.display_name, before.active)),
            new_value: Some(player_audit_value(&updated.display_name, updated.active)),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(player_response(updated, rating))
}

pub async fn deactivate_player(
    state: &SharedState,
    actor: &AuthenticatedUser,
    player_id: Uuid,
) -> Result<(), ApiError> {
    let mut tx = state.db_pool.begin().await?;
    let before = player_repo::find_player_with_rating_for_update(&mut *tx, player_id)
        .await?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player not found."))?;
    let updated = player_repo::update_player(&mut *tx, player_id, None, Some(false))
        .await?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player not found."))?;

    audit_repo::insert_audit_log_entry(
        &mut *tx,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: Some(actor.id),
            action: "player.deactivated".to_owned(),
            entity_type: "player".to_owned(),
            entity_id: Some(updated.id),
            old_value: Some(player_audit_value(&before.display_name, before.active)),
            new_value: Some(player_audit_value(&updated.display_name, updated.active)),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

fn player_response(player: Player, rating: PlayerRating) -> PlayerResponse {
    PlayerResponse {
        id: player.id,
        display_name: player.display_name,
        active: player.active,
        rating: PlayerRatingSummary {
            display_rating: display_rating(rating.rating),
            rank_score: conservative_rank_score(rating.rating, rating.uncertainty),
            games_played: rating.games_played,
            wins: rating.wins,
            losses: rating.losses,
        },
    }
}

fn player_with_rating_response(player: PlayerWithRating) -> PlayerResponse {
    PlayerResponse {
        id: player.id,
        display_name: player.display_name,
        active: player.active,
        rating: PlayerRatingSummary {
            display_rating: display_rating(player.rating),
            rank_score: conservative_rank_score(player.rating, player.uncertainty),
            games_played: player.games_played,
            wins: player.wins,
            losses: player.losses,
        },
    }
}

fn rating_from_player_with_rating(player: &PlayerWithRating) -> PlayerRating {
    PlayerRating {
        player_id: player.id,
        rating: player.rating,
        uncertainty: player.uncertainty,
        games_played: player.games_played,
        wins: player.wins,
        losses: player.losses,
        total_placement: player.total_placement,
        last_played_at: player.last_played_at,
        updated_at: player.updated_at,
    }
}

fn player_audit_value(display_name: &str, active: bool) -> serde_json::Value {
    json!({
        "display_name": display_name,
        "active": active,
    })
}

fn validation_error(error: ValidationError) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "validation_error",
        error.to_string(),
    )
}

fn player_write_error(error: sqlx::Error) -> ApiError {
    if is_unique_violation(&error) {
        ApiError::new(
            StatusCode::CONFLICT,
            "duplicate_display_name",
            "A player with that display name already exists.",
        )
    } else {
        ApiError::from(error)
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db_error| db_error.code())
        .as_deref()
        == Some("23505")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        application::{config::Config, repositories::user_repo, state::AppState},
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
            .expect("admin user should insert");

        AuthenticatedUser {
            id: user_id,
            username: "admin".to_owned(),
            role: "admin".to_owned(),
            active: true,
            player_id: None,
            session_id: Uuid::new_v4(),
        }
    }

    #[tokio::test]
    async fn create_player_creates_default_rating_and_audit_entry() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        let response = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: " Alice ".to_owned(),
            },
        )
        .await
        .expect("player should create");

        assert_eq!(response.display_name, "Alice");
        assert!(response.active);
        assert_eq!(response.rating.display_rating, 1000);
        assert_eq!(response.rating.rank_score, 0);
        assert_eq!(response.rating.games_played, 0);

        let rating_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM player_ratings WHERE player_id = $1")
                .bind(response.id)
                .fetch_one(db.pool())
                .await
                .expect("rating count should load");
        assert_eq!(rating_count, 1);

        let audit_action: String =
            sqlx::query_scalar("SELECT action FROM audit_log WHERE entity_id = $1")
                .bind(response.id)
                .fetch_one(db.pool())
                .await
                .expect("audit action should load");
        assert_eq!(audit_action, "player.created");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn create_player_rejects_duplicate_display_name() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("first player should create");
        let error = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect_err("duplicate name should fail");

        assert_eq!(error.status, StatusCode::CONFLICT.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn create_player_rejects_blank_display_name() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        let error = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: " ".to_owned(),
            },
        )
        .await
        .expect_err("blank display name should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn list_players_excludes_inactive_by_default() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        let alice = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("alice should create");
        let ben = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Ben".to_owned(),
            },
        )
        .await
        .expect("ben should create");
        player_repo::set_player_active(db.pool(), ben.id, false)
            .await
            .expect("ben should deactivate");

        let players = list_players(
            &state,
            ListPlayersQuery {
                include_inactive: None,
            },
        )
        .await
        .expect("players should list");

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].id, alice.id);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn list_players_can_include_inactive() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;

        let alice = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("alice should create");
        let ben = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Ben".to_owned(),
            },
        )
        .await
        .expect("ben should create");
        player_repo::set_player_active(db.pool(), ben.id, false)
            .await
            .expect("ben should deactivate");

        let players = list_players(
            &state,
            ListPlayersQuery {
                include_inactive: Some(true),
            },
        )
        .await
        .expect("players should list");

        assert_eq!(players.len(), 2);
        assert_eq!(players[0].id, alice.id);
        assert_eq!(players[1].id, ben.id);
        assert!(!players[1].active);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn get_player_returns_rating_summary() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let created = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("player should create");

        let player = get_player(&state, created.id)
            .await
            .expect("player should load");

        assert_eq!(player.id, created.id);
        assert_eq!(player.display_name, "Alice");
        assert_eq!(player.rating.display_rating, 1000);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn get_player_rejects_missing_player() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let error = get_player(&state, Uuid::new_v4())
            .await
            .expect_err("missing player should fail");

        assert_eq!(error.status, StatusCode::NOT_FOUND.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn update_player_changes_display_name_and_writes_audit_entry() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let created = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("player should create");

        let updated = update_player(
            &state,
            &actor,
            created.id,
            UpdatePlayerRequest {
                display_name: Some(" Alicia ".to_owned()),
                active: None,
            },
        )
        .await
        .expect("player should update");

        assert_eq!(updated.display_name, "Alicia");
        assert!(updated.active);

        let audit_action: String = sqlx::query_scalar(
            "SELECT action FROM audit_log WHERE entity_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(created.id)
        .fetch_one(db.pool())
        .await
        .expect("audit action should load");
        assert_eq!(audit_action, "player.updated");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn update_player_rejects_duplicate_display_name() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let alice = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("alice should create");
        create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Ben".to_owned(),
            },
        )
        .await
        .expect("ben should create");

        let error = update_player(
            &state,
            &actor,
            alice.id,
            UpdatePlayerRequest {
                display_name: Some("Ben".to_owned()),
                active: None,
            },
        )
        .await
        .expect_err("duplicate name should fail");

        assert_eq!(error.status, StatusCode::CONFLICT.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn update_player_rejects_blank_display_name() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let created = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("player should create");

        let error = update_player(
            &state,
            &actor,
            created.id,
            UpdatePlayerRequest {
                display_name: Some(" ".to_owned()),
                active: None,
            },
        )
        .await
        .expect_err("blank name should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn deactivate_player_soft_deletes_and_writes_audit_entry() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let actor = admin_actor(db.pool()).await;
        let created = create_player(
            &state,
            &actor,
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("player should create");

        deactivate_player(&state, &actor, created.id)
            .await
            .expect("player should deactivate");

        let player = get_player(&state, created.id)
            .await
            .expect("player should still exist");
        assert!(!player.active);

        let audit_action: String = sqlx::query_scalar(
            "SELECT action FROM audit_log WHERE entity_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(created.id)
        .fetch_one(db.pool())
        .await
        .expect("audit action should load");
        assert_eq!(audit_action, "player.deactivated");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
