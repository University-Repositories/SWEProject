use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, migrate::MigrateError};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub role: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub struct Credentials {
    pub id: Uuid,
    pub password_hash: String,
}

pub async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    // i want versioned db changes applied once
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn insert_user(
    pool: &PgPool,
    id: Uuid,
    username: &str,
    password_hash: &str,
) -> Result<UserRecord, sqlx::Error> {
    // i want the db unique rule to catch duplicate names
    sqlx::query_as(
        "INSERT INTO users (id, username, password_hash) VALUES ($1, $2, $3)
         RETURNING id, username, role, display_name, created_at",
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

pub async fn find_credentials(
    pool: &PgPool,
    username: &str,
) -> Result<Option<Credentials>, sqlx::Error> {
    // i want the pw hash kept separate from public user data
    sqlx::query_as("SELECT id, password_hash FROM users WHERE username = $1")
        .bind(username)
        .fetch_optional(pool)
        .await
}

pub async fn insert_session(
    pool: &PgPool,
    token_hash: &[u8; 32],
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    // i want only the auth token hash saved in the db
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(token_hash.as_slice())
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn session_user(
    pool: &PgPool,
    token_hash: &[u8; 32],
) -> Result<Option<UserRecord>, sqlx::Error> {
    // i want expired auth sessions rejected by the db clock
    sqlx::query_as(
        "SELECT u.id, u.username, u.role, u.display_name, u.created_at
         FROM users u JOIN sessions s ON s.user_id = u.id
         WHERE s.token_hash = $1 AND s.expires_at > clock_timestamp()",
    )
    .bind(token_hash.as_slice())
    .fetch_optional(pool)
    .await
}

pub async fn delete_session(pool: &PgPool, token_hash: &[u8; 32]) -> Result<(), sqlx::Error> {
    // i want logout to remove only the matching auth session
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash.as_slice())
        .execute(pool)
        .await?;
    Ok(())
}
