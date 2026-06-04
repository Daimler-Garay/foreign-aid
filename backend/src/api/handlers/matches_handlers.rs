use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::{
    api::error::ApiError,
    application::{auth::permissions::AdminUser, services::match_service, state::SharedState},
    domain::models::matches::CreateMatchRequest,
};

pub async fn submit_match_handler(
    State(state): State<SharedState>,
    admin: AdminUser,
    Json(request): Json<CreateMatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let response = match_service::submit_match(&state, &admin.into_inner(), request).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{Json, body::to_bytes, extract::State, response::IntoResponse};
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{
            config::Config,
            repositories::{player_repo, user_repo},
            state::AppState,
        },
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::{
            auth::{AuthenticatedUser, UserRole},
            matches::PlacementRequest,
        },
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

    async fn admin_user(pool: &crate::db::DatabasePool) -> AuthenticatedUser {
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

    fn request(player_ids: &[Uuid]) -> CreateMatchRequest {
        CreateMatchRequest {
            played_at: Utc::now() - Duration::minutes(1),
            notes: Some("handler test".to_owned()),
            placements: player_ids
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
    async fn admin_can_submit_match() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let admin = admin_user(db.pool()).await;
        let alice = player(db.pool(), "Alice").await;
        let ben = player(db.pool(), "Ben").await;

        let response =
            submit_match_handler(State(state), AdminUser(admin), Json(request(&[alice, ben])))
                .await
                .expect("match should submit")
                .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(value["status"], "confirmed");
        assert!(value["match_id"].as_str().is_some());
        assert_eq!(value["rating_changes"].as_array().unwrap().len(), 2);
        assert_eq!(value["rating_changes"][0]["placement"], 1);
        assert!(value["rating_changes"][0]["old_display_rating"].is_i64());
        assert!(value["rating_changes"][0]["new_display_rating"].is_i64());
        assert!(value["rating_changes"][0]["display_delta"].is_i64());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn match_submission_rejects_invalid_request() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let admin = admin_user(db.pool()).await;
        let alice = player(db.pool(), "Alice").await;
        let mut invalid = request(&[alice]);
        invalid.placements.push(PlacementRequest {
            player_id: alice,
            placement: 2,
        });

        let error = match submit_match_handler(State(state), AdminUser(admin), Json(invalid)).await
        {
            Ok(_) => panic!("duplicate player should fail"),
            Err(error) => error,
        };
        let response = error.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["error"]["code"], "validation_error");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
