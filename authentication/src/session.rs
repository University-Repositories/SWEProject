use crate::AccountError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub struct SessionToken(String);

impl SessionToken {
    pub fn generate() -> Result<Self, AccountError> {
        // i want a fresh auth token from random bytes
        let mut bytes = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| AccountError::Internal)?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_bytes()).into()
    }
}

pub fn hash_token(raw_token: &str) -> Result<[u8; 32], AccountError> {
    // i want bad auth tokens rejected before any db lookup
    if raw_token.len() != 43 {
        return Err(AccountError::SessionInvalid);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(raw_token)
        .map_err(|_| AccountError::SessionInvalid)?;
    if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(&decoded) != raw_token {
        return Err(AccountError::SessionInvalid);
    }
    Ok(Sha256::digest(raw_token.as_bytes()).into())
}

pub fn session_expires_at(
    now: DateTime<Utc>,
    ttl: Duration,
) -> Result<DateTime<Utc>, AccountError> {
    // i want expiry based on the ttl config without overflow
    if ttl.is_zero() {
        return Err(AccountError::Internal);
    }
    let delta = TimeDelta::from_std(ttl).map_err(|_| AccountError::Internal)?;
    now.checked_add_signed(delta).ok_or(AccountError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_unique_tokens_with_32_random_bytes() {
        let first = SessionToken::generate().unwrap();
        let second = SessionToken::generate().unwrap();
        assert_eq!(first.as_str().len(), 43);
        assert_eq!(URL_SAFE_NO_PAD.decode(first.as_str()).unwrap().len(), 32);
        assert!(first.as_str() != second.as_str());
        assert!(!first.as_str().contains(['+', '/', '=']));
    }

    #[test]
    fn hashes_the_token_for_storage_and_lookup() {
        let token = SessionToken::generate().unwrap();
        assert!(token.hash() == hash_token(token.as_str()).unwrap());
        assert!(token.hash().as_slice() != token.as_str().as_bytes());
        assert!(token.hash() != SessionToken::generate().unwrap().hash());
    }

    #[test]
    fn matches_known_sha256_digest() {
        let raw = URL_SAFE_NO_PAD.encode([0u8; 32]);
        let digest = hash_token(&raw).unwrap();
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "0f007385b6f9d4b7eeb2748605afe1a984a0a3bfa3f014d09e2a784ce9e5cd1a"
        );
    }

    #[test]
    fn rejects_malformed_tokens() {
        let valid = URL_SAFE_NO_PAD.encode([0u8; 32]);
        for raw in [
            "".to_owned(),
            "short".into(),
            "a".repeat(42),
            "a".repeat(44),
            "!".repeat(43),
            format!("{valid}="),
            format!("{}B", "A".repeat(42)),
        ] {
            assert_eq!(hash_token(&raw), Err(AccountError::SessionInvalid));
        }
    }

    #[test]
    fn uses_configured_session_lifetime() {
        let now = "2026-10-24T12:00:00Z".parse().unwrap();
        let expiry = session_expires_at(now, Duration::from_secs(604800)).unwrap();
        assert_eq!(expiry.to_rfc3339(), "2026-10-31T12:00:00+00:00");
        assert_eq!(
            session_expires_at(now, Duration::from_secs(60)).unwrap() - now,
            TimeDelta::seconds(60)
        );
    }

    #[test]
    fn rejects_zero_and_overflowing_lifetime() {
        let now = "2026-10-24T12:00:00Z".parse().unwrap();
        assert_eq!(
            session_expires_at(now, Duration::ZERO),
            Err(AccountError::Internal)
        );
        assert_eq!(
            session_expires_at(now, Duration::MAX),
            Err(AccountError::Internal)
        );
        assert_eq!(
            session_expires_at(DateTime::<Utc>::MAX_UTC, Duration::from_secs(1)),
            Err(AccountError::Internal)
        );
    }
}
