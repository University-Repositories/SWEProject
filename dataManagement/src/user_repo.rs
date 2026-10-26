use sqlx::PgPool;
use uuid::Uuid;

use crate::error::RepoError;
use crate::models::UserRow;

#[derive(Debug, Clone)]
pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create(&self, username: &str, password_hash: &str) -> Result<UserRow, RepoError> {
        Self::create_with_executor(&self.pool, username, password_hash).await
    }

    pub async fn create_with_executor<'e, E>(
        executor: E,
        username: &str,
        password_hash: &str,
    ) -> Result<UserRow, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (username, password_hash)
            VALUES ($1, $2)
            RETURNING id, username, password_hash, role, display_name, created_at
            "#,
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserRow>, RepoError> {
        Self::find_by_username_with_executor(&self.pool, username).await
    }

    pub async fn find_by_username_with_executor<'e, E>(
        executor: E,
        username: &str,
    ) -> Result<Option<UserRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, password_hash, role, display_name, created_at
            FROM users
            WHERE lower(username) = lower($1)
            "#,
        )
        .bind(username)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRow>, RepoError> {
        Self::find_by_id_with_executor(&self.pool, id).await
    }

    pub async fn find_by_id_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, password_hash, role, display_name, created_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}
