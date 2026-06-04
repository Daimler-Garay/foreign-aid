use uuid::Uuid;

use crate::{
    application::repositories::RepositoryResult,
    db::DatabasePool,
    domain::models::auth::{User, UserRole},
};

pub async fn insert_user(
    pool: &DatabasePool,
    id: Uuid,
    username: &str,
    password_hash: &str,
    role: UserRole,
) -> RepositoryResult<User> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, username, password_hash, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, username, password_hash, role, active, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .bind(role.as_str())
    .fetch_one(pool)
    .await
}

pub async fn find_user_by_username(
    pool: &DatabasePool,
    username: &str,
) -> RepositoryResult<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, active, created_at, updated_at
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_by_id(pool: &DatabasePool, user_id: Uuid) -> RepositoryResult<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, active, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn set_user_active(
    pool: &DatabasePool,
    user_id: Uuid,
    active: bool,
) -> RepositoryResult<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET active = $2
        WHERE id = $1
        RETURNING id, username, password_hash, role, active, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(active)
    .fetch_optional(pool)
    .await
}

pub async fn ensure_admin_user(
    pool: &DatabasePool,
    id: Uuid,
    username: &str,
    password_hash: &str,
) -> RepositoryResult<User> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, username, password_hash, role, active)
        VALUES ($1, $2, $3, 'admin', TRUE)
        ON CONFLICT (username) DO UPDATE
        SET password_hash = EXCLUDED.password_hash,
            role = 'admin',
            active = TRUE
        RETURNING id, username, password_hash, role, active, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, DatabaseOptions, options::PostgresOptions};

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
    async fn can_seed_and_reactivate_admin_user() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = Uuid::new_v4();

        let admin = ensure_admin_user(pool, user_id, "admin", "hash-one")
            .await
            .expect("admin should insert");
        assert_eq!(admin.id, user_id);
        assert_eq!(admin.role, "admin");
        assert!(admin.active);

        set_user_active(pool, user_id, false)
            .await
            .expect("user should update");

        let admin = ensure_admin_user(pool, Uuid::new_v4(), "admin", "hash-two")
            .await
            .expect("admin should update");
        assert_eq!(admin.id, user_id);
        assert_eq!(admin.password_hash, "hash-two");
        assert_eq!(admin.role, "admin");
        assert!(admin.active);

        let found = find_user_by_id(pool, user_id)
            .await
            .expect("lookup should run")
            .expect("user should exist");
        assert_eq!(found.username, "admin");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
