use std::future::Future;

use chrono::{DateTime, Utc};
use data_management::{CatalogRepo, RecordingCredit, RepoError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api_types::{Credit, CreditKind, CreditRole, RecordingSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Solo,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Active,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub id: Uuid,
    pub mode: GameMode,
    pub status: GameStatus,
    pub player_ids: Vec<Uuid>,
    pub opening_recording_id: Uuid,
    pub accepted_links: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveRecord {
    pub turn_number: i32,
    pub user_id: Uuid,
    pub recording_id: Uuid,
    pub credit_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMove {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub turn_number: i32,
    pub recording_id: Uuid,
    pub credit_id: Uuid,
}

pub trait GameStore: Send + Sync {
    fn create_solo(
        &self,
        user_id: Uuid,
        opening_recording_id: Uuid,
    ) -> impl Future<Output = Result<GameRecord, RepoError>> + Send;

    fn get(
        &self,
        game_id: Uuid,
    ) -> impl Future<Output = Result<Option<GameRecord>, RepoError>> + Send;

    fn list_moves(
        &self,
        game_id: Uuid,
    ) -> impl Future<Output = Result<Vec<MoveRecord>, RepoError>> + Send;

    fn add_move(&self, new_move: NewMove) -> impl Future<Output = Result<(), RepoError>> + Send;

    fn end(
        &self,
        game_id: Uuid,
        accepted_links: i32,
        ended_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<bool, RepoError>> + Send;

    fn random_playable_recording(
        &self,
    ) -> impl Future<Output = Result<Option<Uuid>, RepoError>> + Send;
}

#[derive(Debug, Clone)]
pub struct SongInfo {
    pub summary: RecordingSummary,
    pub credits: Vec<Credit>,
}

pub trait SongCatalog: Send + Sync {
    fn song(
        &self,
        recording_id: Uuid,
    ) -> impl Future<Output = Result<Option<SongInfo>, RepoError>> + Send;
}

#[derive(Debug, Clone)]
pub struct PgSongCatalog {
    repo: CatalogRepo,
}

impl PgSongCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: CatalogRepo::new(pool),
        }
    }
}

impl SongCatalog for PgSongCatalog {
    async fn song(&self, recording_id: Uuid) -> Result<Option<SongInfo>, RepoError> {
        let row = self.repo.get_recording(recording_id).await?;
        Ok(row.map(|row| SongInfo {
            summary: RecordingSummary {
                id: row.id,
                title: row.title,
                artist_name: row.artist_name,
                year: row.year,
                release_id: row.release_id,
            },
            credits: row.credits.into_iter().map(credit_from_row).collect(),
        }))
    }
}

fn credit_from_row(row: RecordingCredit) -> Credit {
    let kind = match row.kind.to_lowercase().as_str() {
        "label" => CreditKind::Label,
        "album" | "release" => CreditKind::Album,
        _ => CreditKind::Person,
    };
    let role = match row.role.to_lowercase().as_str() {
        "feature" | "featured_artist" => CreditRole::Feature,
        "producer" => CreditRole::Producer,
        "writer" | "songwriter" => CreditRole::Writer,
        "label" => CreditRole::Label,
        "album" => CreditRole::Album,
        _ => CreditRole::Artist,
    };
    Credit {
        credit_id: row.credit_id,
        name: row.name,
        kind,
        role,
    }
}
