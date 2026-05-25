use sqlx::{PgConnection, PgPool};
use thiserror::Error;

use crate::db::postgres::{PostgresDatabase, PostgresOptions};

pub mod options;
pub mod postgres;

pub type DatabasePool = PgPool;
pub type DatabaseConnection = PgConnection;
pub type TestDatabase = PostgresDatabase;

#[derive(Clone, Debug)]
pub struct DatabaseOptions {
    pub postgres: PostgresOptions,
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error(transparent)]
    SQLxError(#[from] sqlx::Error),
    #[error(transparent)]
    SQLxMigrateError(#[from] sqlx::migrate::MigrateError),
}
