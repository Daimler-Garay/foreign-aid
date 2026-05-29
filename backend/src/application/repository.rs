pub mod matches_repo;
pub mod player_repo;

pub type RepositoryResult<T> = Result<T, sqlx::Error>;
