use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    api::error::ApiError,
    application::{
        auth::permissions::AdminUser,
        services::{audit_service, replay_service},
        state::SharedState,
    },
    domain::models::{audit::AuditLogQuery, recalculation::RecalculateRatingsRequest},
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

pub async fn list_audit_log_handler(
    State(state): State<SharedState>,
    _admin: AdminUser,
    Query(query): Query<AuditLogQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let response = audit_service::list_audit_log(&state, query).await?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{Json, body::to_bytes, extract::Query, extract::State, response::IntoResponse};
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

    #[tokio::test]
    async fn admin_can_view_audit_log_without_secrets() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let admin = admin_user(db.pool()).await;
        crate::application::repositories::audit_repo::insert_audit_log_entry(
            db.pool(),
            crate::application::repositories::audit_repo::NewAuditLogEntry {
                id: Uuid::new_v4(),
                actor_user_id: Some(admin.id),
                action: "test.secret".to_owned(),
                entity_type: "test".to_owned(),
                entity_id: None,
                old_value: None,
                new_value: Some(serde_json::json!({
                    "username": "admin",
                    "password_hash": "hash"
                })),
            },
        )
        .await
        .expect("audit should insert");

        let response = list_audit_log_handler(
            State(state),
            AdminUser(admin),
            Query(AuditLogQuery { limit: Some(10) }),
        )
        .await
        .expect("audit log should list")
        .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let body = String::from_utf8(body.to_vec()).expect("body should be utf8");

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("test.secret"));
        assert!(!body.contains("hash"));
        assert!(!body.contains("password_hash"));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
