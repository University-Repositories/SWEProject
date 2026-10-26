use data_management::{CatalogImporter, create_pool, load_default_subset, seed_dev_fixture};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    println!("Connecting to database at {}...", database_url);
    let pool = create_pool(&database_url).await?;

    let args: Vec<String> = env::args().collect();
    if args
        .iter()
        .any(|a| a == "--fixture" || a == "--seed-fixture")
    {
        println!("Seeding dev fixture into database...");
        seed_dev_fixture(&pool).await?;
        println!("Dev fixture seeded successfully!");
        return Ok(());
    }

    let min_credit_count: usize = env::var("MIN_CREDIT_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    println!(
        "Importing catalog data (min_credit_count = {})...",
        min_credit_count
    );

    let report = if args.len() > 1 && !args[1].starts_with("--") {
        let file_path = &args[1];
        println!("Reading input from file: {}", file_path);
        let content = std::fs::read_to_string(file_path)?;
        let importer = CatalogImporter::new(min_credit_count);
        importer.import_from_json(&pool, &content).await?
    } else {
        println!("Loading bundled MusicBrainz subset...");
        load_default_subset(&pool, min_credit_count).await?
    };

    println!("\nImport completed successfully!");
    print!("{}", report);

    Ok(())
}
