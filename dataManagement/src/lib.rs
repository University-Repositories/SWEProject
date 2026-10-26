use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

pub mod importer;
pub use importer::{
    CatalogImporter, CoverageReport, ImportCredit, ImportRecording, load_default_subset,
    populate_pop_songs,
};

// -----------------------------------------------------------------------------
// Database Helpers
// -----------------------------------------------------------------------------

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn seed_dev_fixture(pool: &PgPool) -> Result<(), sqlx::Error> {
    const FIXTURE_SQL: &str = include_str!("../dev_fixture.sql");
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(FIXTURE_SQL).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Error Handling
// -----------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Conflict on constraint: {0}")]
    Conflict(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl RepoError {
    pub fn from_sqlx(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err
            && db_err.code().as_deref() == Some("23505")
        {
            let constraint = db_err
                .constraint()
                .unwrap_or("unique_violation")
                .to_string();
            return RepoError::Conflict(constraint);
        }
        RepoError::Database(err)
    }
}

// -----------------------------------------------------------------------------
// Models
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct SessionRow {
    pub token_hash: Vec<u8>,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct RecordingSummaryRow {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct CreditEntityRow {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingCredit {
    pub credit_id: Uuid,
    pub name: String,
    pub kind: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingDetailRow {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
    pub credits: Vec<RecordingCredit>,
}

// -----------------------------------------------------------------------------
// UserRepo
// -----------------------------------------------------------------------------

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
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id, username, password_hash, role, display_name, created_at",
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
            "SELECT id, username, password_hash, role, display_name, created_at FROM users WHERE lower(username) = lower($1)",
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
            "SELECT id, username, password_hash, role, display_name, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}

// -----------------------------------------------------------------------------
// SessionRepo
// -----------------------------------------------------------------------------

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
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
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
            "SELECT token_hash, user_id, created_at, expires_at FROM sessions WHERE token_hash = $1 AND expires_at > $2",
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
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(executor)
            .await
            .map_err(RepoError::from_sqlx)?;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// CatalogRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CatalogRepo {
    pool: PgPool,
}

impl CatalogRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<RecordingSummaryRow>, RepoError> {
        Self::search_with_executor(&self.pool, query, limit).await
    }

    pub async fn search_with_executor<'e, E>(
        executor: E,
        query: &str,
        limit: i64,
    ) -> Result<Vec<RecordingSummaryRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, RecordingSummaryRow>(
            r#"
            SELECT id, title, artist_name, year, release_id
            FROM recordings
            ORDER BY
                GREATEST(similarity(title, $1), similarity(artist_name, $1)) DESC,
                year ASC NULLS LAST,
                id ASC
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get_recording(&self, id: Uuid) -> Result<Option<RecordingDetailRow>, RepoError> {
        Self::get_recording_with_executor(&self.pool, id).await
    }

    pub async fn get_recording_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<RecordingDetailRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e> + Copy,
    {
        let rec = sqlx::query_as::<_, RecordingSummaryRow>(
            "SELECT id, title, artist_name, year, release_id FROM recordings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        let rec = match rec {
            Some(r) => r,
            None => return Ok(None),
        };

        let credits_rows = sqlx::query(
            r#"
            SELECT rc.credit_id, ce.name, ce.kind, rc.role
            FROM recording_credits rc
            JOIN credit_entities ce ON ce.id = rc.credit_id
            WHERE rc.recording_id = $1
            ORDER BY rc.role ASC, ce.name ASC
            "#,
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        let credits = credits_rows
            .into_iter()
            .map(|r| RecordingCredit {
                credit_id: r.get("credit_id"),
                name: r.get("name"),
                kind: r.get("kind"),
                role: r.get("role"),
            })
            .collect();

        Ok(Some(RecordingDetailRow {
            id: rec.id,
            title: rec.title,
            artist_name: rec.artist_name,
            year: rec.year,
            release_id: rec.release_id,
            credits,
        }))
    }

    pub async fn get_credit_ids(&self, id: Uuid) -> Result<Vec<Uuid>, RepoError> {
        Self::get_credit_ids_with_executor(&self.pool, id).await
    }

    pub async fn get_credit_ids_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Vec<Uuid>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let rows = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT credit_id FROM recording_credits WHERE recording_id = $1 ORDER BY credit_id ASC",
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(rows)
    }
}
