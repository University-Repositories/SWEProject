use crate::{AccountError, validation::validate_password};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand_core::{OsRng, RngCore};

pub async fn hash_password(password: &str) -> Result<String, AccountError> {
    validate_password(password)?;
    let password = password.to_owned();
    // i want pw hashing off the async request thread
    tokio::task::spawn_blocking(move || {
        let mut salt_bytes = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut salt_bytes)
            .map_err(|_| AccountError::Internal)?;
        let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| AccountError::Internal)?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| AccountError::Internal)
    })
    .await
    .map_err(|_| AccountError::Internal)?
}

pub async fn verify_password(password: &str, stored_hash: &str) -> Result<(), AccountError> {
    // i want invalid pw input to use the same login error
    validate_password(password).map_err(|_| AccountError::InvalidCredentials)?;
    let password = password.to_owned();
    let stored_hash = stored_hash.to_owned();
    tokio::task::spawn_blocking(move || {
        let hash = PasswordHash::new(&stored_hash).map_err(|_| AccountError::Internal)?;
        match Argon2::default().verify_password(password.as_bytes(), &hash) {
            Ok(()) => Ok(()),
            Err(argon2::password_hash::Error::Password) => Err(AccountError::InvalidCredentials),
            Err(_) => Err(AccountError::Internal),
        }
    })
    .await
    .map_err(|_| AccountError::Internal)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_with_argon2id_and_checks_password() {
        let hash = hash_password("TestPass123!").await.unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains("TestPass123!"));
        assert_eq!(verify_password("TestPass123!", &hash).await, Ok(()));
        assert_eq!(
            verify_password("WrongPass123!", &hash).await,
            Err(AccountError::InvalidCredentials)
        );
    }

    #[tokio::test]
    async fn salts_each_password_separately() {
        let first = hash_password("TestPass123!").await.unwrap();
        let second = hash_password("TestPass123!").await.unwrap();
        assert!(first != second);
        assert_eq!(verify_password("TestPass123!", &second).await, Ok(()));
    }

    #[tokio::test]
    async fn preserves_password_spaces_and_case() {
        let hash = hash_password(" TestPass123! ").await.unwrap();
        assert_eq!(verify_password(" TestPass123! ", &hash).await, Ok(()));
        assert_eq!(
            verify_password("TestPass123!", &hash).await,
            Err(AccountError::InvalidCredentials)
        );
        assert_eq!(
            verify_password(" testpass123! ", &hash).await,
            Err(AccountError::InvalidCredentials)
        );
    }

    #[tokio::test]
    async fn rejects_invalid_password_before_hashing() {
        assert_eq!(
            hash_password("short12").await,
            Err(AccountError::InvalidPassword)
        );
        assert_eq!(
            hash_password(&"a".repeat(129)).await,
            Err(AccountError::InvalidPassword)
        );
    }

    #[tokio::test]
    async fn treats_bad_login_input_as_invalid_credentials() {
        assert_eq!(
            verify_password("short", "unused").await,
            Err(AccountError::InvalidCredentials)
        );
        assert_eq!(
            verify_password(&"a".repeat(129), "unused").await,
            Err(AccountError::InvalidCredentials)
        );
    }

    #[tokio::test]
    async fn handles_corrupt_stored_hash_without_panicking() {
        assert_eq!(
            verify_password("TestPass123!", "not a hash").await,
            Err(AccountError::Internal)
        );
    }
}
