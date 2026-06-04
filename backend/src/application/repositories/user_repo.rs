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
