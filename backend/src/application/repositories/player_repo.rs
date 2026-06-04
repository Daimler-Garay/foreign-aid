use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::{
    application::repositories::RepositoryResult,
    db::DatabasePool,
    domain::models::players::{Player, PlayerRating, PlayerWithRating},
};

pub async fn insert_player<'e, E>(
    executor: E,
    id: Uuid,
    display_name: &str,
    user_id: Option<Uuid>,
) -> RepositoryResult<Player>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, Player>(
        r#"
        INSERT INTO players (id, user_id, display_name)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, display_name, active, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(display_name)
    .fetch_one(executor)
    .await
}

pub async fn insert_default_rating<'e, E>(
    executor: E,
    player_id: Uuid,
) -> RepositoryResult<PlayerRating>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, PlayerRating>(
        r#"
        INSERT INTO player_ratings (player_id)
        VALUES ($1)
        RETURNING player_id, rating, uncertainty, games_played, wins, losses,
                  total_placement, last_played_at, updated_at
        "#,
    )
    .bind(player_id)
    .fetch_one(executor)
    .await
}

pub async fn find_player_with_rating(
    pool: &DatabasePool,
    player_id: Uuid,
) -> RepositoryResult<Option<PlayerWithRating>> {
    sqlx::query_as::<_, PlayerWithRating>(
        r#"
        SELECT
            p.id,
            p.user_id,
            p.display_name,
            p.active,
            p.created_at,
            p.updated_at,
            pr.rating,
            pr.uncertainty,
            pr.games_played,
            pr.wins,
            pr.losses,
            pr.total_placement,
            pr.last_played_at
        FROM players p
        JOIN player_ratings pr ON pr.player_id = p.id
        WHERE p.id = $1
        "#,
    )
    .bind(player_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_players_with_ratings(
    pool: &DatabasePool,
    include_inactive: bool,
) -> RepositoryResult<Vec<PlayerWithRating>> {
    sqlx::query_as::<_, PlayerWithRating>(
        r#"
        SELECT
            p.id,
            p.user_id,
            p.display_name,
            p.active,
            p.created_at,
            p.updated_at,
            pr.rating,
            pr.uncertainty,
            pr.games_played,
            pr.wins,
            pr.losses,
            pr.total_placement,
            pr.last_played_at
        FROM players p
        JOIN player_ratings pr ON pr.player_id = p.id
        WHERE $1 OR p.active = TRUE
        ORDER BY p.display_name ASC, p.id ASC
        "#,
    )
    .bind(include_inactive)
    .fetch_all(pool)
    .await
}

pub async fn set_player_active(
    pool: &DatabasePool,
    player_id: Uuid,
    active: bool,
) -> RepositoryResult<Option<Player>> {
    sqlx::query_as::<_, Player>(
        r#"
        UPDATE players
        SET active = $2
        WHERE id = $1
        RETURNING id, user_id, display_name, active, created_at, updated_at
        "#,
    )
    .bind(player_id)
    .bind(active)
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod tests {
    use crate::{
        application::repositories::{player_repo, user_repo},
        db::{Database, DatabaseOptions, options::PostgresOptions},
        domain::models::auth::UserRole,
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

    #[tokio::test]
    async fn can_insert_read_and_update_user_and_player() {
        let db = Database::open_test_database(test_options())
            .await
            .expect("should create a temporary test database");
        let pool = db.pool();
        let user_id = uuid::Uuid::new_v4();
        let player_id = uuid::Uuid::new_v4();

        let user = user_repo::insert_user(pool, user_id, "admin", "password-hash", UserRole::Admin)
            .await
            .expect("user should insert");
        assert_eq!(user.id, user_id);
        assert_eq!(user.role, "admin");

        let found_user = user_repo::find_user_by_username(pool, "admin")
            .await
            .expect("user lookup should succeed")
            .expect("user should exist");
        assert_eq!(found_user.id, user_id);

        let inactive_user = user_repo::set_user_active(pool, user_id, false)
            .await
            .expect("user update should succeed")
            .expect("user should exist");
        assert!(!inactive_user.active);

        let player = player_repo::insert_player(pool, player_id, "Alice", Some(user_id))
            .await
            .expect("player should insert");
        assert_eq!(player.id, player_id);
        assert_eq!(player.user_id, Some(user_id));

        let rating = player_repo::insert_default_rating(pool, player_id)
            .await
            .expect("rating should insert");
        assert_eq!(rating.player_id, player_id);
        assert_eq!(rating.rating, 25.0);

        let player_with_rating = player_repo::find_player_with_rating(pool, player_id)
            .await
            .expect("player lookup should succeed")
            .expect("player should exist");
        assert_eq!(player_with_rating.display_name, "Alice");
        assert_eq!(player_with_rating.games_played, 0);

        let inactive_player = player_repo::set_player_active(pool, player_id, false)
            .await
            .expect("player update should succeed")
            .expect("player should exist");
        assert!(!inactive_player.active);

        db.drop()
            .await
            .expect("should drop temporary test database");
    }
}
