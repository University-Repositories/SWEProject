use crate::{api_types::ApiError, config::Config};
use authentication::{
    AccountError,
    session::{SessionToken, hash_token},
};
use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
use cookie::{Cookie, SameSite, time::Duration};

pub const SESSION_COOKIE: &str = "b4b_session";

fn header(cookie: Cookie<'_>) -> Result<HeaderValue, ApiError> {
    let mut value = HeaderValue::from_str(&cookie.to_string()).map_err(|_| ApiError::internal())?;
    // i want the auth header hidden if debug output is used later
    value.set_sensitive(true);
    Ok(value)
}

pub fn session_cookie(token: &SessionToken, config: &Config) -> Result<HeaderValue, ApiError> {
    // i want the cookie ttl and secure flag to follow config
    let seconds = i64::try_from(config.session_ttl.as_secs())
        .ok()
        .filter(|seconds| *seconds > 0)
        .ok_or_else(ApiError::internal)?;
    header(
        Cookie::build((SESSION_COOKIE, token.as_str()))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .secure(config.cookie_secure)
            .max_age(Duration::seconds(seconds))
            .build(),
    )
}

pub fn clear_session_cookie(config: &Config) -> Result<HeaderValue, ApiError> {
    // i want logout to clear the same auth cookie path
    header(
        Cookie::build((SESSION_COOKIE, ""))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .secure(config.cookie_secure)
            .max_age(Duration::ZERO)
            .build(),
    )
}

pub fn read_session_token(headers: &HeaderMap) -> Result<String, AccountError> {
    let mut token = None;
    for value in headers.get_all(COOKIE) {
        let value = value.to_str().map_err(|_| AccountError::SessionInvalid)?;
        for cookie in Cookie::split_parse(value) {
            let cookie = cookie.map_err(|_| AccountError::SessionInvalid)?;
            if cookie.name() == SESSION_COOKIE {
                // i want duplicate auth cookies rejected instead of guessing
                if token.is_some() {
                    return Err(AccountError::SessionInvalid);
                }
                hash_token(cookie.value())?;
                token = Some(cookie.value().to_owned());
            }
        }
    }
    // i want the db check left to session middleware
    token.ok_or(AccountError::SessionInvalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    fn config(secure: bool) -> Config {
        Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/beat4beat".into()),
            "SESSION_TTL_SECONDS" => Some("604800".into()),
            "COOKIE_SECURE" => Some(secure.to_string()),
            _ => None,
        })
        .unwrap()
    }

    #[test]
    fn sets_required_cookie_attributes() {
        let token = SessionToken::generate().unwrap();
        for secure in [false, true] {
            let header = session_cookie(&token, &config(secure)).unwrap();
            assert!(header.is_sensitive());
            let cookie = Cookie::parse(header.to_str().unwrap()).unwrap();
            assert_eq!(cookie.name(), SESSION_COOKIE);
            assert!(cookie.value() == token.as_str());
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.same_site(), Some(SameSite::Lax));
            assert_eq!(cookie.path(), Some("/"));
            assert_eq!(cookie.secure().unwrap_or(false), secure);
            assert_eq!(cookie.max_age().unwrap().whole_seconds(), 604800);
            assert!(cookie.domain().is_none());
        }
    }

    #[test]
    fn uses_custom_ttl_and_rejects_invalid_lifetime() {
        let token = SessionToken::generate().unwrap();
        let mut config = config(false);
        config.session_ttl = StdDuration::from_secs(60);
        let value = session_cookie(&token, &config).unwrap();
        let cookie = Cookie::parse(value.to_str().unwrap()).unwrap();
        assert_eq!(cookie.max_age().unwrap().whole_seconds(), 60);
        for ttl in [StdDuration::ZERO, StdDuration::MAX] {
            config.session_ttl = ttl;
            assert!(session_cookie(&token, &config).is_err());
        }
    }

    #[test]
    fn clears_cookie_with_same_scope() {
        for secure in [false, true] {
            let value = clear_session_cookie(&config(secure)).unwrap();
            let cookie = Cookie::parse(value.to_str().unwrap()).unwrap();
            assert_eq!(cookie.name(), SESSION_COOKIE);
            assert_eq!(cookie.value(), "");
            assert_eq!(cookie.path(), Some("/"));
            assert_eq!(cookie.max_age().unwrap().whole_seconds(), 0);
            assert_eq!(cookie.secure().unwrap_or(false), secure);
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        }
    }

    #[test]
    fn reads_session_among_other_cookies() {
        let token = SessionToken::generate().unwrap();
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("theme=dark"));
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("other=value; {SESSION_COOKIE}={}", token.as_str()))
                .unwrap(),
        );
        assert!(read_session_token(&headers).unwrap() == token.as_str());
    }

    #[test]
    fn rejects_missing_or_malformed_session_cookie() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            read_session_token(&headers),
            Err(AccountError::SessionInvalid)
        );
        for value in ["theme=dark", "b4b_session=", "b4b_session=bad", "broken"] {
            headers.insert(COOKIE, HeaderValue::from_str(value).unwrap());
            assert_eq!(
                read_session_token(&headers),
                Err(AccountError::SessionInvalid)
            );
        }
        headers.insert(COOKIE, HeaderValue::from_bytes(&[0xff]).unwrap());
        assert_eq!(
            read_session_token(&headers),
            Err(AccountError::SessionInvalid)
        );
    }

    #[test]
    fn rejects_duplicate_session_cookies() {
        let first = SessionToken::generate().unwrap();
        let second = SessionToken::generate().unwrap();
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={}", first.as_str())).unwrap(),
        );
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={}", second.as_str())).unwrap(),
        );
        assert_eq!(
            read_session_token(&headers),
            Err(AccountError::SessionInvalid)
        );
        headers.clear();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={}; {SESSION_COOKIE}={}",
                first.as_str(),
                first.as_str()
            ))
            .unwrap(),
        );
        assert_eq!(
            read_session_token(&headers),
            Err(AccountError::SessionInvalid)
        );
    }
}
