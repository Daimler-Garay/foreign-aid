use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::handlers::auth_handlers::{login_handler, logout_handler, me_handler},
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
}
