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
        sqlx::migrate!("../backend/infrastructure/migrations/")
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
