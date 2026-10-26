use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::{LinkValidation, MatchMove, MatchSession, NewSong, Song};

/// Connect to PostgreSQL using a connection pool.
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

// ---------------------------------------------------------
// SONG HELPERS
// ---------------------------------------------------------

/// Inserts a new song into the database, or updates existing on mbid conflict.
pub async fn upsert_song(pool: &PgPool, song: &NewSong) -> Result<Song, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r#"
        INSERT INTO songs (mbid, title, release_year, duration_ms, artists, albums, credits, locations)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (mbid) DO UPDATE SET
            title = EXCLUDED.title,
            release_year = COALESCE(EXCLUDED.release_year, songs.release_year),
            duration_ms = COALESCE(EXCLUDED.duration_ms, songs.duration_ms),
            artists = EXCLUDED.artists,
            albums = EXCLUDED.albums,
            credits = EXCLUDED.credits,
            locations = EXCLUDED.locations
        RETURNING id, mbid, title, release_year, duration_ms, artists, albums, credits, locations, created_at
        "#,
    )
    .bind(&song.mbid)
    .bind(&song.title)
    .bind(song.release_year)
    .bind(song.duration_ms)
    .bind(&song.artists)
    .bind(&song.albums)
    .bind(&song.credits)
    .bind(&song.locations)
    .fetch_one(pool)
    .await
}

/// Fetches a song by its internal database UUID.
pub async fn get_song_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Song>, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r#"
        SELECT id, mbid, title, release_year, duration_ms, artists, albums, credits, locations, created_at
        FROM songs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Searches for songs matching a query title (case-insensitive substring match).
pub async fn search_songs_by_title(pool: &PgPool, query: &str, limit: i64) -> Result<Vec<Song>, sqlx::Error> {
    let pattern = format!("%{}%", query);
    sqlx::query_as::<_, Song>(
        r#"
        SELECT id, mbid, title, release_year, duration_ms, artists, albums, credits, locations, created_at
        FROM songs
        WHERE title ILIKE $1
        ORDER BY title ASC
        LIMIT $2
        "#,
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Validates whether two songs are legally connected according to the 5 mechanics.
/// Evaluates array overlaps and year matching directly inside PostgreSQL.
pub async fn check_song_link(
    pool: &PgPool,
    song1_id: Uuid,
    song2_id: Uuid,
) -> Result<Option<LinkValidation>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT 
            (s1.artists && s2.artists) 
                OR (s1.albums && s2.albums) 
                OR (s1.release_year IS NOT NULL AND s1.release_year = s2.release_year)
                OR (s1.credits && s2.credits) 
                OR (s1.locations && s2.locations) AS is_linked,
            ARRAY(SELECT unnest(s1.artists) INTERSECT SELECT unnest(s2.artists)) AS shared_artists,
            ARRAY(SELECT unnest(s1.albums) INTERSECT SELECT unnest(s2.albums)) AS shared_albums,
            ARRAY(SELECT unnest(s1.credits) INTERSECT SELECT unnest(s2.credits)) AS shared_credits,
            ARRAY(SELECT unnest(s1.locations) INTERSECT SELECT unnest(s2.locations)) AS shared_locations,
            CASE WHEN s1.release_year IS NOT NULL AND s1.release_year = s2.release_year 
                 THEN s1.release_year ELSE NULL END AS shared_year
        FROM songs s1, songs s2
        WHERE s1.id = $1 AND s2.id = $2
        "#,
    )
    .bind(song1_id)
    .bind(song2_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let is_linked: bool = r.get("is_linked");
        let shared_artists: Vec<String> = r.get("shared_artists");
        let shared_albums: Vec<String> = r.get("shared_albums");
        let shared_credits: Vec<String> = r.get("shared_credits");
        let shared_locations: Vec<String> = r.get("shared_locations");
        let shared_year: Option<i32> = r.get("shared_year");

        LinkValidation {
            is_linked,
            shared_artists,
            shared_albums,
            shared_year,
            shared_credits,
            shared_locations,
        }
    }))
}

