use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Conflict on constraint: {0}")]
    Conflict(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl RepoError {
    pub fn from_sqlx(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            // Postgres error code 23505 is unique_violation
            if db_err.code().as_deref() == Some("23505") {
                let constraint = db_err
                    .constraint()
                    .unwrap_or("unique_violation")
                    .to_string();
                return RepoError::Conflict(constraint);
            }
        }
        RepoError::Database(err)
    }
}
