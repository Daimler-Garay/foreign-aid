use axum::{Router, routing::get};

use crate::{
    api::handlers::leaderboard_handlers::get_leaderboard_handler, application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new().route("/", get(get_leaderboard_handler))
}
