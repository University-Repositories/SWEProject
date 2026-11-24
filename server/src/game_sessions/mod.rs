mod store;
#[cfg(test)]
mod tests;

pub use store::{
    GameMode, GameRecord, GameStatus, GameStore, MoveRecord, NewMove, PgSongCatalog, SongCatalog,
    SongInfo,
};

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use data_management::RepoError;
use game_engine::{Chain, CreditId, MAX_USES, MoveAttempt, MoveRejection, RecordingId, Song};
use uuid::Uuid;

use crate::api_types::{Credit, RecordingSummary};

#[derive(Debug)]
pub enum GameError {
    NotFound,
    GameEnded,
    NoPlayableSongs,
    UnknownRecording,
    TurnTaken,
    /// The saved moves no longer replay through the engine.
    BrokenHistory,
    Repo(RepoError),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::NotFound => f.write_str("game not found"),
            GameError::GameEnded => f.write_str("the game has ended"),
            GameError::UnknownRecording => f.write_str("that recording is not in the catalog"),
            GameError::NoPlayableSongs => f.write_str("the catalog has no playable songs"),
            GameError::TurnTaken => f.write_str("another move was saved for this turn first"),
            GameError::BrokenHistory => f.write_str("the saved moves no longer follow the rules"),
            GameError::Repo(err) => write!(f, "storage error: {err}"),
        }
    }
}

impl std::error::Error for GameError {}

impl From<RepoError> for GameError {
    fn from(err: RepoError) -> Self {
        GameError::Repo(err)
    }
}

#[derive(Debug, Clone)]
pub struct GameView {
    pub id: Uuid,
    pub status: GameStatus,
    pub opening: RecordingSummary,
    pub links: Vec<LinkView>,
    pub credit_uses: Vec<CreditUseView>,
    pub accepted_links: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct LinkView {
    pub position: u32,
    pub recording: RecordingSummary,
    pub credit: Credit,
    pub uses_after: u8,
}

#[derive(Debug, Clone)]
pub struct CreditUseView {
    pub credit: Credit,
    pub used: u8,
    pub remaining: u8,
}

#[derive(Debug, Clone)]
pub struct MoveOutcome {
    pub rejection: Option<MoveRejection>,
    pub game: GameView,
}

impl MoveOutcome {
    pub fn accepted(&self) -> bool {
        self.rejection.is_none()
    }
}

pub struct GameSessions<S, C> {
    store: S,
    catalog: C,
}

type SongCache = HashMap<Uuid, Option<SongInfo>>;
impl<S: GameStore, C: SongCatalog> GameSessions<S, C> {
    pub fn new(store: S, catalog: C) -> Self {
        Self { store, catalog }
    }
    pub async fn start_solo(
        &self,
        user_id: Uuid,
        opening: Option<Uuid>,
    ) -> Result<GameView, GameError> {
        let opening_id = match opening {
            Some(id) => id,
            None => self
                .store
                .random_playable_recording()
                .await?
                .ok_or(GameError::NoPlayableSongs)?,
        };
        let mut songs = SongCache::new();
        match self.song(&mut songs, opening_id).await? {
            Some(song) if !song.credits.is_empty() => {}
            _ => return Err(GameError::UnknownRecording),
        }
        let game = self.store.create_solo(user_id, opening_id).await?;
        self.view(&game, &[], &mut songs).await
    }

    pub async fn get_state(&self, user_id: Uuid, game_id: Uuid) -> Result<GameView, GameError> {
        let game = self.load_game(user_id, game_id).await?;
        let moves = self.store.list_moves(game_id).await?;
        self.view(&game, &moves, &mut SongCache::new()).await
    }

    pub async fn submit_move(
        &self,
        user_id: Uuid,
        game_id: Uuid,
        recording_id: Uuid,
        credit_id: Uuid,
    ) -> Result<MoveOutcome, GameError> {
        let game = self.load_game(user_id, game_id).await?;
        if game.status == GameStatus::Ended {
            return Err(GameError::GameEnded);
        }
        let mut moves = self.store.list_moves(game_id).await?;
        let mut songs = SongCache::new();
        let next = self
            .song(&mut songs, recording_id)
            .await?
            .ok_or(GameError::UnknownRecording)?;

        let mut chain = self.replay(&game, &moves, &mut songs).await?;
        let attempt = MoveAttempt {
            song: engine_song(&next),
            selected_credit: CreditId(credit_id),
        };
        if let Err(rejection) = chain.apply(attempt) {
            let game = self.view(&game, &moves, &mut songs).await?;
            return Ok(MoveOutcome {
                rejection: Some(rejection),
                game,
            });
        }

        let turn_number = moves.last().map_or(1, |last| last.turn_number + 1);
        let saved = self
            .store
            .add_move(NewMove {
                game_id,
                user_id,
                turn_number,
                recording_id,
                credit_id,
            })
            .await;
        match saved {
            Ok(()) => {}
            Err(RepoError::Conflict(reason)) if reason == "game ended" => {
                return Err(GameError::GameEnded);
            }
            Err(RepoError::Conflict(_)) => return Err(GameError::TurnTaken),
            Err(err) => return Err(err.into()),
        }
        moves.push(MoveRecord {
            turn_number,
            user_id,
            recording_id,
            credit_id,
        });
        let game = self.view(&game, &moves, &mut songs).await?;
        Ok(MoveOutcome {
            rejection: None,
            game,
        })
    }

