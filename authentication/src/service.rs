use crate::{
    AccountError,
    password::{hash_password, verify_password},
    session::{SessionToken, hash_token, session_expires_at},
    validation::{validate_password, validate_username},
};
use chrono::{DateTime, Utc};
use data_management::accounts::{self, UserRecord};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

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
    accounts::insert_user(pool, Uuid::new_v4(), username, &password_hash)
        .await
        .map_err(registration_error)
}

fn registration_error(error: sqlx::Error) -> AccountError {
    // i want only the username db conflict exposed to the user
    if error.as_database_error().is_some_and(|error| {
        error.is_unique_violation() && error.constraint() == Some("users_username_key")
    }) {
        return AccountError::UsernameTaken;
    }
    AccountError::Internal
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
    let credentials = accounts::find_credentials(pool, username)
        .await
        .map_err(|_| AccountError::Internal)?
        .ok_or(AccountError::InvalidCredentials)?;
    verify_password(password, &credentials.password_hash).await?;
    let expires_at = session_expires_at(Utc::now(), ttl)?;
    let token = SessionToken::generate()?;
    accounts::insert_session(pool, &token.hash(), credentials.id, expires_at)
        .await
        .map_err(|_| AccountError::Internal)?;
    Ok(LoginSession { token, expires_at })
}

pub async fn authenticate(pool: &PgPool, raw_token: &str) -> Result<UserRecord, AccountError> {
    let token_hash = hash_token(raw_token)?;
    // i want every auth check to use the saved session expiry
    accounts::session_user(pool, &token_hash)
        .await
        .map_err(|_| AccountError::Internal)?
        .ok_or(AccountError::SessionInvalid)
}

pub async fn logout(pool: &PgPool, raw_token: &str) -> Result<(), AccountError> {
    let token_hash = hash_token(raw_token)?;
    accounts::delete_session(pool, &token_hash)
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
            registration_error(sqlx::Error::PoolClosed),
            AccountError::Internal
        );
    }
}
