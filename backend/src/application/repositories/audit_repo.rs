use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::{application::repositories::RepositoryResult, domain::models::audit::AuditLogEntry};

#[derive(Debug, Clone)]
pub struct NewAuditLogEntry {
    pub id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

pub async fn insert_audit_log_entry<'e, E>(
    executor: E,
    entry: NewAuditLogEntry,
) -> RepositoryResult<AuditLogEntry>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, AuditLogEntry>(
        r#"
        INSERT INTO audit_log (
            id,
            actor_user_id,
            action,
            entity_type,
            entity_id,
            old_value,
            new_value
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, actor_user_id, action, entity_type, entity_id,
                  old_value, new_value, created_at
        "#,
    )
    .bind(entry.id)
    .bind(entry.actor_user_id)
    .bind(entry.action)
    .bind(entry.entity_type)
    .bind(entry.entity_id)
    .bind(entry.old_value)
    .bind(entry.new_value)
    .fetch_one(executor)
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
    async fn can_insert_audit_log_entry() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let id = Uuid::new_v4();

        let entry = insert_audit_log_entry(
            db.pool(),
            NewAuditLogEntry {
                id,
                actor_user_id: None,
                action: "user.login_failed".to_owned(),
                entity_type: "user".to_owned(),
                entity_id: None,
                old_value: None,
                new_value: Some(serde_json::json!({
                    "username": "admin",
                    "result": "failure"
                })),
            },
        )
        .await
        .expect("audit entry should insert");

        assert_eq!(entry.id, id);
        assert_eq!(entry.action, "user.login_failed");
        assert_eq!(entry.entity_type, "user");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
