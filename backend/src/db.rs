use sqlx::{PgConnection, PgPool};
use thiserror::Error;

use crate::db::{options::PostgresOptions, postgres::PostgresDatabase};

pub mod options;
pub mod postgres;

pub type DatabasePool = PgPool;
pub type DatabaseConnection = PgConnection;
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
}
