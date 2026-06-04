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
        state::SharedState,
    },
    domain::{
        models::{
            auth::AuthenticatedUser,
            players::{
                CreatePlayerRequest, Player, PlayerRating, PlayerRatingSummary, PlayerResponse,
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

fn player_response(player: Player, rating: PlayerRating) -> PlayerResponse {
    PlayerResponse {
        id: player.id,
        display_name: player.display_name,
        active: player.active,
        rating: PlayerRatingSummary {
            display_rating: (rating.rating * 40.0).round() as i32,
            rank_score: ((rating.rating - (3.0 * rating.uncertainty)) * 40.0).round() as i32,
            games_played: rating.games_played,
            wins: rating.wins,
            losses: rating.losses,
        },
    }
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
}
