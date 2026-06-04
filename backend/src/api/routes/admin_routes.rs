use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::handlers::admin_handlers::{list_audit_log_handler, recalculate_ratings_handler},
    application::state::SharedState,
};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/recalculate-ratings", post(recalculate_ratings_handler))
        .route("/audit-log", get(list_audit_log_handler))
}
