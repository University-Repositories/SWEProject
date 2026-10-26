use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// A song stored in the database with array-based metadata.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Song {
    pub id: Uuid,
    pub mbid: String,
    pub title: String,
    pub release_year: Option<i32>,
    pub duration_ms: Option<i32>,
    pub artists: Vec<String>,
    pub albums: Vec<String>,
    pub credits: Vec<String>,
    pub locations: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// DTO for inserting a new song into the database.
#[derive(Debug, Clone, Default)]
pub struct NewSong {
    pub mbid: String,
    pub title: String,
    pub release_year: Option<i32>,
    pub duration_ms: Option<i32>,
    pub artists: Vec<String>,
    pub albums: Vec<String>,
    pub credits: Vec<String>,
    pub locations: Vec<String>,
}

/// Result of checking whether two songs are legally linked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkValidation {
    pub is_linked: bool,
    pub shared_artists: Vec<String>,
    pub shared_albums: Vec<String>,
    pub shared_year: Option<i32>,
    pub shared_credits: Vec<String>,
    pub shared_locations: Vec<String>,
}

/// Represents a PvP match session.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct MatchSession {
    pub id: Uuid,
    pub player1_id: Uuid,
    pub player2_id: Option<Uuid>,
    pub winner_id: Option<Uuid>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// Represents an individual turn in a match.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct MatchMove {
    pub id: Uuid,
    pub match_id: Uuid,
    pub player_id: Uuid,
    pub turn_number: i32,
    pub song_id: Uuid,
    pub link_type: Option<String>,
    pub link_value: Option<String>,
    pub created_at: DateTime<Utc>,
}
