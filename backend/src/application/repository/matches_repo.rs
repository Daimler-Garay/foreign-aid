use uuid::Uuid;

use crate::{
    application::{repository::RepositoryResult, state::SharedState},
    domain::models::matches::{CreateMatchRequest, Match, MatchDetail, MatchPlayerDetail},
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

pub async fn get_match_detail_by_id(
    id: Uuid,
    state: &SharedState,
) -> RepositoryResult<MatchDetail> {
    let match_detail = sqlx::query_as::<_, Match>(r#"SELECT * FROM matches WHERE id = $1"#)
        .bind(id)
        .fetch_one(&state.db_pool)
        .await?;

    let players = sqlx::query_as::<_, MatchPlayerDetail>(
        r#"SELECT
            p.id,
            p.display_name,
            p.active,
            p.rating,
            p.rating_deviation,
            p.volatility,
            p.games_played,
            p.wins,
            p.losses,
            p.created_at,
            p.updated_at,

            mp.match_id,
            mp.player_id,
            mp.placement,
            mp.joined_at,
            mp.eliminated_at,
            mp.old_rating,
            mp.old_rating_deviation,
            mp.new_rating,
            mp.new_rating_deviation,
            mp.rating_delta
        FROM match_players mp
        JOIN players p ON p.id = mp.player_id
        WHERE mp.match_id = $1
        ORDER BY mp.joined_at ASC"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(MatchDetail {
        match_detail,
        players,
    })
}
