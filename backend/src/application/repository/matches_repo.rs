use uuid::Uuid;

use crate::{
    application::{repository::RepositoryResult, state::SharedState},
    domain::models::matches::{CreateMatchRequest, Match},
};

pub async fn create_match(
    host_player_id: Uuid,
    req: &CreateMatchRequest,
    state: &SharedState,
) -> RepositoryResult<Match> {
    // init pool connection
    let mut tx = state.db_pool.begin().await?;

    let match_row = sqlx::query_as::<_, Match>(
        "INSERT INTO matches (
         host_player_id, status, notes
    ) VALUES ($1, 'lobby', $2) RETURNING *",
    )
    .bind(host_player_id)
    .bind(&req.notes)
    // using tx here because the query needs to share same connection
    .fetch_one(&mut *tx)
    .await?;

    // failure here auto rollbacks the first
    sqlx::query("INSERT INTO match_players (match_id, player_id) VALUES ($1, $2)")
        .bind(match_row.id)
        .bind(host_player_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(match_row)
}

pub async fn get_match_by_id(id: Uuid, state: &SharedState) -> RepositoryResult<Match> {
    let query = sqlx::query_as::<_, Match>("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db_pool)
        .await?;

    Ok(query)
}
