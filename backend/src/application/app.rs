use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    api::server,
    application::{config, state::AppState},
    db::Database,
};

pub async fn run() {
    let config = config::load();

    let db_pool = Database::connect(config.clone().into())
        .await
        .expect("Failed to connect to the database");

    // execute migrations
    Database::migrate(&db_pool)
        .await
        .expect("Failed to run database migrations");

    // build app state
    let shared_state = Arc::new(AppState { config, db_pool });

    server::start(shared_state).await;
}
