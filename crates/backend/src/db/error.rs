//! Database error types

use deadpool_postgres::PoolError;

#[derive(Debug)]
pub enum DbError {
    Pool(PoolError),
    Postgres(tokio_postgres::Error),
}

impl From<PoolError> for DbError {
    fn from(e: PoolError) -> Self {
        DbError::Pool(e)
    }
}

impl From<tokio_postgres::Error> for DbError {
    fn from(e: tokio_postgres::Error) -> Self {
        DbError::Postgres(e)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Pool(e) => write!(f, "Pool error: {}", e),
            DbError::Postgres(e) => write!(f, "Postgres error: {}", e),
        }
    }
}
