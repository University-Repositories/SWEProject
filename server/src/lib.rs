pub mod api_types;
pub mod app;
pub mod config;
pub mod error;
pub mod json;

use sqlx::{ConnectOptions, PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn connect_db(config: &config::Config) -> Result<PgPool, sqlx::Error> {
    // reuse db connections with a short timeout
    let options = config
        .database_url
        .parse::<sqlx::postgres::PgConnectOptions>()?;
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options.disable_statement_logging())
        .await
}
