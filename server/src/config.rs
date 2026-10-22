use std::{env, error::Error, fmt, net::SocketAddr, time::Duration};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub session_ttl: Duration,
    pub cookie_secure: bool,
    pub bind_address: SocketAddr,
}

#[derive(Debug, PartialEq)]
pub struct ConfigError(&'static str);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        // validate env without exposing values in errors
        let database_url = lookup("DATABASE_URL")
            .filter(|value| !value.trim().is_empty())
            .ok_or(ConfigError("DATABASE_URL is required"))?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(ConfigError("DATABASE_URL must use PostgreSQL"));
        }
        // keep ttl positive and within signed limits
        let seconds = lookup("SESSION_TTL_SECONDS")
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0 && *value <= i64::MAX as u64)
            .ok_or(ConfigError(
                "SESSION_TTL_SECONDS must be a positive integer",
            ))?;
        let cookie_secure = match lookup("COOKIE_SECURE").as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => return Err(ConfigError("COOKIE_SECURE must be true or false")),
        };
        // default addr stays local
        let bind_address = lookup("BIND_ADDRESS")
            .unwrap_or_else(|| "127.0.0.1:3000".into())
            .parse()
            .map_err(|_| ConfigError("BIND_ADDRESS must be a socket address"))?;
        Ok(Self {
            database_url,
            session_ttl: Duration::from_secs(seconds),
            cookie_secure,
            bind_address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn values() -> HashMap<String, String> {
        [
            (
                "DATABASE_URL",
                "postgres://beat4beat:beat4beat@localhost:5432/beat4beat",
            ),
            ("SESSION_TTL_SECONDS", "604800"),
            ("COOKIE_SECURE", "false"),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect()
    }

    #[test]
    fn reads_local_config() {
        let values = values();
        let config = Config::from_lookup(|key| values.get(key).cloned()).unwrap();
        assert_eq!(config.session_ttl, Duration::from_secs(604800));
        assert!(!config.cookie_secure);
        assert_eq!(config.bind_address.to_string(), "127.0.0.1:3000");
    }

    #[test]
    fn rejects_missing_values() {
        for key in ["DATABASE_URL", "SESSION_TTL_SECONDS", "COOKIE_SECURE"] {
            let mut values = values();
            values.remove(key);
            assert!(Config::from_lookup(|key| values.get(key).cloned()).is_err());
        }
    }

    #[test]
    fn rejects_invalid_lifetime() {
        for value in ["0", "-1", "text", "9223372036854775808"] {
            let mut values = values();
            values.insert("SESSION_TTL_SECONDS".into(), value.into());
            assert!(Config::from_lookup(|key| values.get(key).cloned()).is_err());
        }
    }

    #[test]
    fn rejects_invalid_database_and_cookie_settings() {
        for (key, value) in [
            ("DATABASE_URL", ""),
            ("DATABASE_URL", "sqlite://test"),
            ("COOKIE_SECURE", "yes"),
            ("BIND_ADDRESS", "invalid"),
        ] {
            let mut values = values();
            values.insert(key.into(), value.into());
            assert!(Config::from_lookup(|key| values.get(key).cloned()).is_err());
        }
    }

    #[test]
    fn supports_secure_cookies_and_custom_address() {
        let mut values = values();
        values.insert("COOKIE_SECURE".into(), "true".into());
        values.insert("BIND_ADDRESS".into(), "127.0.0.1:4000".into());
        let config = Config::from_lookup(|key| values.get(key).cloned()).unwrap();
        assert!(config.cookie_secure);
        assert_eq!(config.bind_address.port(), 4000);
    }
}
