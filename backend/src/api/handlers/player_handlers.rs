use axum::extract::{Path, Query};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use uuid::Uuid;

use crate::{
    api::error::ApiError,
    application::{auth::permissions::AdminUser, services::player_service, state::SharedState},
    domain::models::players::{CreatePlayerRequest, ListPlayersQuery},
};

pub async fn list_players_handler(
    State(state): State<SharedState>,
    Query(query): Query<ListPlayersQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let players = player_service::list_players(&state, query).await?;

    Ok(Json(players))
}

pub async fn create_player_handler(
    State(state): State<SharedState>,
    admin: AdminUser,
    Json(request): Json<CreatePlayerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let player = player_service::create_player(&state, &admin.into_inner(), request).await?;

    Ok((StatusCode::CREATED, Json(player)))
}

pub async fn get_player_handler(
    State(state): State<SharedState>,
    Path(player_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let player = player_service::get_player(&state, player_id).await?;

    Ok(Json(player))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{Json, extract::State, response::IntoResponse};

    use super::*;
    use crate::{
        application::{config::Config, repositories::user_repo, state::AppState},
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::auth::{AuthenticatedUser, UserRole},
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

    #[tokio::test]
    async fn admin_can_create_player() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        user_repo::insert_user(db.pool(), user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("admin should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let response = create_player_handler(
            State(state),
            AdminUser(AuthenticatedUser {
                id: user_id,
                username: "admin".to_owned(),
                role: "admin".to_owned(),
                active: true,
                player_id: None,
                session_id: Uuid::new_v4(),
            }),
            Json(CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            }),
        )
        .await
        .expect("player should create")
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn list_players_returns_success() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        user_repo::insert_user(db.pool(), user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("admin should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        create_player_handler(
            State(state.clone()),
            AdminUser(AuthenticatedUser {
                id: user_id,
                username: "admin".to_owned(),
                role: "admin".to_owned(),
                active: true,
                player_id: None,
                session_id: Uuid::new_v4(),
            }),
            Json(CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            }),
        )
        .await
        .expect("player should create");

        let response = list_players_handler(
            State(state),
            Query(ListPlayersQuery {
                include_inactive: None,
            }),
        )
        .await
        .expect("players should list")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn get_player_returns_success() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        user_repo::insert_user(db.pool(), user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("admin should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let created = player_service::create_player(
            &state,
            &AuthenticatedUser {
                id: user_id,
                username: "admin".to_owned(),
                role: "admin".to_owned(),
                active: true,
                player_id: None,
                session_id: Uuid::new_v4(),
            },
            CreatePlayerRequest {
                display_name: "Alice".to_owned(),
            },
        )
        .await
        .expect("player should create");

        let response = get_player_handler(State(state), Path(created.id))
            .await
            .expect("player should load")
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
