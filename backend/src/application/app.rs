use std::sync::Arc;

use thiserror::Error;

use crate::{
    api::server::{self, ServerError},
    application::{
        auth::{password, password::PasswordError},
        config::{self, ConfigError},
        repositories::user_repo,
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

pub async fn seed_admin(username: &str, password: &str) -> Result<String, StartupError> {
    let username = username.trim();
    if username.is_empty() {
        return Err(StartupError::SeedAdmin(
            "admin username must not be blank".to_owned(),
        ));
    }
    if password.is_empty() {
        return Err(StartupError::SeedAdmin(
            "admin password must not be blank".to_owned(),
        ));
    }

    let config = config::load()?;
    let db_pool = Database::connect(config.into()).await?;
    Database::migrate(&db_pool).await?;

    let password_hash = password::hash_password(password)?;
    let user =
        user_repo::ensure_admin_user(&db_pool, uuid::Uuid::new_v4(), username, &password_hash)
            .await?;

    Ok(user.username)
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error(transparent)]
    Repository(#[from] sqlx::Error),
    #[error("{0}")]
    SeedAdmin(String),
}
