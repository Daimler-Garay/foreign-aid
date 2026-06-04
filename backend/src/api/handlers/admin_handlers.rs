use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::{
    api::error::ApiError,
    application::{auth::permissions::AdminUser, services::replay_service, state::SharedState},
    domain::models::recalculation::RecalculateRatingsRequest,
};

pub async fn recalculate_ratings_handler(
    State(state): State<SharedState>,
    admin: AdminUser,
    Json(request): Json<RecalculateRatingsRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let response =
        replay_service::recalculate_ratings(&state, &admin.into_inner(), request).await?;

    Ok((StatusCode::OK, Json(response)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{Json, body::to_bytes, extract::State, response::IntoResponse};
    use uuid::Uuid;

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

    #[tokio::test]
    async fn admin_can_recalculate_ratings() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let admin = admin_user(db.pool()).await;

        let response = recalculate_ratings_handler(
            State(state),
            AdminUser(admin),
            Json(RecalculateRatingsRequest {
                reason: "handler test".to_owned(),
            }),
        )
        .await
        .expect("recalculation should succeed")
        .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["status"], "succeeded");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
