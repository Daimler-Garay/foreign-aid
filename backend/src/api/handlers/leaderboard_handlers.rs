use axum::{Json, extract::Query, response::IntoResponse};

use crate::{
    api::error::ApiError,
    application::{services::leaderboard_service, state::SharedState},
    domain::models::leaderboard::LeaderboardQuery,
};

pub async fn get_leaderboard_handler(
    axum::extract::State(state): axum::extract::State<SharedState>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let response = leaderboard_service::get_leaderboard(&state, query).await?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::to_bytes, extract::State, response::IntoResponse};
    use uuid::Uuid;

    use super::*;
    use crate::{
        application::{config::Config, repositories::player_repo, state::AppState},
        db::{Database, DatabaseOptions, options::PostgresOptions},
    };

    fn test_options() -> DatabaseOptions {
        DatabaseOptions {
            postgres: PostgresOptions {
                database_url: None,
                db: "foreign_aid".to_string(),
                host: "localhost".to_string(),
                port: 5433,
                user: "admin".to_string(),
                password: "admin".to_string(),
                max_connections: 5,
            },
        }
    }

    fn test_config() -> Config {
        Config {
            app_env: "test".to_owned(),
            app_host: "127.0.0.1".to_owned(),
            app_port: 0,
            database: test_options().postgres,
        }
    }

    async fn player(pool: &crate::db::DatabasePool, display_name: &str) -> Uuid {
        let player_id = Uuid::new_v4();
        player_repo::insert_player(pool, player_id, display_name, None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");

        player_id
    }

    #[tokio::test]
    async fn get_leaderboard_returns_rows() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        player(db.pool(), "Alice").await;

        let response = get_leaderboard_handler(
            State(state),
            Query(LeaderboardQuery {
                min_games: None,
                include_inactive: None,
            }),
        )
        .await
        .expect("leaderboard should load")
        .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value.as_array().unwrap().len(), 1);
        assert_eq!(value[0]["display_name"], "Alice");
        assert_eq!(value[0]["rank"], serde_json::Value::Null);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
