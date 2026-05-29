use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::{
    api::handlers::player_handlers::{
        add_player_handler, delete_player_handler, get_player_by_id_handler, list_player_handler,
    },
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_player_handler))
        .route("/{id}", get(get_player_by_id_handler))
        .route("/", post(add_player_handler))
        .route("/{id}", delete(delete_player_handler))
}
