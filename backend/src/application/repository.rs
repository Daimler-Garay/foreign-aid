pub mod player;

pub type RepositoryResult<T> = Result<T, sqlx::Error>;
