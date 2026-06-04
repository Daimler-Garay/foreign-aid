use axum::{Router, routing::post};

use crate::{
    api::handlers::admin_handlers::recalculate_ratings_handler, application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new().route("/recalculate-ratings", post(recalculate_ratings_handler))
}
