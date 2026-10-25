use game_engine::{
    AcceptedLink, Chain, CreditId, MAX_USES, MoveAttempt, MoveRejection, RecordingId, Song,
};
use uuid::Uuid;

// Small helpers so each test reads as songs and credits, not raw UUIDs.
fn rec(n: u128) -> RecordingId {
    RecordingId(Uuid::from_u128(n))
}

fn credit(n: u128) -> CreditId {
    CreditId(Uuid::from_u128(1_000 + n))
}

fn song(id: u128, credits: &[u128]) -> Song {
    Song::new(rec(id), credits.iter().map(|&c| credit(c)))
}

fn attempt(next: Song, via: u128) -> MoveAttempt {
    MoveAttempt {
        song: next,
        selected_credit: credit(via),
    }
}

#[test]
fn accepted_move_links_through_a_shared_credit() {
    let mut chain = Chain::new(song(1, &[10, 11]));

    let link = chain.apply(attempt(song(2, &[11, 12]), 11)).unwrap();

    assert_eq!(
        link,
        AcceptedLink {
            song: rec(2),
            credit: credit(11),
            uses_after: 1,
        }
    );
    assert_eq!(chain.songs(), &[rec(1), rec(2)]);
    assert_eq!(chain.current().id, rec(2));
    assert_eq!(chain.remaining_uses(credit(11)), MAX_USES - 1);
}

#[test]
fn opening_song_does_not_consume_a_use() {
    let chain = Chain::new(song(1, &[10, 11]));

    assert_eq!(chain.remaining_uses(credit(10)), MAX_USES);
    assert_eq!(chain.remaining_uses(credit(11)), MAX_USES);
    assert_eq!(chain.songs(), &[rec(1)]);
}

#[test]
fn rejects_a_song_already_played() {
    let mut chain = Chain::new(song(1, &[10]));
    chain.apply(attempt(song(2, &[10]), 10)).unwrap();

    let result = chain.apply(attempt(song(1, &[10]), 10));

    assert_eq!(result, Err(MoveRejection::SongAlreadyPlayed));
}

#[test]
fn rejects_a_credit_not_on_the_previous_song() {
    let mut chain = Chain::new(song(1, &[10]));

    let result = chain.apply(attempt(song(2, &[10, 20]), 20));

    assert_eq!(result, Err(MoveRejection::CreditNotOnPreviousSong));
}

#[test]
fn rejects_a_credit_not_on_the_submitted_song() {
    let mut chain = Chain::new(song(1, &[10, 20]));

    let result = chain.apply(attempt(song(2, &[10]), 20));

    assert_eq!(result, Err(MoveRejection::CreditNotOnSubmittedSong));
}

#[test]
fn fourth_use_of_a_credit_is_rejected() {
    let mut chain = Chain::new(song(1, &[10]));
    for (next, expected_uses) in [(2, 1), (3, 2), (4, 3)] {
        let link = chain.apply(attempt(song(next, &[10]), 10)).unwrap();
        assert_eq!(link.uses_after, expected_uses);
    }
    assert_eq!(chain.remaining_uses(credit(10)), 0);

    let result = chain.apply(attempt(song(5, &[10]), 10));

    assert_eq!(result, Err(MoveRejection::CreditExhausted));
}

#[test]
fn rejected_attempt_leaves_the_chain_unchanged() {
    let mut chain = Chain::new(song(1, &[10]));
    chain.apply(attempt(song(2, &[10, 11]), 10)).unwrap();
    let songs_before = chain.songs().to_vec();
    let current_before = chain.current().clone();
    let uses_before = chain.remaining_uses(credit(10));

    let result = chain.apply(attempt(song(3, &[12]), 12));

    assert!(result.is_err());
    assert_eq!(chain.songs(), songs_before.as_slice());
    assert_eq!(chain.current(), &current_before);
    assert_eq!(chain.remaining_uses(credit(10)), uses_before);
}

#[test]
fn earlier_rule_wins_when_a_move_breaks_two_rules() {
    let mut chain = Chain::new(song(1, &[10]));
    chain.apply(attempt(song(2, &[10]), 10)).unwrap();

    // Already played, and credit 99 is on neither song.
    let replay = chain.validate(&attempt(song(1, &[10]), 99));
    assert_eq!(replay, Err(MoveRejection::SongAlreadyPlayed));

    // Credit 99 is on neither the current song nor the submitted one.
    let missing = chain.validate(&attempt(song(3, &[10]), 99));
    assert_eq!(missing, Err(MoveRejection::CreditNotOnPreviousSong));
}

#[test]
fn validate_does_not_change_the_chain() {
    let chain = Chain::new(song(1, &[10]));

    assert_eq!(chain.validate(&attempt(song(2, &[10]), 10)), Ok(()));
    assert_eq!(chain.songs(), &[rec(1)]);
    assert_eq!(chain.remaining_uses(credit(10)), MAX_USES);
}
