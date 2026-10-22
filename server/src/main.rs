use server::{
    app::{AppState, router},
    config::Config,
    connect_db,
};
use std::{error::Error, io};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // load local env if present
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err("unable to read local environment file".into()),
    }
    let config = Config::from_env()?;
    // connect db before accepting requests
    let pool = connect_db(&config)
        .await
        .map_err(|_| io::Error::other("database connection failed"))?;
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    println!("server listening on {}", listener.local_addr()?);
    axum::serve(
        listener,
        router(AppState {
            pool: pool.clone(),
            config,
        }),
    )
    .with_graceful_shutdown(async {
        // wait for ctrl c before closing
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    pool.close().await;
    Ok(())
}
