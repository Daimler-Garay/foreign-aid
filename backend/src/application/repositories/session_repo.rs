use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    application::repositories::RepositoryResult,
    db::DatabasePool,
    domain::models::auth::{AuthenticatedUser, UserSession},
};

pub async fn insert_session(
    pool: &DatabasePool,
    session_id: Uuid,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> RepositoryResult<UserSession> {
    sqlx::query_as::<_, UserSession>(
        r#"
        INSERT INTO user_sessions (id, user_id, expires_at)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, expires_at, revoked_at, created_at, last_seen_at
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

pub async fn find_authenticated_user_by_session_id(
    pool: &DatabasePool,
    session_id: Uuid,
) -> RepositoryResult<Option<AuthenticatedUser>> {
    sqlx::query_as::<_, AuthenticatedUser>(
        r#"
        UPDATE user_sessions s
        SET last_seen_at = now()
        FROM users u
        LEFT JOIN players p ON p.user_id = u.id
        WHERE s.id = $1
          AND s.user_id = u.id
          AND s.revoked_at IS NULL
          AND s.expires_at > now()
          AND u.active = TRUE
        RETURNING
            u.id,
            u.username,
            u.role,
            u.active,
            p.id AS player_id,
            s.id AS session_id
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
}

pub async fn revoke_session(pool: &DatabasePool, session_id: Uuid) -> RepositoryResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE user_sessions
        SET revoked_at = now()
        WHERE id = $1
          AND revoked_at IS NULL
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;
    use crate::{
        application::repositories::user_repo,
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

    #[tokio::test]
    async fn can_create_load_and_revoke_session() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();

        user_repo::insert_user(pool, user_id, "admin", "password-hash", UserRole::Admin)
            .await
            .expect("user should insert");

        let session = insert_session(pool, session_id, user_id, Utc::now() + Duration::hours(1))
            .await
            .expect("session should insert");
        assert_eq!(session.id, session_id);

        let authenticated = find_authenticated_user_by_session_id(pool, session_id)
            .await
            .expect("session lookup should run")
            .expect("session should authenticate");
        assert_eq!(authenticated.id, user_id);
        assert_eq!(authenticated.session_id, session_id);

        let revoked = revoke_session(pool, session_id)
            .await
            .expect("session should revoke");
        assert!(revoked);

        let authenticated = find_authenticated_user_by_session_id(pool, session_id)
            .await
            .expect("session lookup should run");
        assert!(authenticated.is_none());

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
