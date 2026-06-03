use axum::{Router, routing::post};

use crate::{
    api::handlers::matches_handlers::create_match_handler, application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new().route("/", post(create_match_handler))
}