    pub async fn end(&self, user_id: Uuid, game_id: Uuid) -> Result<GameView, GameError> {
        let game = self.load_game(user_id, game_id).await?;
        if game.status == GameStatus::Ended {
            return Err(GameError::GameEnded);
        }
        let moves = self.store.list_moves(game_id).await?;
        let accepted_links = moves.len() as i32;
        // match the timestamp precision postgres keeps
        let now = Utc::now();
        let ended_at = DateTime::from_timestamp_micros(now.timestamp_micros()).unwrap_or(now);
        match self.store.end(game_id, accepted_links, ended_at).await {
            Ok(true) => {}
            Ok(false) => return Err(GameError::GameEnded),
            Err(RepoError::Conflict(_)) => return Err(GameError::TurnTaken),
            Err(error) => return Err(error.into()),
        }
        let ended = GameRecord {
            status: GameStatus::Ended,
            accepted_links: Some(accepted_links),
            ended_at: Some(ended_at),
            ..game
        };
        self.view(&ended, &moves, &mut SongCache::new()).await
    }

    async fn load_game(&self, user_id: Uuid, game_id: Uuid) -> Result<GameRecord, GameError> {
        match self.store.get(game_id).await? {
            Some(game) if game.mode == GameMode::Solo && game.player_ids.contains(&user_id) => {
                Ok(game)
            }
            _ => Err(GameError::NotFound),
        }
    }

    async fn song(
        &self,
        songs: &mut SongCache,
        recording_id: Uuid,
    ) -> Result<Option<SongInfo>, GameError> {
        if let Some(found) = songs.get(&recording_id) {
            return Ok(found.clone());
        }
        let found = self.catalog.song(recording_id).await?;
        songs.insert(recording_id, found.clone());
        Ok(found)
    }

    async fn saved_song(
        &self,
        songs: &mut SongCache,
        recording_id: Uuid,
    ) -> Result<SongInfo, GameError> {
        self.song(songs, recording_id)
            .await?
            .ok_or(GameError::BrokenHistory)
    }

    async fn replay(
        &self,
        game: &GameRecord,
        moves: &[MoveRecord],
        songs: &mut SongCache,
    ) -> Result<Chain, GameError> {
        let opening = self.saved_song(songs, game.opening_recording_id).await?;
        let mut chain = Chain::new(engine_song(&opening));
        for saved in moves {
            let song = self.saved_song(songs, saved.recording_id).await?;
            let attempt = MoveAttempt {
                song: engine_song(&song),
                selected_credit: CreditId(saved.credit_id),
            };
            chain.apply(attempt).map_err(|_| GameError::BrokenHistory)?;
        }
        Ok(chain)
    }

    async fn view(
        &self,
        game: &GameRecord,
        moves: &[MoveRecord],
        songs: &mut SongCache,
    ) -> Result<GameView, GameError> {
        let opening = self.saved_song(songs, game.opening_recording_id).await?;
        let mut previous = opening.clone();
        let mut links = Vec::with_capacity(moves.len());
        let mut used: HashMap<Uuid, u8> = HashMap::new();
        let mut first_used: Vec<Credit> = Vec::new();

        for (index, saved) in moves.iter().enumerate() {
            let song = self.saved_song(songs, saved.recording_id).await?;
            // The engine checked that the credit was on both songs when the move was
            // made, so look on the new song first and the previous song second.
            let credit = find_credit(&song, saved.credit_id)
                .or_else(|| find_credit(&previous, saved.credit_id))
                .ok_or(GameError::BrokenHistory)?;
            let count = used.entry(saved.credit_id).or_insert(0);
            *count += 1;
            if *count == 1 {
                first_used.push(credit.clone());
            }
            links.push(LinkView {
                position: index as u32 + 1,
                recording: song.summary.clone(),
                credit,
                uses_after: *count,
            });
            previous = song;
        }

        let credit_uses = first_used
            .into_iter()
            .map(|credit| {
                let used = used.get(&credit.credit_id).copied().unwrap_or(0);
                CreditUseView {
                    credit,
                    used,
                    remaining: MAX_USES.saturating_sub(used),
                }
            })
            .collect();
        let accepted_links = match game.status {
            GameStatus::Ended => game
                .accepted_links
                .map_or(links.len() as u32, |count| count as u32),
            GameStatus::Active => links.len() as u32,
        };

        Ok(GameView {
            id: game.id,
            status: game.status,
            opening: opening.summary,
            links,
            credit_uses,
            accepted_links,
            started_at: game.started_at,
            ended_at: game.ended_at,
        })
    }
}

fn engine_song(song: &SongInfo) -> Song {
    Song::new(
        RecordingId(song.summary.id),
        song.credits.iter().map(|credit| CreditId(credit.credit_id)),
    )
}

fn find_credit(song: &SongInfo, credit_id: Uuid) -> Option<Credit> {
    song.credits
        .iter()
        .find(|credit| credit.credit_id == credit_id)
        .cloned()
}
