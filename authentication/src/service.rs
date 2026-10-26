use crate::{
    AccountError,
    password::{hash_password, verify_password},
    session::{SessionToken, hash_token, session_expires_at},
    validation::{validate_password, validate_username},
};
use chrono::{DateTime, Utc};
use data_management::{RepoError, SessionRepo, UserRepo, UserRow};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

/// Public account data. The password hash never leaves this crate.
#[derive(Debug)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub role: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<UserRow> for UserRecord {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            role: row.role,
            display_name: row.display_name,
            created_at: row.created_at,
        }
    }
}

pub struct LoginSession {
    pub token: SessionToken,
    pub expires_at: DateTime<Utc>,
}

pub async fn register(
    pool: &PgPool,
    username: &str,
    password: &str,
) -> Result<UserRecord, AccountError> {
    validate_username(username)?;
    let password_hash = hash_password(password).await?;
    // i want new accts to start with the db player role
    UserRepo::new(pool.clone())
        .create(username, &password_hash)
        .await
        .map(UserRecord::from)
        .map_err(registration_error)
}

fn registration_error(error: RepoError) -> AccountError {
    // i want only the username db conflict exposed to the user
    match error {
        RepoError::Conflict(constraint) if constraint == "users_lower_username_idx" => {
            AccountError::UsernameTaken
        }
        _ => AccountError::Internal,
    }
}

pub async fn login(
    pool: &PgPool,
    username: &str,
    password: &str,
    ttl: Duration,
) -> Result<LoginSession, AccountError> {
    // i want bad login input to use one auth error
    validate_username(username).map_err(|_| AccountError::InvalidCredentials)?;
    validate_password(password).map_err(|_| AccountError::InvalidCredentials)?;
    let user = UserRepo::new(pool.clone())
        .find_by_username(username)
        .await
        .map_err(|_| AccountError::Internal)?
        .ok_or(AccountError::InvalidCredentials)?;
    verify_password(password, &user.password_hash).await?;
    let expires_at = session_expires_at(Utc::now(), ttl)?;
    let token = SessionToken::generate()?;
    SessionRepo::new(pool.clone())
        .create(user.id, &token.hash(), expires_at)
        .await
        .map_err(|_| AccountError::Internal)?;
    Ok(LoginSession { token, expires_at })
}

pub async fn authenticate(pool: &PgPool, raw_token: &str) -> Result<UserRecord, AccountError> {
    let token_hash = hash_token(raw_token)?;
    // i want every auth check to use the saved session expiry
    let session = SessionRepo::new(pool.clone())
        .find_valid(&token_hash, Utc::now())
        .await
        .map_err(|_| AccountError::Internal)?
        .ok_or(AccountError::SessionInvalid)?;
    UserRepo::new(pool.clone())
        .find_by_id(session.user_id)
        .await
        .map_err(|_| AccountError::Internal)?
        .map(UserRecord::from)
        .ok_or(AccountError::SessionInvalid)
}

pub async fn logout(pool: &PgPool, raw_token: &str) -> Result<(), AccountError> {
    let token_hash = hash_token(raw_token)?;
    SessionRepo::new(pool.clone())
        .delete(&token_hash)
        .await
        .map_err(|_| AccountError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn rejects_bad_input_without_a_db_connection() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        assert!(matches!(
            register(&pool, "ab", "validpass").await,
            Err(AccountError::InvalidUsername)
        ));
        assert!(matches!(
            register(&pool, "Sam", "short").await,
            Err(AccountError::InvalidPassword)
        ));
        for (username, password) in [("ab", "validpass"), ("Sam", "short")] {
            assert!(matches!(
                login(&pool, username, password, Duration::from_secs(60)).await,
                Err(AccountError::InvalidCredentials)
            ));
        }
        assert!(matches!(
            authenticate(&pool, "bad").await,
            Err(AccountError::SessionInvalid)
        ));
        assert_eq!(
            logout(&pool, "bad").await,
            Err(AccountError::SessionInvalid)
        );
    }

    #[test]
    fn keeps_unexpected_db_errors_private() {
        assert_eq!(
            registration_error(RepoError::Database(sqlx::Error::PoolClosed)),
            AccountError::Internal
        );
    }
}
