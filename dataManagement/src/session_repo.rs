use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::RepoError;
use crate::models::SessionRow;

#[derive(Debug, Clone)]
pub struct SessionRepo {
    pool: PgPool,
}

impl SessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepoError> {
        Self::create_with_executor(&self.pool, user_id, token_hash, expires_at).await
    }

    pub async fn create_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query(
            r#"
            INSERT INTO sessions (token_hash, user_id, expires_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(expires_at)
        .execute(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(())
    }

    pub async fn find_valid(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRow>, RepoError> {
        Self::find_valid_with_executor(&self.pool, token_hash, now).await
    }

    pub async fn find_valid_with_executor<'e, E>(
        executor: E,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT token_hash, user_id, created_at, expires_at
            FROM sessions
            WHERE token_hash = $1 AND expires_at > $2
            "#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn delete(&self, token_hash: &[u8]) -> Result<(), RepoError> {
        Self::delete_with_executor(&self.pool, token_hash).await
    }

    pub async fn delete_with_executor<'e, E>(
        executor: E,
        token_hash: &[u8],
    ) -> Result<(), RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        // Deleting a missing session is not an error
        sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .execute(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(())
    }
}
