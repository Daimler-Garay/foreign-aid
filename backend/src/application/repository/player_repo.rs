use uuid::Uuid;

use crate::{
    application::{repository::RepositoryResult, state::SharedState},
    domain::models::players::{CreatePlayer, Player},
};

pub async fn add_player(
    player: CreatePlayer,
    state: &SharedState,
) -> RepositoryResult<CreatePlayer> {
    tracing::trace!("player: {:#?}", player);
    let player = sqlx::query_as::<_, CreatePlayer>(
        r#"INSERT into players (
            display_name, active
        ) VALUES ($1, $2) RETURNING id, display_name, active, rating, rating_deviation, volatility, games_played, wins, losses, created_at, updated_at"#
    ).bind(&player.display_name).bind(true).fetch_one(&state.db_pool).await?;

    Ok(player)
}

pub async fn get_player_by_id(id: Uuid, state: &SharedState) -> RepositoryResult<Player> {
    let player = sqlx::query_as::<_, Player>(r#"SELECT * FROM players WHERE id = $1"#)
        .bind(id)
        .fetch_one(&state.db_pool)
        .await?;
    Ok(player)
}

pub async fn list_players(state: &SharedState) -> RepositoryResult<Vec<Player>> {
    let player = sqlx::query_as::<_, Player>(r#"SELECT * FROM players"#)
        .fetch_all(&state.db_pool)
        .await?;
    Ok(player)
}
