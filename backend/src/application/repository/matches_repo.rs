use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    application::{repository::RepositoryResult, state::SharedState},
    domain::models::{
        matches::{
            CreateMatchRequest, Match, MatchDetail, MatchHistoryRow, MatchPlayer, MatchPlayerDetail,
        },
        players::Player,
    },
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

pub async fn get_match_history(state: &SharedState) -> RepositoryResult<Vec<MatchDetail>> {
    let rows = sqlx::query_as::<_, MatchHistoryRow>(
        r#"SELECT
            m.id            AS match_id,
            m.host_player_id,
            m.status,
            m.notes,
            m.created_at    AS match_created_at,
            m.started_at,
            m.completed_at,
            p.id            AS player_id,
            p.display_name,
            p.active,
            p.rating,
            p.rating_deviation,
            p.volatility,
            p.games_played,
            p.wins,
            p.losses,
            p.created_at    AS player_created_at,
            p.updated_at    AS player_updated_at,
            mp.placement,
            mp.joined_at,
            mp.eliminated_at,
            mp.old_rating,
            mp.old_rating_deviation,
            mp.new_rating,
            mp.new_rating_deviation,
            mp.rating_delta
        FROM matches m
        JOIN match_players mp ON mp.match_id = m.id
        JOIN players p ON p.id = mp.player_id
        WHERE m.status = 'completed'
        ORDER BY m.completed_at DESC, mp.joined_at ASC"#,
    )
    .fetch_all(&state.db_pool)
    .await?;

    let mut map: HashMap<Uuid, MatchDetail> = HashMap::new();
    let mut order: Vec<Uuid> = Vec::new();

    for row in rows {
        if !map.contains_key(&row.match_id) {
            order.push(row.match_id);
            map.insert(
                row.match_id,
                MatchDetail {
                    match_detail: Match {
                        id: row.match_id,
                        host_player_id: row.host_player_id,
                        status: row.status.clone(),
                        notes: row.notes.clone(),
                        created_at: row.match_created_at,
                        started_at: row.started_at,
                        completed_at: row.completed_at,
                    },
                    players: Vec::new(),
                },
            );
        }

        map.get_mut(&row.match_id)
            .unwrap()
            .players
            .push(MatchPlayerDetail {
                player: Player {
                    id: row.player_id,
                    display_name: row.display_name,
                    active: row.active,
                    rating: row.rating,
                    rating_deviation: row.rating_deviation,
                    volatility: row.volatility,
                    games_played: row.games_played,
                    wins: row.wins,
                    losses: row.losses,
                    created_at: row.player_created_at,
                    updated_at: row.player_updated_at,
                },
                match_player: MatchPlayer {
                    match_id: row.match_id,
                    player_id: row.player_id,
                    placement: row.placement,
                    joined_at: row.joined_at,
                    eliminated_at: row.eliminated_at,
                    old_rating: row.old_rating,
                    old_rating_deviation: row.old_rating_deviation,
                    new_rating: row.new_rating,
                    new_rating_deviation: row.new_rating_deviation,
                    rating_delta: row.rating_delta,
                },
            });
    }

    Ok(order.into_iter().filter_map(|id| map.remove(&id)).collect())
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
