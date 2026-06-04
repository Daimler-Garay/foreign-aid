use std::cmp::Ordering;

use axum::http::StatusCode;

use crate::{
    api::error::ApiError,
    application::{
        repositories::player_repo,
        services::rating_service::{conservative_rank_score, display_rating},
        state::SharedState,
    },
    domain::models::{
        leaderboard::{LeaderboardQuery, LeaderboardRow},
        players::PlayerWithRating,
    },
};

pub const DEFAULT_MIN_GAMES: i32 = 3;

pub async fn get_leaderboard(
    state: &SharedState,
    query: LeaderboardQuery,
) -> Result<Vec<LeaderboardRow>, ApiError> {
    let min_games = query.min_games.unwrap_or(DEFAULT_MIN_GAMES);
    if min_games < 0 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "min_games must be greater than or equal to 0.",
        ));
    }

    let include_inactive = query.include_inactive.unwrap_or(false);
    let players = player_repo::list_players_with_ratings(&state.db_pool, include_inactive).await?;

    Ok(build_leaderboard(players, min_games))
}

fn build_leaderboard(players: Vec<PlayerWithRating>, min_games: i32) -> Vec<LeaderboardRow> {
    let mut rows = players
        .into_iter()
        .map(|player| {
            let display_rating = display_rating(player.rating);
            let rank_score = conservative_rank_score(player.rating, player.uncertainty);
            let win_rate = (player.games_played > 0)
                .then_some(player.wins as f64 / player.games_played as f64);
            let average_placement = (player.games_played > 0)
                .then_some(player.total_placement as f64 / player.games_played as f64);

            LeaderboardRow {
                rank: None,
                player_id: player.id,
                display_name: player.display_name,
                display_rating,
                rank_score,
                games_played: player.games_played,
                wins: player.wins,
                losses: player.losses,
                win_rate,
                average_placement,
                last_played_at: player.last_played_at,
                active: player.active,
            }
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| compare_rows(left, right, min_games));

    let mut next_rank = 1;
    for row in &mut rows {
        if row.games_played >= min_games {
            row.rank = Some(next_rank);
            next_rank += 1;
        }
    }

    rows
}

fn compare_rows(left: &LeaderboardRow, right: &LeaderboardRow, min_games: i32) -> Ordering {
    let left_ranked = left.games_played >= min_games;
    let right_ranked = right.games_played >= min_games;

    match (left_ranked, right_ranked) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (true, true) => compare_ranked_rows(left, right),
        (false, false) => compare_unranked_rows(left, right),
    }
}

fn compare_ranked_rows(left: &LeaderboardRow, right: &LeaderboardRow) -> Ordering {
    right
        .rank_score
        .cmp(&left.rank_score)
        .then_with(|| right.display_rating.cmp(&left.display_rating))
        .then_with(|| right.wins.cmp(&left.wins))
        .then_with(|| compare_optional_f64_desc(right.win_rate, left.win_rate))
        .then_with(|| right.games_played.cmp(&left.games_played))
        .then_with(|| left.display_name.cmp(&right.display_name))
        .then_with(|| left.player_id.cmp(&right.player_id))
}

fn compare_unranked_rows(left: &LeaderboardRow, right: &LeaderboardRow) -> Ordering {
    right
        .display_rating
        .cmp(&left.display_rating)
        .then_with(|| right.games_played.cmp(&left.games_played))
        .then_with(|| left.display_name.cmp(&right.display_name))
        .then_with(|| left.player_id.cmp(&right.player_id))
}

