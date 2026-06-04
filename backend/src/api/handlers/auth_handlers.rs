use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    api::error::ApiError,
    application::{
        auth::{
            password::verify_password,
            session::{
                append_expired_session_cookie, append_session_cookie, new_session_id,
                session_expires_at,
            },
        },
        repositories::{session_repo, user_repo},
        state::SharedState,
    },
    domain::models::auth::{AuthenticatedUser, CurrentUserResponse, LoginRequest, LoginResponse},
};

pub async fn login_handler(
    State(state): State<SharedState>,
    Json(request): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let unauthorized = || {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Invalid username or password.",
        )
    };

    let Some(user) = user_repo::find_user_by_username(&state.db_pool, &request.username).await?
    else {
        return Err(unauthorized());
    };

    if !user.active {
        return Err(unauthorized());
    }

    let password_matches =
        verify_password(&request.password, &user.password_hash).map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_verification_failed",
                "Password verification failed.",
            )
        })?;

    if !password_matches {
        return Err(unauthorized());
    }

    let session_id = new_session_id();
    session_repo::insert_session(&state.db_pool, session_id, user.id, session_expires_at()).await?;

    let mut headers = HeaderMap::new();
    append_session_cookie(
        &mut headers,
        session_id,
        state.config.app_env == "production",
    );

    Ok((
        headers,
        Json(LoginResponse {
            user: CurrentUserResponse {
                id: user.id,
                username: user.username,
                role: user.role,
                active: user.active,
                player_id: None,
            },
        }),
    ))
}

pub async fn logout_handler(
    State(state): State<SharedState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    session_repo::revoke_session(&state.db_pool, user.session_id).await?;

    let mut headers = HeaderMap::new();
    append_expired_session_cookie(&mut headers, state.config.app_env == "production");

    Ok((headers, StatusCode::NO_CONTENT))
}

pub async fn me_handler(user: AuthenticatedUser) -> Json<CurrentUserResponse> {
    Json(CurrentUserResponse {
        id: user.id,
        username: user.username,
        role: user.role,
        active: user.active,
        player_id: user.player_id,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        Json,
        extract::State,
        http::{StatusCode, header::SET_COOKIE},
        response::IntoResponse,
    };
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{
            auth::password::hash_password,
            config::Config,
            repositories::{session_repo, user_repo},
            state::AppState,
        },
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

    #[tokio::test]
    async fn valid_login_sets_session_cookie() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let password_hash = hash_password("secret").expect("hash should succeed");
        user_repo::insert_user(
            db.pool(),
            Uuid::new_v4(),
            "admin",
            &password_hash,
            UserRole::Admin,
        )
        .await
        .expect("user should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let response = login_handler(
            State(state),
            Json(LoginRequest {
                username: "admin".to_owned(),
                password: "secret".to_owned(),
            }),
        )
        .await
        .expect("login should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(SET_COOKIE).is_some());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn invalid_login_is_rejected_generically() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let password_hash = hash_password("secret").expect("hash should succeed");
        user_repo::insert_user(
            db.pool(),
            Uuid::new_v4(),
            "admin",
            &password_hash,
            UserRole::Admin,
        )
        .await
        .expect("user should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let error = match login_handler(
            State(state),
            Json(LoginRequest {
                username: "admin".to_owned(),
                password: "wrong".to_owned(),
            }),
        )
        .await
        {
            Ok(_) => panic!("login should fail"),
            Err(error) => error,
        };

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn logout_revokes_session_and_clears_cookie() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        user_repo::insert_user(db.pool(), user_id, "admin", "hash", UserRole::Admin)
            .await
            .expect("user should insert");
        session_repo::insert_session(
            db.pool(),
            session_id,
            user_id,
            Utc::now() + Duration::hours(1),
        )
        .await
        .expect("session should insert");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let response = logout_handler(
            State(state),
            AuthenticatedUser {
                id: user_id,
                username: "admin".to_owned(),
                role: "admin".to_owned(),
                active: true,
                player_id: None,
                session_id,
            },
        )
        .await
        .expect("logout should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response
                .headers()
                .get(SET_COOKIE)
                .expect("set-cookie should exist")
                .to_str()
                .expect("cookie should be visible")
                .contains("Max-Age=0")
        );

        let authenticated =
            session_repo::find_authenticated_user_by_session_id(db.pool(), session_id)
                .await
                .expect("lookup should run");
        assert!(authenticated.is_none());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
