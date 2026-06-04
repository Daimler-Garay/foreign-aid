use axum::{Router, routing::get};

use crate::{
    api::handlers::player_handlers::{
        create_player_handler, get_player_handler, list_players_handler,
    },
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_players_handler).post(create_player_handler))
        .route("/{id}", get(get_player_handler))
}