fn compare_optional_f64_desc(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
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

    async fn player(pool: &crate::db::DatabasePool, display_name: &str, active: bool) -> Uuid {
        let player_id = Uuid::new_v4();
        player_repo::insert_player(pool, player_id, display_name, None)
            .await
            .expect("player should insert");
        player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");
        player_repo::set_player_active(pool, player_id, active)
            .await
            .expect("active state should update");

        player_id
    }

    struct TestRatingStats {
        rating: f64,
        uncertainty: f64,
        games_played: i32,
        wins: i32,
        losses: i32,
        total_placement: i32,
    }

    async fn set_rating_stats(
        pool: &crate::db::DatabasePool,
        player_id: Uuid,
        stats: TestRatingStats,
    ) {
        sqlx::query(
            r#"
            UPDATE player_ratings
            SET rating = $2,
                uncertainty = $3,
                games_played = $4,
                wins = $5,
                losses = $6,
                total_placement = $7,
                last_played_at = $8
            WHERE player_id = $1
            "#,
        )
        .bind(player_id)
        .bind(stats.rating)
        .bind(stats.uncertainty)
        .bind(stats.games_played)
        .bind(stats.wins)
        .bind(stats.losses)
        .bind(stats.total_placement)
        .bind(Utc::now())
        .execute(pool)
        .await
        .expect("rating stats should update");
    }

    #[tokio::test]
    async fn leaderboard_sorts_ranked_before_unranked() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let ranked = player(db.pool(), "Ranked", true).await;
        let unranked = player(db.pool(), "Unranked", true).await;
        set_rating_stats(
            db.pool(),
            ranked,
            TestRatingStats {
                rating: 30.0,
                uncertainty: 7.0,
                games_played: 3,
                wins: 2,
                losses: 1,
                total_placement: 5,
            },
        )
        .await;
        set_rating_stats(
            db.pool(),
            unranked,
            TestRatingStats {
                rating: 40.0,
                uncertainty: 7.0,
                games_played: 2,
                wins: 2,
                losses: 0,
                total_placement: 2,
            },
        )
        .await;

        let rows = get_leaderboard(
            &state,
            LeaderboardQuery {
                min_games: None,
                include_inactive: None,
            },
        )
        .await
        .expect("leaderboard should load");

        assert_eq!(rows[0].player_id, ranked);
        assert_eq!(rows[0].rank, Some(1));
        assert_eq!(rows[1].player_id, unranked);
        assert_eq!(rows[1].rank, None);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn leaderboard_min_games_query_controls_ranked_threshold() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let player_id = player(db.pool(), "One Game", true).await;
        set_rating_stats(
            db.pool(),
            player_id,
            TestRatingStats {
                rating: 25.0,
                uncertainty: 8.0,
                games_played: 1,
                wins: 1,
                losses: 0,
                total_placement: 1,
            },
        )
        .await;

        let rows = get_leaderboard(
            &state,
            LeaderboardQuery {
                min_games: Some(1),
                include_inactive: None,
            },
        )
        .await
        .expect("leaderboard should load");

        assert_eq!(rows[0].rank, Some(1));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn leaderboard_excludes_inactive_by_default_and_can_include_them() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let active = player(db.pool(), "Active", true).await;
        let inactive = player(db.pool(), "Inactive", false).await;

        let rows = get_leaderboard(
            &state,
            LeaderboardQuery {
                min_games: None,
                include_inactive: None,
            },
        )
        .await
        .expect("leaderboard should load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].player_id, active);

        let rows = get_leaderboard(
            &state,
            LeaderboardQuery {
                min_games: None,
                include_inactive: Some(true),
            },
        )
        .await
        .expect("leaderboard should load");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.player_id == inactive));

        db.drop()
            .await
            .expect("should drop temporary test database");
    }

    #[tokio::test]
    async fn leaderboard_calculates_win_rate_and_average_placement() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let state = Arc::new(AppState {
            config: test_config(),
            db_pool: db.pool().clone(),
        });
        let player_id = player(db.pool(), "Stats", true).await;
        let no_games = player(db.pool(), "No Games", true).await;
        set_rating_stats(
            db.pool(),
            player_id,
            TestRatingStats {
                rating: 25.0,
                uncertainty: 8.0,
                games_played: 4,
                wins: 1,
                losses: 3,
                total_placement: 10,
            },
        )
        .await;

        let rows = get_leaderboard(
            &state,
            LeaderboardQuery {
                min_games: Some(0),
                include_inactive: None,
            },
        )
        .await
        .expect("leaderboard should load");
        let stats = rows.iter().find(|row| row.player_id == player_id).unwrap();
        let empty = rows.iter().find(|row| row.player_id == no_games).unwrap();

        assert_eq!(stats.win_rate, Some(0.25));
        assert_eq!(stats.average_placement, Some(2.5));
        assert_eq!(empty.win_rate, None);
        assert_eq!(empty.average_placement, None);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
