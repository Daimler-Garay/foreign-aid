use axum::{
    Json, Router,
    extract::{Request, State},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use thiserror::Error;
use tokio::{
    net::TcpListener,
    signal::{
        self,
        unix::{self, SignalKind},
    },
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{api::error::ApiError, application::state::SharedState};

pub async fn start(state: SharedState) -> Result<(), ServerError> {
    let addr = state.config.service_socket_addr()?;
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|source| ServerError::Bind { addr, source })?;

    tracing::info!("listening on {}", addr);

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(ServerError::Serve)?;

    tracing::info!("server shutdown successfully");
    Ok(())
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
        .fallback(error_404_handler)
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        unix::signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("received termination signal, shutting down");
}

pub async fn healthz_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz_handler(
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map_err(|error| {
            tracing::error!(%error, "database readiness check failed");
            ApiError::service_unavailable("database_not_ready", "Database is not ready.")
        })?;

    Ok(Json(json!({ "status": "ready" })))
}

pub async fn error_404_handler(request: Request) -> impl IntoResponse {
    tracing::debug!(method = %request.method(), uri = %request.uri(), "route not found");
    ApiError::not_found("route_not_found", "Route not found.").into_response()
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    Config(#[from] crate::application::config::ConfigError),
    #[error("failed to bind HTTP listener at {addr}")]
    Bind {
        addr: std::net::SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("HTTP server failed")]
    Serve(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::to_bytes, extract::State, http::StatusCode, response::IntoResponse};

    use super::*;
    use crate::{
        application::{config::Config, state::AppState},
        db::{Database, options::PostgresOptions},
    };

    fn test_config() -> Config {
        Config {
            app_env: "test".to_owned(),
            app_host: "127.0.0.1".to_owned(),
            app_port: 0,
            database: PostgresOptions {
                database_url: None,
                db: "foreign_aid".to_owned(),
                host: "localhost".to_owned(),
                port: 5433,
                user: "admin".to_owned(),
                password: "admin".to_owned(),
                max_connections: 5,
            },
        }
    }

    #[tokio::test]
    async fn healthz_returns_success() {
        let response = healthz_handler().await.into_response();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_returns_success_when_database_is_available() {
        let config = test_config();
        let db_pool = Database::connect(config.clone().into())
            .await
            .expect("test database should be available");
        let state = Arc::new(AppState { config, db_pool });

        let response = readyz_handler(State(state))
            .await
            .expect("readyz should succeed")
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn not_found_uses_error_envelope() {
        let request = Request::builder()
            .uri("/missing")
            .body(axum::body::Body::empty())
            .expect("request should build");

        let response = error_404_handler(request).await.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(value["error"]["code"], "route_not_found");
    }
}
