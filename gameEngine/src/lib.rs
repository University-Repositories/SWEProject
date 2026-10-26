use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

pub const MAX_USES: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordingId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreditId(pub Uuid);

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

#[derive(Debug, Clone)]
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
