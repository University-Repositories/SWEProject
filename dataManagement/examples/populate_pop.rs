use data_management::{create_pool, populate_pop_songs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    println!("Connecting to database at {}...", db_url);
    let pool = create_pool(&db_url).await?;

    // Populate 1000 songs in the pop genre
    let count = populate_pop_songs(&pool, 1000).await?;

    println!("Successfully imported {} pop songs!", count);
    Ok(())
}
