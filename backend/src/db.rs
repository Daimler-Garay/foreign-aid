use sqlx::PgPool;
use thiserror::Error;

use crate::db::{options::PostgresOptions, postgres::PostgresDatabase};

pub mod options;
pub mod postgres;

pub type DatabasePool = PgPool;
#[cfg(test)]
pub type TestDatabase = PostgresDatabase;

#[derive(Clone, Debug)]
pub struct DatabaseOptions {
    pub postgres: PostgresOptions,
}

pub struct Database;

impl Database {
    pub async fn connect(options: DatabaseOptions) -> Result<DatabasePool, DatabaseError> {
        let db = PostgresDatabase::connect(options).await?;
        Ok(db.pool().clone())
    }

    #[cfg(test)]
    pub async fn open_test_database(
        options: DatabaseOptions,
    ) -> Result<TestDatabase, DatabaseError> {
        // create test db
        let db = PostgresDatabase::connect_test(options).await?;

        // Run db migrations
        Self::migrate(db.pool()).await?;

        Ok(db)
    }

    pub async fn migrate(pool: &DatabasePool) -> Result<(), DatabaseError> {
        sqlx::migrate!("./infrastructure/migrations/")
            .run(pool)
            .await?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error(transparent)]
    SQLxError(#[from] sqlx::Error),
    #[error(transparent)]
    SQLxMigrateError(#[from] sqlx::migrate::MigrateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::options::PostgresOptions;

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
    async fn can_connect_to_postgres() {
        let pool = Database::connect(test_options())
            .await
            .expect("should connect to postgres");

        let result: i32 = sqlx::query_scalar("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("should execute SELECT 1");

        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn can_create_and_drop_test_database() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");

        let result: i32 = sqlx::query_scalar("SELECT 1")
            .fetch_one(db.pool())
            .await
            .expect("should query temporary database");

        assert_eq!(result, 1);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn migrations_create_production_schema_without_uuid_defaults() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");

        let tables = [
            "users",
            "players",
            "player_ratings",
            "matches",
            "match_players",
            "audit_log",
            "rating_recalculation_runs",
        ];

        for table in tables {
            let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
                .bind(format!("public.{table}"))
                .fetch_one(db.pool())
                .await
                .expect("should check table existence");

            assert!(exists, "expected table {table} to exist");
        }

        let id_defaults: Vec<Option<String>> = sqlx::query_scalar(
            r#"
            SELECT column_default
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND column_name IN ('id', 'player_id')
              AND table_name IN (
                  'users',
                  'players',
                  'player_ratings',
                  'matches',
                  'audit_log',
                  'rating_recalculation_runs'
              )
            ORDER BY table_name, column_name
            "#,
        )
        .fetch_all(db.pool())
        .await
        .expect("should read id column defaults");

        assert!(
            id_defaults.iter().all(Option::is_none),
            "application-owned UUID columns should not have database defaults"
        );

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
