use uuid::Uuid;

use crate::{
    application::{repository::RepositoryResult, state::SharedState},
    domain::models::matches::{Match, MatchPlayer},
};

pub async fn create_match() {
    todo!()
}

pub async fn get_match_by_id(id: Uuid, state: &SharedState) -> RepositoryResult<Match> {
    let query = sqlx::query_as::<_, Match>("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db_pool)
        .await?;

    Ok(query)
}

