use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

pub const MAX_USES: u8 = 3;
pub const STARTING_LIVES: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordingId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreditId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    pub id: RecordingId,
    pub credits: HashSet<CreditId>,
}

impl Song {
    pub fn new(id: RecordingId, credits: impl IntoIterator<Item = CreditId>) -> Self {
        Song {
            id,
            credits: credits.into_iter().collect(),
        }
    }
    pub fn has_credit(&self, credit: CreditId) -> bool {
        self.credits.contains(&credit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveAttempt {
    pub song: Song,
    pub selected_credit: CreditId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedLink {
    pub song: RecordingId,
    pub credit: CreditId,
    pub uses_after: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveRejection {
    SongAlreadyPlayed,
    CreditNotOnPreviousSong,
    CreditNotOnSubmittedSong,
    CreditExhausted,
}

impl fmt::Display for MoveRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message: &str = match self {
            MoveRejection::SongAlreadyPlayed => "that song has already been played",
            MoveRejection::CreditNotOnPreviousSong => "that credit is not on the previous song",
            MoveRejection::CreditNotOnSubmittedSong => "that credit is not on the submitted song",
            MoveRejection::CreditExhausted => "that credit has no uses left",
        };
        f.write_str(message)
    }
}

impl std::error::Error for MoveRejection {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    order: Vec<RecordingId>,
    played: HashSet<RecordingId>,
    uses: HashMap<CreditId, u8>,
    current: Song,
}

impl Chain {
    pub fn new(opening: Song) -> Self {
        Chain {
            order: vec![opening.id],
            played: HashSet::from([opening.id]),
            uses: HashMap::new(),
            current: opening,
        }
    }
    pub fn validate(&self, attempt: &MoveAttempt) -> Result<(), MoveRejection> {
        let credit = attempt.selected_credit;

        if self.played.contains(&attempt.song.id) {
            return Err(MoveRejection::SongAlreadyPlayed);
        }
        if !self.current.has_credit(credit) {
            return Err(MoveRejection::CreditNotOnPreviousSong);
        }
        if !attempt.song.has_credit(credit) {
            return Err(MoveRejection::CreditNotOnSubmittedSong);
        }
        if self.uses_of(credit) >= MAX_USES {
            return Err(MoveRejection::CreditExhausted);
        }
        Ok(())
    }
    pub fn apply(&mut self, attempt: MoveAttempt) -> Result<AcceptedLink, MoveRejection> {
        self.validate(&attempt)?;

        let credit = attempt.selected_credit;
        let uses = self.uses.entry(credit).or_insert(0);
        *uses += 1;
        let uses_after = *uses;

        let id = attempt.song.id;
        self.order.push(id);
        self.played.insert(id);
        self.current = attempt.song;

        Ok(AcceptedLink {
            song: id,
            credit,
            uses_after,
        })
    }
    pub fn remaining_uses(&self, credit: CreditId) -> u8 {
        MAX_USES.saturating_sub(self.uses_of(credit))
    }
    pub fn songs(&self) -> &[RecordingId] {
        &self.order
    }
    pub fn current(&self) -> &Song {
        &self.current
    }
    fn uses_of(&self, credit: CreditId) -> u8 {
        self.uses.get(&credit).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    PlayerOne,
    PlayerTwo,
}

impl Turn {
    pub fn other(self) -> Self {
        match self {
            Turn::PlayerOne => Turn::PlayerTwo,
            Turn::PlayerTwo => Turn::PlayerOne,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    OutOfLives,
    Forfeit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchOutcome {
    pub winner: PlayerId,
    pub loser: PlayerId,
    pub reason: EndReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitRejection {
    MatchOver,
    NotYourTurn,
    StaleTurn,
    Chain(MoveRejection),
}

impl fmt::Display for SubmitRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubmitRejection::MatchOver => f.write_str("the match is already over"),
            SubmitRejection::NotYourTurn => f.write_str("it is not your turn"),
            SubmitRejection::StaleTurn => f.write_str("that turn has already ended"),
            SubmitRejection::Chain(reason) => reason.fmt(f),
        }
    }
}

impl std::error::Error for SubmitRejection {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    pub chain: Chain,
    pub player_one: PlayerId,
    pub player_two: PlayerId,
    pub lives_one: u8,
    pub lives_two: u8,
    pub active_turn: Turn,
    pub turn_number: u32,
    pub turn_length_ms: u64,
    pub turn_deadline_ms: u64,
    pub outcome: Option<MatchOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    chain: Chain,
    player_one: PlayerId,
    player_two: PlayerId,
    lives_one: u8,
    lives_two: u8,
    active_turn: Turn,
    turn_number: u32,
    turn_length_ms: u64,
    turn_deadline_ms: u64,
    outcome: Option<MatchOutcome>,
}

impl Match {
    pub fn new(
        opening: Song,
        player_one: PlayerId,
        player_two: PlayerId,
        turn_length_ms: u64,
        started_at_ms: u64,
    ) -> Self {
        Match {
            chain: Chain::new(opening),
            player_one,
            player_two,
            lives_one: STARTING_LIVES,
            lives_two: STARTING_LIVES,
            active_turn: Turn::PlayerOne,
            turn_number: 1,
            turn_length_ms,
            turn_deadline_ms: started_at_ms.saturating_add(turn_length_ms),
            outcome: None,
        }
    }

    pub fn from_snapshot(snapshot: MatchSnapshot) -> Self {
        Match {
            chain: snapshot.chain,
            player_one: snapshot.player_one,
            player_two: snapshot.player_two,
            lives_one: snapshot.lives_one,
            lives_two: snapshot.lives_two,
            active_turn: snapshot.active_turn,
            turn_number: snapshot.turn_number,
            turn_length_ms: snapshot.turn_length_ms,
            turn_deadline_ms: snapshot.turn_deadline_ms,
            outcome: snapshot.outcome,
        }
    }

    pub fn snapshot(&self) -> MatchSnapshot {
        MatchSnapshot {
            chain: self.chain.clone(),
            player_one: self.player_one,
            player_two: self.player_two,
            lives_one: self.lives_one,
            lives_two: self.lives_two,
            active_turn: self.active_turn,
            turn_number: self.turn_number,
            turn_length_ms: self.turn_length_ms,
            turn_deadline_ms: self.turn_deadline_ms,
            outcome: self.outcome,
        }
    }

    pub fn settle(&mut self, now_ms: u64) {
        while self.outcome.is_none() && now_ms >= self.turn_deadline_ms {
            let expired_at = self.turn_deadline_ms;
            self.penalize_active_turn(expired_at);
        }
    }

    pub fn submit(
        &mut self,
        player: PlayerId,
        turn_number: u32,
        attempt: MoveAttempt,
        now_ms: u64,
    ) -> Result<AcceptedLink, SubmitRejection> {
        self.settle(now_ms);

        if self.outcome.is_some() {
            return Err(SubmitRejection::MatchOver);
        }
        if turn_number != self.turn_number {
            return Err(SubmitRejection::StaleTurn);
        }
        if player != self.active_player() {
            return Err(SubmitRejection::NotYourTurn);
        }

        let link = self.chain.apply(attempt).map_err(SubmitRejection::Chain)?;

        self.advance_turn(now_ms);
        Ok(link)
    }

    pub fn forfeit(&mut self, player: PlayerId) -> Result<MatchOutcome, SubmitRejection> {
        if self.outcome.is_some() {
            return Err(SubmitRejection::MatchOver);
        }
        let outcome = if player == self.player_one {
            MatchOutcome {
                winner: self.player_two,
                loser: self.player_one,
                reason: EndReason::Forfeit,
            }
        } else if player == self.player_two {
            MatchOutcome {
                winner: self.player_one,
                loser: self.player_two,
                reason: EndReason::Forfeit,
            }
        } else {
            return Err(SubmitRejection::NotYourTurn);
        };
        self.outcome = Some(outcome);
        Ok(outcome)
    }

    pub fn chain(&self) -> &Chain {
        &self.chain
    }

    pub fn active_turn(&self) -> Turn {
        self.active_turn
    }

    pub fn active_player(&self) -> PlayerId {
        match self.active_turn {
            Turn::PlayerOne => self.player_one,
            Turn::PlayerTwo => self.player_two,
        }
    }

    pub fn turn_number(&self) -> u32 {
        self.turn_number
    }

    pub fn turn_deadline_ms(&self) -> u64 {
        self.turn_deadline_ms
    }

    pub fn lives(&self, player: PlayerId) -> Option<u8> {
        if player == self.player_one {
            Some(self.lives_one)
        } else if player == self.player_two {
            Some(self.lives_two)
        } else {
            None
        }
    }

    pub fn outcome(&self) -> Option<MatchOutcome> {
        self.outcome
    }

    pub fn winner(&self) -> Option<PlayerId> {
        self.outcome.map(|o| o.winner)
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    fn penalize_active_turn(&mut self, at_ms: u64) {
        match self.active_turn {
            Turn::PlayerOne => {
                self.lives_one = self.lives_one.saturating_sub(1);
                if self.lives_one == 0 {
                    self.outcome = Some(MatchOutcome {
                        winner: self.player_two,
                        loser: self.player_one,
                        reason: EndReason::OutOfLives,
                    });
                    return;
                }
            }
            Turn::PlayerTwo => {
                self.lives_two = self.lives_two.saturating_sub(1);
                if self.lives_two == 0 {
                    self.outcome = Some(MatchOutcome {
                        winner: self.player_one,
                        loser: self.player_two,
                        reason: EndReason::OutOfLives,
                    });
                    return;
                }
            }
        }
        self.advance_turn(at_ms);
    }

    fn advance_turn(&mut self, now_ms: u64) {
        self.active_turn = self.active_turn.other();
        self.turn_number += 1;
        self.turn_deadline_ms = now_ms.saturating_add(self.turn_length_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn rec(n: u128) -> RecordingId {
        RecordingId(Uuid::from_u128(n))
    }

    fn cred(n: u128) -> CreditId {
        CreditId(Uuid::from_u128(n))
    }

    fn song(id: u128, credits: &[u128]) -> Song {
        Song::new(rec(id), credits.iter().copied().map(cred))
    }

    #[test]
    fn alternates_turns_and_resets_deadline_on_accepted_move() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100, 200]), p1, p2, 15_000, 1_000);

        assert_eq!(m.active_player(), p1);
        assert_eq!(m.turn_number(), 1);
        assert_eq!(m.turn_deadline_ms(), 16_000);
        assert_eq!(m.lives(p1), Some(3));
        assert_eq!(m.lives(p2), Some(3));

        let attempt = MoveAttempt {
            song: song(11, &[100, 300]),
            selected_credit: cred(100),
        };
        let link = m.submit(p1, 1, attempt, 5_000).unwrap();
        assert_eq!(link.song, rec(11));
        assert_eq!(link.uses_after, 1);
        assert_eq!(m.active_player(), p2);
        assert_eq!(m.turn_number(), 2);
        assert_eq!(m.turn_deadline_ms(), 20_000);
    }

    #[test]
    fn wrong_guess_does_not_cost_life_or_pass_turn() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 15_000, 1_000);

        let bad = MoveAttempt {
            song: song(11, &[200]),
            selected_credit: cred(200),
        };
        assert_eq!(
            m.submit(p1, 1, bad, 4_000),
            Err(SubmitRejection::Chain(
                MoveRejection::CreditNotOnPreviousSong
            ))
        );
        assert_eq!(m.active_player(), p1);
        assert_eq!(m.turn_number(), 1);
        assert_eq!(m.lives(p1), Some(3));
        assert_eq!(m.turn_deadline_ms(), 16_000);
    }

    #[test]
    fn rejects_out_of_turn_and_stale_turn_submissions() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 15_000, 1_000);

        let attempt = MoveAttempt {
            song: song(11, &[100]),
            selected_credit: cred(100),
        };
        assert_eq!(
            m.submit(p2, 1, attempt.clone(), 2_000),
            Err(SubmitRejection::NotYourTurn)
        );
        assert_eq!(
            m.submit(p1, 2, attempt, 2_000),
            Err(SubmitRejection::StaleTurn)
        );
    }

