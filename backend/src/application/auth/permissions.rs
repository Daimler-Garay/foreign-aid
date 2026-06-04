use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use crate::{
    api::error::ApiError,
    application::state::SharedState,
    domain::models::auth::{AuthenticatedUser, UserRole},
};

#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthenticatedUser);

impl AdminUser {
    pub fn into_inner(self) -> AuthenticatedUser {
        self.0
    }
}

pub fn require_admin(user: AuthenticatedUser) -> Result<AdminUser, ApiError> {
    if user.role == UserRole::Admin.as_str() {
        Ok(AdminUser(user))
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin access is required.",
        ))
    }
}

impl FromRequestParts<SharedState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        require_admin(user)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::Body, http::Request};
    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{
            config::Config,
            repositories::{session_repo, user_repo},
            state::AppState,
        },
        db::{Database, DatabaseOptions, options::PostgresOptions},
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

    fn user_with_role(role: UserRole) -> AuthenticatedUser {
        AuthenticatedUser {
            id: Uuid::new_v4(),
            username: "user".to_owned(),
            role: role.as_str().to_owned(),
            active: true,
            player_id: None,
            session_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn require_admin_accepts_admin_user() {
        let user = user_with_role(UserRole::Admin);

        let admin = require_admin(user).expect("admin should be authorized");

        assert_eq!(admin.into_inner().role, "admin");
    }

    #[test]
    fn require_admin_rejects_player_user() {
        let user = user_with_role(UserRole::Player);

        let error = require_admin(user).expect_err("player should be forbidden");

        assert_eq!(error.status, StatusCode::FORBIDDEN.as_u16());
    }

    #[tokio::test]
    async fn admin_extractor_rejects_anonymous_request() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let request = Request::builder()
            .uri("/admin-only")
            .body(Body::empty())
            .expect("request should build");
        let (mut parts, _) = request.into_parts();

        let error = match AdminUser::from_request_parts(&mut parts, &state).await {
            Ok(_) => panic!("anonymous request should fail"),
            Err(error) => error,
        };

        assert_eq!(error.status, StatusCode::UNAUTHORIZED.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn admin_extractor_authorizes_admin_session() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        user_repo::insert_user(pool, user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("user should insert");
        session_repo::insert_session(
            pool,
            session_id,
            user_id,
            crate::application::auth::session::session_expires_at(),
        )
        .await
        .expect("session should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: pool.clone(),
        });
        let request = Request::builder()
            .uri("/admin-only")
            .header(
                axum::http::header::COOKIE,
                format!(
                    "{}={session_id}",
                    crate::application::auth::session::SESSION_COOKIE_NAME
                ),
            )
            .body(Body::empty())
            .expect("request should build");
        let (mut parts, _) = request.into_parts();

        let admin = AdminUser::from_request_parts(&mut parts, &state)
            .await
            .expect("admin should be authorized");

        assert_eq!(admin.into_inner().id, user_id);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn admin_extractor_rejects_player_session() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        user_repo::insert_user(pool, user_id, "player", "hash", UserRole::Player)
            .await
            .expect("user should insert");
        session_repo::insert_session(
            pool,
            session_id,
            user_id,
            crate::application::auth::session::session_expires_at(),
        )
        .await
        .expect("session should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: pool.clone(),
        });
        let request = Request::builder()
            .uri("/admin-only")
            .header(
                axum::http::header::COOKIE,
                format!(
                    "{}={session_id}",
                    crate::application::auth::session::SESSION_COOKIE_NAME
                ),
            )
            .body(Body::empty())
            .expect("request should build");
        let (mut parts, _) = request.into_parts();

        let error = match AdminUser::from_request_parts(&mut parts, &state).await {
            Ok(_) => panic!("player request should fail"),
            Err(error) => error,
        };

        assert_eq!(error.status, StatusCode::FORBIDDEN.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn admin_extractor_rejects_player_for_replay_void_and_correction_paths() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        user_repo::insert_user(pool, user_id, "player", "hash", UserRole::Player)
            .await
            .expect("user should insert");
        session_repo::insert_session(
            pool,
            session_id,
            user_id,
            crate::application::auth::session::session_expires_at(),
        )
        .await
        .expect("session should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: pool.clone(),
        });

        for uri in [
            "/api/admin/recalculate-ratings",
            "/api/matches/00000000-0000-0000-0000-000000000001/void",
            "/api/matches/00000000-0000-0000-0000-000000000001/correct",
        ] {
            let request = Request::builder()
                .uri(uri)
                .header(
                    axum::http::header::COOKIE,
                    format!(
                        "{}={session_id}",
                        crate::application::auth::session::SESSION_COOKIE_NAME
                    ),
                )
                .body(Body::empty())
                .expect("request should build");
            let (mut parts, _) = request.into_parts();

            let error = match AdminUser::from_request_parts(&mut parts, &state).await {
                Ok(_) => panic!("player request should fail for {uri}"),
                Err(error) => error,
            };

            assert_eq!(error.status, StatusCode::FORBIDDEN.as_u16());
        }

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
