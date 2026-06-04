use axum::http::StatusCode;

use crate::{
    api::error::ApiError,
    application::{repositories::audit_repo, state::SharedState},
    domain::models::audit::{AuditLogEntry, AuditLogQuery, AuditLogResponse},
};

pub const DEFAULT_AUDIT_LOG_LIMIT: i64 = 100;
pub const MAX_AUDIT_LOG_LIMIT: i64 = 500;

pub async fn list_audit_log(
    state: &SharedState,
    query: AuditLogQuery,
) -> Result<Vec<AuditLogResponse>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_AUDIT_LOG_LIMIT);
    if !(1..=MAX_AUDIT_LOG_LIMIT).contains(&limit) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            format!("limit must be between 1 and {MAX_AUDIT_LOG_LIMIT}."),
        ));
    }

    let entries = audit_repo::list_audit_log_entries(&state.db_pool, limit).await?;

    Ok(entries.into_iter().map(audit_log_response).collect())
}

fn audit_log_response(entry: AuditLogEntry) -> AuditLogResponse {
    AuditLogResponse {
        id: entry.id,
        actor_user_id: entry.actor_user_id,
        action: entry.action,
        entity_type: entry.entity_type,
        entity_id: entry.entity_id,
        old_value: entry.old_value.map(sanitize_audit_value),
        new_value: entry.new_value.map(sanitize_audit_value),
        created_at: entry.created_at,
    }
}

fn sanitize_audit_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter_map(|(key, value)| {
                    (!is_sensitive_key(&key)).then(|| (key, sanitize_audit_value(value)))
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sanitize_audit_value).collect())
        }
        other => other,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password"
            | "password_hash"
            | "token"
            | "session_token"
            | "session_id"
            | "cookie"
            | "set_cookie"
            | "authorization"
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{
            config::Config, repositories::audit_repo::NewAuditLogEntry, state::AppState,
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

    #[tokio::test]
    async fn audit_log_lists_newest_first_and_applies_limit() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        for action in ["first", "second", "third"] {
            audit_repo::insert_audit_log_entry(
                db.pool(),
                NewAuditLogEntry {
                    id: Uuid::new_v4(),
                    actor_user_id: None,
                    action: action.to_owned(),
                    entity_type: "test".to_owned(),
                    entity_id: None,
                    old_value: None,
                    new_value: None,
                },
            )
            .await
            .expect("audit entry should insert");
        }

        let entries = list_audit_log(&state, AuditLogQuery { limit: Some(2) })
            .await
            .expect("audit log should list");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, "third");
        assert_eq!(entries[1].action, "second");

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn audit_log_response_omits_secrets() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        audit_repo::insert_audit_log_entry(
            db.pool(),
            NewAuditLogEntry {
                id: Uuid::new_v4(),
                actor_user_id: None,
                action: "test.secret".to_owned(),
                entity_type: "test".to_owned(),
                entity_id: None,
                old_value: None,
                new_value: Some(serde_json::json!({
                    "username": "admin",
                    "password": "plaintext",
                    "nested": {
                        "password_hash": "hash",
                        "result": "ok"
                    }
                })),
            },
        )
        .await
        .expect("audit entry should insert");

        let entries = list_audit_log(&state, AuditLogQuery { limit: None })
            .await
            .expect("audit log should list");
        let serialized = serde_json::to_string(&entries).expect("entries should serialize");

        assert!(serialized.contains("admin"));
        assert!(serialized.contains("result"));
        assert!(!serialized.contains("plaintext"));
        assert!(!serialized.contains("hash"));
        assert!(!serialized.contains("password"));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
