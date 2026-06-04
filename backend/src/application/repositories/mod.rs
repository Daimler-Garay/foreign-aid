pub mod audit_repo;
pub mod match_repo;
pub mod player_repo;
pub mod rating_repo;
pub mod session_repo;
pub mod user_repo;

pub type RepositoryResult<T> = Result<T, sqlx::Error>;
