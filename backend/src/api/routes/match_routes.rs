use axum::{Router, routing::get};

use crate::{
    api::handlers::matches_handlers::{
        get_match_handler, list_matches_handler, submit_match_handler,
    },
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_matches_handler).post(submit_match_handler))
        .route("/{id}", get(get_match_handler))
}
