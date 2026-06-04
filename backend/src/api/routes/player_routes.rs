use axum::{Router, routing::post};

use crate::{
    api::handlers::player_handlers::create_player_handler, application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new().route("/", post(create_player_handler))
}
