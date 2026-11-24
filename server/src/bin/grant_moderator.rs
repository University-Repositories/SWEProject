use data_management::UserRepo;

use std::{error::Error, io};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let username = args
        .next()
        .filter(|_| args.next().is_none())
        .ok_or("usage: grant_moderator <username>")?;

    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?;
    let pool = data_management::create_pool(&database_url)
        .await
        .map_err(|_| "database connection failed")?;
    data_management::run_migrations(&pool).await?;

    // the acct has to exist first
    let users = UserRepo::new(pool);
    let user = users
        .find_by_username(&username)
        .await?
        .ok_or("user not found")?;
    if !users.set_role(user.id, "moderator").await? {
        return Err("user not found".into());
    }
    println!("granted moderator to {}", user.username);
    Ok(())
}
