use data_management::{
    check_song_link, create_match, create_pool, is_song_already_played, join_match,
    record_move, upsert_song, NewSong,
};
use uuid::Uuid;

#[tokio::test]
async fn test_database_gameplay_flow() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    let pool = create_pool(&db_url).await?;

    // 1. Insert two songs that share artist and producer
    let song1 = upsert_song(
        &pool,
        &NewSong {
            mbid: "test-song-1-queen-bohemian".to_string(),
            title: "Bohemian Rhapsody".to_string(),
            release_year: Some(1975),
            duration_ms: Some(354000),
            artists: vec!["Queen".to_string()],
            albums: vec!["A Night at the Opera".to_string()],
            credits: vec!["Freddie Mercury".to_string(), "Roy Thomas Baker".to_string()],
            locations: vec!["Rockfield Studios".to_string(), "London".to_string()],
        },
    )
    .await?;

    let song2 = upsert_song(
        &pool,
        &NewSong {
            mbid: "test-song-2-queen-killer".to_string(),
            title: "Killer Queen".to_string(),
            release_year: Some(1974),
            duration_ms: Some(181000),
            artists: vec!["Queen".to_string()],
            albums: vec!["Sheer Heart Attack".to_string()],
            credits: vec!["Freddie Mercury".to_string(), "Roy Thomas Baker".to_string()],
            locations: vec!["Rockfield Studios".to_string()],
        },
    )
    .await?;

    // 2. Insert a song that does NOT link
    let song3 = upsert_song(
        &pool,
        &NewSong {
            mbid: "test-song-3-daft-punk".to_string(),
            title: "Around the World".to_string(),
            release_year: Some(1997),
            duration_ms: Some(429000),
            artists: vec!["Daft Punk".to_string()],
            albums: vec!["Homework".to_string()],
            credits: vec!["Thomas Bangalter".to_string(), "Guy-Manuel".to_string()],
            locations: vec!["Paris".to_string()],
        },
    )
    .await?;

    // 3. Test link checking
    let link_1_2 = check_song_link(&pool, song1.id, song2.id).await?.expect("link check should return result");
    assert!(link_1_2.is_linked, "Bohemian Rhapsody and Killer Queen should link");
    assert_eq!(link_1_2.shared_artists, vec!["Queen".to_string()]);
    assert_eq!(link_1_2.shared_locations, vec!["Rockfield Studios".to_string()]);

    let link_1_3 = check_song_link(&pool, song1.id, song3.id).await?.expect("link check should return result");
    assert!(!link_1_3.is_linked, "Queen and Daft Punk should NOT link");

    // 4. Test match flow
    // First ensure two dummy users exist for FK constraints
    let player1_id = Uuid::new_v4();
    let player2_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash') ON CONFLICT DO NOTHING",
    )
    .bind(player1_id)
    .bind(format!("p1_{}", player1_id))
    .bind(format!("p1_{}@example.com", player1_id))
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash') ON CONFLICT DO NOTHING",
    )
    .bind(player2_id)
    .bind(format!("p2_{}", player2_id))
    .bind(format!("p2_{}@example.com", player2_id))
    .execute(&pool)
    .await?;

    // Create match and player 2 joins
    let m = create_match(&pool, player1_id).await?;
    assert_eq!(m.status, "waiting");

    let m = join_match(&pool, m.id, player2_id).await?;
    assert_eq!(m.status, "active");

    // Turn 1: Player 1 plays Bohemian Rhapsody
    let move1 = record_move(&pool, m.id, player1_id, 1, song1.id, None, None).await?;
    assert_eq!(move1.turn_number, 1);

    // Check repeat prevention
    assert!(is_song_already_played(&pool, m.id, song1.id).await?);
    assert!(!is_song_already_played(&pool, m.id, song2.id).await?);

    // Turn 2: Player 2 plays Killer Queen (linked by Artist)
    let move2 = record_move(
        &pool,
        m.id,
        player2_id,
        2,
        song2.id,
        Some("artist"),
        Some("Queen"),
    )
    .await?;
    assert_eq!(move2.turn_number, 2);
    assert_eq!(move2.link_type.as_deref(), Some("artist"));

    println!("All database gameplay tests passed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_populate_pop_songs_small() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    let pool = create_pool(&db_url).await?;

    // Import 3 songs from MusicBrainz
    let imported = data_management::populate_pop_songs(&pool, 3).await?;
    assert_eq!(imported, 3, "Expected 3 songs to be imported");

    Ok(())
}
