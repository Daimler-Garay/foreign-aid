use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

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
        repositories::{
            audit_repo::{self, NewAuditLogEntry},
            session_repo, user_repo,
        },
        state::SharedState,
    },
    domain::models::auth::{
        AuthenticatedUser, CurrentUserResponse, LoginRequest, LoginResponse, User,
    },
};

const LOGIN_RATE_LIMIT_ATTEMPTS: usize = 5;
const LOGIN_RATE_LIMIT_WINDOW_SECONDS: i64 = 60;
static LOGIN_ATTEMPTS: OnceLock<Mutex<HashMap<String, Vec<chrono::DateTime<Utc>>>>> =
    OnceLock::new();

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

    check_login_rate_limit(&request.username)?;

    let Some(user) = user_repo::find_user_by_username(&state.db_pool, &request.username).await?
    else {
        record_login_audit(&state, None, &request.username, false).await?;
        return Err(unauthorized());
    };

    if !user.active {
        record_login_audit(&state, Some(&user), &request.username, false).await?;
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
        record_login_audit(&state, Some(&user), &request.username, false).await?;
        return Err(unauthorized());
    }

    let session_id = new_session_id();
    session_repo::insert_session(&state.db_pool, session_id, user.id, session_expires_at()).await?;
    record_login_audit(&state, Some(&user), &user.username, true).await?;
    clear_login_rate_limit(&user.username);

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

fn check_login_rate_limit(username: &str) -> Result<(), ApiError> {
    let key = username.trim().to_ascii_lowercase();
    let now = Utc::now();
    let cutoff = now - Duration::seconds(LOGIN_RATE_LIMIT_WINDOW_SECONDS);
    let attempts = LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts.lock().expect("login attempts mutex poisoned");
    let entries = attempts.entry(key).or_default();
    entries.retain(|attempted_at| *attempted_at >= cutoff);

    if entries.len() >= LOGIN_RATE_LIMIT_ATTEMPTS {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many login attempts. Try again shortly.",
        ));
    }

    entries.push(now);
    Ok(())
}

fn clear_login_rate_limit(username: &str) {
    let Some(attempts) = LOGIN_ATTEMPTS.get() else {
        return;
    };
    let mut attempts = attempts.lock().expect("login attempts mutex poisoned");
    attempts.remove(&username.trim().to_ascii_lowercase());
}

async fn record_login_audit(
    state: &SharedState,
    user: Option<&User>,
    username: &str,
    succeeded: bool,
) -> Result<(), ApiError> {
    let (action, result) = if succeeded {
        ("user.login", "success")
    } else {
        ("user.login_failed", "failure")
    };

    audit_repo::insert_audit_log_entry(
        &state.db_pool,
        NewAuditLogEntry {
            id: Uuid::new_v4(),
            actor_user_id: user.filter(|_| succeeded).map(|user| user.id),
            action: action.to_owned(),
            entity_type: "user".to_owned(),
            entity_id: user.map(|user| user.id),
            old_value: None,
            new_value: Some(json!({
                "username": username,
                "result": result,
            })),
        },
    )
    .await?;

    Ok(())
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
    use chrono::Utc;
    use serde_json::Value;
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

    #[derive(Debug, sqlx::FromRow)]
    struct AuditRow {
        action: String,
        actor_user_id: Option<Uuid>,
        entity_id: Option<Uuid>,
        new_value: Option<Value>,
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

    async fn latest_audit_row(pool: &crate::db::DatabasePool) -> AuditRow {
        sqlx::query_as::<_, AuditRow>(
            r#"
            SELECT action, actor_user_id, entity_id, new_value
            FROM audit_log
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .fetch_one(pool)
        .await
        .expect("audit row should exist")
    }

    #[tokio::test]
    async fn valid_login_sets_session_cookie_and_audits_success() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        let password_hash = hash_password("secret").expect("hash should succeed");
        user_repo::insert_user(db.pool(), user_id, "admin", &password_hash, UserRole::Admin)
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

        let audit = latest_audit_row(db.pool()).await;
        assert_eq!(audit.action, "user.login");
        assert_eq!(audit.actor_user_id, Some(user_id));
        assert_eq!(audit.entity_id, Some(user_id));
        assert_eq!(audit.new_value.as_ref().unwrap()["username"], "admin");
        assert_eq!(audit.new_value.as_ref().unwrap()["result"], "success");
        let audit_json =
            serde_json::to_string(&audit.new_value).expect("audit value should serialize");
        assert!(!audit_json.contains("secret"));
        assert!(!audit_json.contains(&password_hash));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn invalid_login_is_rejected_generically_and_audited_without_password() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        let password_hash = hash_password("secret").expect("hash should succeed");
        user_repo::insert_user(db.pool(), user_id, "admin", &password_hash, UserRole::Admin)
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

        let audit = latest_audit_row(db.pool()).await;
        assert_eq!(audit.action, "user.login_failed");
        assert_eq!(audit.actor_user_id, None);
        assert_eq!(audit.entity_id, Some(user_id));
        assert_eq!(audit.new_value.as_ref().unwrap()["username"], "admin");
        assert_eq!(audit.new_value.as_ref().unwrap()["result"], "failure");
        let audit_json =
            serde_json::to_string(&audit.new_value).expect("audit value should serialize");
        assert!(!audit_json.contains("wrong"));
        assert!(!audit_json.contains(&password_hash));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn repeated_login_attempts_are_rate_limited() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let username = format!("rate-limited-{}", Uuid::new_v4());

        for _ in 0..LOGIN_RATE_LIMIT_ATTEMPTS {
            let error = match login_handler(
                State(state.clone()),
                Json(LoginRequest {
                    username: username.clone(),
                    password: "wrong".to_owned(),
                }),
            )
            .await
            {
                Ok(_) => panic!("login should fail"),
                Err(error) => error,
            };
            assert_eq!(error.status, StatusCode::UNAUTHORIZED.as_u16());
        }

        let error = match login_handler(
            State(state),
            Json(LoginRequest {
                username,
                password: "wrong".to_owned(),
            }),
        )
        .await
        {
            Ok(_) => panic!("login should be rate limited"),
            Err(error) => error,
        };

        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS.as_u16());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn inactive_user_login_is_rejected_and_audited() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let user_id = Uuid::new_v4();
        let password_hash = hash_password("secret").expect("hash should succeed");
        user_repo::insert_user(db.pool(), user_id, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("user should insert");
        user_repo::set_user_active(db.pool(), user_id, false)
            .await
            .expect("user should deactivate");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });

        let error = match login_handler(
            State(state),
            Json(LoginRequest {
                username: "admin".to_owned(),
                password: "secret".to_owned(),
            }),
        )
        .await
        {
            Ok(_) => panic!("inactive login should fail"),
            Err(error) => error,
        };

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let audit = latest_audit_row(db.pool()).await;
        assert_eq!(audit.action, "user.login_failed");
        assert_eq!(audit.actor_user_id, None);
        assert_eq!(audit.entity_id, Some(user_id));
        assert_eq!(audit.new_value.as_ref().unwrap()["result"], "failure");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn me_handler_returns_current_user() {
        let user_id = Uuid::new_v4();
        let player_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();

        let Json(response) = me_handler(AuthenticatedUser {
            id: user_id,
            username: "admin".to_owned(),
            role: "admin".to_owned(),
            active: true,
            player_id: Some(player_id),
            session_id,
        })
        .await;

        assert_eq!(response.id, user_id);
        assert_eq!(response.username, "admin");
        assert_eq!(response.role, "admin");
        assert!(response.active);
        assert_eq!(response.player_id, Some(player_id));
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
