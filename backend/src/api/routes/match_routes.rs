use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::handlers::matches_handlers::{
        create_match_handler, get_match_detail_by_id_handler, get_match_history_handler,
    },
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/",
            post(create_match_handler).get(get_match_history_handler),
        )
        .route("/{id}", get(get_match_detail_by_id_handler))
}