/// Finds legal candidate songs that can link to the current song.
pub async fn find_candidate_songs(
    pool: &PgPool,
    current_song_id: Uuid,
    limit: i64,
) -> Result<Vec<Song>, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r#"
        SELECT s2.id, s2.mbid, s2.title, s2.release_year, s2.duration_ms, 
               s2.artists, s2.albums, s2.credits, s2.locations, s2.created_at
        FROM songs s1, songs s2
        WHERE s1.id = $1
          AND s2.id != s1.id
          AND (
              s1.artists && s2.artists
              OR s1.albums && s2.albums
              OR (s1.release_year IS NOT NULL AND s1.release_year = s2.release_year)
              OR s1.credits && s2.credits
              OR s1.locations && s2.locations
          )
        LIMIT $2
        "#,
    )
    .bind(current_song_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------
// MATCH & TURN HELPERS
// ---------------------------------------------------------

/// Creates a new match in the 'waiting' state for player 1.
pub async fn create_match(pool: &PgPool, player1_id: Uuid) -> Result<MatchSession, sqlx::Error> {
    sqlx::query_as::<_, MatchSession>(
        r#"
        INSERT INTO matches (player1_id, status)
        VALUES ($1, 'waiting')
        RETURNING id, player1_id, player2_id, winner_id, status, started_at, finished_at
        "#,
    )
    .bind(player1_id)
    .fetch_one(pool)
    .await
}

/// Player 2 joins a match, advancing its status to 'active'.
pub async fn join_match(
    pool: &PgPool,
    match_id: Uuid,
    player2_id: Uuid,
) -> Result<MatchSession, sqlx::Error> {
    sqlx::query_as::<_, MatchSession>(
        r#"
        UPDATE matches
        SET player2_id = $2, status = 'active'
        WHERE id = $1
        RETURNING id, player1_id, player2_id, winner_id, status, started_at, finished_at
        "#,
    )
    .bind(match_id)
    .bind(player2_id)
    .fetch_one(pool)
    .await
}

/// Checks if a song has already been played in this match.
pub async fn is_song_already_played(
    pool: &PgPool,
    match_id: Uuid,
    song_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM match_moves
            WHERE match_id = $1 AND song_id = $2
        )
        "#,
    )
    .bind(match_id)
    .bind(song_id)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Gets the song played on the latest turn of this match.
pub async fn get_last_played_song(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Option<Song>, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r#"
        SELECT s.id, s.mbid, s.title, s.release_year, s.duration_ms, 
               s.artists, s.albums, s.credits, s.locations, s.created_at
        FROM match_moves m
        JOIN songs s ON s.id = m.song_id
        WHERE m.match_id = $1
        ORDER BY m.turn_number DESC
        LIMIT 1
        "#,
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
}

/// Records a move in the match ledger.
pub async fn record_move(
    pool: &PgPool,
    match_id: Uuid,
    player_id: Uuid,
    turn_number: i32,
    song_id: Uuid,
    link_type: Option<&str>,
    link_value: Option<&str>,
) -> Result<MatchMove, sqlx::Error> {
    sqlx::query_as::<_, MatchMove>(
        r#"
        INSERT INTO match_moves (match_id, player_id, turn_number, song_id, link_type, link_value)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, match_id, player_id, turn_number, song_id, link_type, link_value, created_at
        "#,
    )
    .bind(match_id)
    .bind(player_id)
    .bind(turn_number)
    .bind(song_id)
    .bind(link_type)
    .bind(link_value)
    .fetch_one(pool)
    .await
}

/// Concludes a match with a winner.
pub async fn finish_match(
    pool: &PgPool,
    match_id: Uuid,
    winner_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE matches
        SET winner_id = $2, status = 'finished', finished_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(match_id)
    .bind(winner_id)
    .execute(pool)
    .await?;

    Ok(())
}