    #[test]
    fn timeout_costs_one_life_and_passes_turn_until_zero_lives() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 10_000, 0);

        // Turn 1 (p1) expires at 10_000
        m.settle(10_000);
        assert_eq!(m.lives(p1), Some(2));
        assert_eq!(m.lives(p2), Some(3));
        assert_eq!(m.active_player(), p2);
        assert_eq!(m.turn_number(), 2);
        assert_eq!(m.turn_deadline_ms(), 20_000);
        assert!(!m.is_over());

        // Turn 2 (p2) expires at 20_000 and Turn 3 (p1) expires at 30_000
        m.settle(32_000);
        assert_eq!(m.lives(p2), Some(2));
        assert_eq!(m.lives(p1), Some(1));
        assert_eq!(m.active_player(), p2);
        assert_eq!(m.turn_number(), 4);
        assert_eq!(m.turn_deadline_ms(), 40_000);

        // Turn 4 (p2) expires at 40_000, Turn 5 (p1) expires at 50_000 -> p1 hits 0 lives
        m.settle(99_000);
        assert_eq!(m.lives(p2), Some(1));
        assert_eq!(m.lives(p1), Some(0));
        assert!(m.is_over());
        assert_eq!(
            m.outcome(),
            Some(MatchOutcome {
                winner: p2,
                loser: p1,
                reason: EndReason::OutOfLives,
            })
        );
    }

    #[test]
    fn late_submission_triggers_timeout_before_applying() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 10_000, 0);

        let attempt = MoveAttempt {
            song: song(11, &[100]),
            selected_credit: cred(100),
        };
        assert_eq!(
            m.submit(p1, 1, attempt, 10_000),
            Err(SubmitRejection::StaleTurn)
        );
        assert_eq!(m.lives(p1), Some(2));
        assert_eq!(m.active_player(), p2);
        assert_eq!(m.chain().songs(), &[rec(10)]);
    }

    #[test]
    fn forfeit_and_match_over_block_further_actions() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 10_000, 0);

        assert_eq!(
            m.forfeit(p1),
            Ok(MatchOutcome {
                winner: p2,
                loser: p1,
                reason: EndReason::Forfeit,
            })
        );
        assert_eq!(m.winner(), Some(p2));
        assert_eq!(m.forfeit(p2), Err(SubmitRejection::MatchOver));

        let attempt = MoveAttempt {
            song: song(11, &[100]),
            selected_credit: cred(100),
        };
        assert_eq!(
            m.submit(p2, 1, attempt, 1_000),
            Err(SubmitRejection::MatchOver)
        );
    }

    #[test]
    fn snapshot_round_trips_match_state() {
        let p1 = player(1);
        let p2 = player(2);
        let mut m = Match::new(song(10, &[100]), p1, p2, 10_000, 500);
        m.settle(10_500);

        let restored = Match::from_snapshot(m.snapshot());
        assert_eq!(restored, m);
    }
}
