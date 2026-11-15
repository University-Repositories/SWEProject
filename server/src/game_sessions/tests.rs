use std::sync::Mutex;

use super::*;
use crate::api_types::{CreditKind, CreditRole};

#[derive(Default)]
struct MemoryStore {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    games: HashMap<Uuid, GameRecord>,
    moves: HashMap<Uuid, Vec<MoveRecord>>,
    playable: Vec<Uuid>,
    conflict_on_add: bool,
}

impl MemoryStore {
    fn with_playable(ids: &[Uuid]) -> Self {
        let store = MemoryStore::default();
        store.inner.lock().unwrap().playable = ids.to_vec();
        store
    }

    fn saved_moves(&self, game_id: Uuid) -> Vec<MoveRecord> {
        let inner = self.inner.lock().unwrap();
        inner.moves.get(&game_id).cloned().unwrap_or_default()
    }

    fn saved_game(&self, game_id: Uuid) -> GameRecord {
        self.inner.lock().unwrap().games[&game_id].clone()
    }
}

impl GameStore for MemoryStore {
    async fn create_solo(
        &self,
        user_id: Uuid,
        opening_recording_id: Uuid,
    ) -> Result<GameRecord, RepoError> {
        let game = GameRecord {
            id: Uuid::new_v4(),
            mode: GameMode::Solo,
            status: GameStatus::Active,
            player_ids: vec![user_id],
            opening_recording_id,
            accepted_links: None,
            started_at: Utc::now(),
            ended_at: None,
        };
        let mut inner = self.inner.lock().unwrap();
        inner.games.insert(game.id, game.clone());
        Ok(game)
    }

    async fn get(&self, game_id: Uuid) -> Result<Option<GameRecord>, RepoError> {
        Ok(self.inner.lock().unwrap().games.get(&game_id).cloned())
    }

    async fn list_moves(&self, game_id: Uuid) -> Result<Vec<MoveRecord>, RepoError> {
        Ok(self.saved_moves(game_id))
    }

