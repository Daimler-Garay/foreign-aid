use std::sync::Arc;

use thiserror::Error;

use crate::{
    api::server::{self, ServerError},
    application::{
        config::{self, ConfigError},
        state::AppState,
    },
    db::{Database, DatabaseError},
};

pub async fn run() -> Result<(), StartupError> {
    let config = config::load()?;

    let db_pool = Database::connect(config.clone().into()).await?;
    Database::migrate(&db_pool).await?;

    let shared_state = Arc::new(AppState { config, db_pool });

    server::start(shared_state).await?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Server(#[from] ServerError),
}