    async fn add_move(&self, new_move: NewMove) -> Result<(), RepoError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.conflict_on_add {
            return Err(RepoError::Conflict("moves_game_id_turn_number_key".into()));
        }
        let moves = inner.moves.entry(new_move.game_id).or_default();
        if moves.iter().any(|m| m.turn_number == new_move.turn_number) {
            return Err(RepoError::Conflict("moves_game_id_turn_number_key".into()));
        }
        moves.push(MoveRecord {
            turn_number: new_move.turn_number,
            user_id: new_move.user_id,
            recording_id: new_move.recording_id,
            credit_id: new_move.credit_id,
        });
        Ok(())
    }

    async fn end(
        &self,
        game_id: Uuid,
        accepted_links: i32,
        ended_at: DateTime<Utc>,
    ) -> Result<bool, RepoError> {
        let mut inner = self.inner.lock().unwrap();
        match inner.games.get_mut(&game_id) {
            Some(game) if game.status == GameStatus::Active => {
                game.status = GameStatus::Ended;
                game.accepted_links = Some(accepted_links);
                game.ended_at = Some(ended_at);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn random_playable_recording(&self) -> Result<Option<Uuid>, RepoError> {
        Ok(self.inner.lock().unwrap().playable.first().copied())
    }
}

struct MemoryCatalog {
    songs: HashMap<Uuid, SongInfo>,
}

impl SongCatalog for MemoryCatalog {
    async fn song(&self, recording_id: Uuid) -> Result<Option<SongInfo>, RepoError> {
        Ok(self.songs.get(&recording_id).cloned())
    }
}

// Test catalog. Songs 1 to 5 all share credit 10, so 1 -> 2 -> 3 -> 4 uses it
// three times and 5 would be its fourth use. Song 6 shares nothing with the others.

fn song_id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn credit_id(n: u128) -> Uuid {
    Uuid::from_u128(1_000 + n)
}

fn credit(n: u128) -> Credit {
    Credit {
        credit_id: credit_id(n),
        name: format!("Credit {n}"),
        kind: CreditKind::Person,
        role: CreditRole::Producer,
    }
}

fn song(n: u128, credits: &[u128]) -> SongInfo {
    SongInfo {
        summary: RecordingSummary {
            id: song_id(n),
            title: format!("Song {n}"),
            artist_name: format!("Artist {n}"),
            year: Some(2000),
            release_id: None,
        },
        credits: credits.iter().map(|&c| credit(c)).collect(),
    }
}

fn catalog() -> MemoryCatalog {
    let songs = [
        song(1, &[10, 11]),
        song(2, &[10, 12]),
        song(3, &[10, 13]),
        song(4, &[10, 14]),
        song(5, &[10, 15]),
        song(6, &[20]),
        song(7, &[]),
    ];
    MemoryCatalog {
        songs: songs.into_iter().map(|s| (s.summary.id, s)).collect(),
    }
}

fn sessions(store: MemoryStore) -> GameSessions<MemoryStore, MemoryCatalog> {
    GameSessions::new(store, catalog())
}

fn player() -> Uuid {
    Uuid::from_u128(9_001)
}

fn other_player() -> Uuid {
    Uuid::from_u128(9_002)
}

async fn started(service: &GameSessions<MemoryStore, MemoryCatalog>) -> GameView {
    service
        .start_solo(player(), Some(song_id(1)))
        .await
        .unwrap()
}

#[tokio::test]
async fn starts_with_the_given_opening_and_fresh_counts() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    assert_eq!(game.opening.id, song_id(1));
    assert_eq!(game.status, GameStatus::Active);
    assert!(game.links.is_empty());
    assert!(game.credit_uses.is_empty());
    assert_eq!(game.accepted_links, 0);
    assert!(game.ended_at.is_none());
}

#[tokio::test]
async fn picks_a_random_playable_opening_when_none_is_given() {
    let service = sessions(MemoryStore::with_playable(&[song_id(3)]));
    let game = service.start_solo(player(), None).await.unwrap();
    assert_eq!(game.opening.id, song_id(3));
}

#[tokio::test]
async fn rejects_openings_that_cannot_be_played() {
    let service = sessions(MemoryStore::default());
    let unknown = service.start_solo(player(), Some(song_id(99))).await;
    assert!(matches!(unknown, Err(GameError::UnknownRecording)));
    let no_credits = service.start_solo(player(), Some(song_id(7))).await;
    assert!(matches!(no_credits, Err(GameError::UnknownRecording)));
    let empty_catalog = service.start_solo(player(), None).await;
    assert!(matches!(empty_catalog, Err(GameError::NoPlayableSongs)));
}

#[tokio::test]
async fn accepted_moves_are_saved_with_the_next_turn_number() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;

    let first = service
        .submit_move(player(), game.id, song_id(2), credit_id(10))
        .await
        .unwrap();
    assert!(first.accepted());
    let second = service
        .submit_move(player(), game.id, song_id(3), credit_id(10))
        .await
        .unwrap();
    assert!(second.accepted());

    let turns: Vec<i32> = service
        .store
        .saved_moves(game.id)
        .iter()
        .map(|m| m.turn_number)
        .collect();
    assert_eq!(turns, vec![1, 2]);

    let view = second.game;
    let order: Vec<(u32, Uuid, u8)> = view
        .links
        .iter()
        .map(|link| (link.position, link.recording.id, link.uses_after))
        .collect();
    assert_eq!(order, vec![(1, song_id(2), 1), (2, song_id(3), 2)]);
    assert_eq!(view.accepted_links, 2);
    assert_eq!(view.credit_uses.len(), 1);
    assert_eq!(view.credit_uses[0].credit.credit_id, credit_id(10));
    assert_eq!(view.credit_uses[0].used, 2);
    assert_eq!(view.credit_uses[0].remaining, 1);
}

#[tokio::test]
async fn a_rejected_move_leaves_saved_moves_and_counts_unchanged() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    service
        .submit_move(player(), game.id, song_id(2), credit_id(10))
        .await
        .unwrap();
    let before = service.get_state(player(), game.id).await.unwrap();

    let outcome = service
        .submit_move(player(), game.id, song_id(6), credit_id(20))
        .await
        .unwrap();
    assert_eq!(
        outcome.rejection,
        Some(MoveRejection::CreditNotOnPreviousSong)
    );

    assert_eq!(service.store.saved_moves(game.id).len(), 1);
    let after = service.get_state(player(), game.id).await.unwrap();
    assert_eq!(after.links.len(), before.links.len());
    assert_eq!(after.credit_uses[0].used, before.credit_uses[0].used);
    assert_eq!(after.credit_uses[0].remaining, 2);
    assert_eq!(outcome.game.links.len(), 1);
}

#[tokio::test]
async fn the_fourth_use_of_a_credit_is_rejected() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    for next in [2, 3, 4] {
        let outcome = service
            .submit_move(player(), game.id, song_id(next), credit_id(10))
            .await
            .unwrap();
        assert!(outcome.accepted());
    }

    let fourth = service
        .submit_move(player(), game.id, song_id(5), credit_id(10))
        .await
        .unwrap();
    assert_eq!(fourth.rejection, Some(MoveRejection::CreditExhausted));
    assert_eq!(service.store.saved_moves(game.id).len(), 3);
    assert_eq!(fourth.game.credit_uses[0].remaining, 0);
}

#[tokio::test]
async fn saved_moves_are_replayed_so_played_songs_stay_played() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    service
        .submit_move(player(), game.id, song_id(2), credit_id(10))
        .await
        .unwrap();

    // Song 2 is the current song, so credit 12 is on both songs, but song 2 is played.
    let again = service
        .submit_move(player(), game.id, song_id(2), credit_id(12))
        .await
        .unwrap();
    assert_eq!(again.rejection, Some(MoveRejection::SongAlreadyPlayed));
    let opening = service
        .submit_move(player(), game.id, song_id(1), credit_id(10))
        .await
        .unwrap();
    assert_eq!(opening.rejection, Some(MoveRejection::SongAlreadyPlayed));
}

#[tokio::test]
async fn moves_after_end_are_rejected_and_the_result_is_saved_once() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    for next in [2, 3] {
        service
            .submit_move(player(), game.id, song_id(next), credit_id(10))
            .await
            .unwrap();
    }

    let ended = service.end(player(), game.id).await.unwrap();
    assert_eq!(ended.status, GameStatus::Ended);
    assert_eq!(ended.accepted_links, 2);
    assert!(ended.ended_at.is_some());
    let saved = service.store.saved_game(game.id);

    let late_move = service
        .submit_move(player(), game.id, song_id(4), credit_id(10))
        .await;
    assert!(matches!(late_move, Err(GameError::GameEnded)));
    let second_end = service.end(player(), game.id).await;
    assert!(matches!(second_end, Err(GameError::GameEnded)));

    assert_eq!(service.store.saved_game(game.id), saved);
    assert_eq!(service.store.saved_moves(game.id).len(), 2);
    let state = service.get_state(player(), game.id).await.unwrap();
    assert_eq!(state.accepted_links, 2);
    assert_eq!(state.ended_at, saved.ended_at);
}

#[tokio::test]
async fn other_players_cannot_see_or_change_a_game() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;

    let read = service.get_state(other_player(), game.id).await;
    assert!(matches!(read, Err(GameError::NotFound)));
    let moved = service
        .submit_move(other_player(), game.id, song_id(2), credit_id(10))
        .await;
    assert!(matches!(moved, Err(GameError::NotFound)));
    let ended = service.end(other_player(), game.id).await;
    assert!(matches!(ended, Err(GameError::NotFound)));
    let missing = service.get_state(player(), Uuid::new_v4()).await;
    assert!(matches!(missing, Err(GameError::NotFound)));

    assert!(service.store.saved_moves(game.id).is_empty());
    assert_eq!(service.store.saved_game(game.id).status, GameStatus::Active);
}

#[tokio::test]
async fn an_unknown_song_is_rejected_without_saving() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    let outcome = service
        .submit_move(player(), game.id, song_id(99), credit_id(10))
        .await;
    assert!(matches!(outcome, Err(GameError::UnknownRecording)));
    assert!(service.store.saved_moves(game.id).is_empty());
}

#[tokio::test]
async fn a_turn_saved_by_another_request_returns_turn_taken() {
    let service = sessions(MemoryStore::default());
    let game = started(&service).await;
    service.store.inner.lock().unwrap().conflict_on_add = true;
    let outcome = service
        .submit_move(player(), game.id, song_id(2), credit_id(10))
        .await;
    assert!(matches!(outcome, Err(GameError::TurnTaken)));
}
