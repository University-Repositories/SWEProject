use game_engine::{Chain, MoveAttempt};

use crate::{
    api_types::{ApiError, ApiErrorCode},
    chain::ChainCheckResponse,
};

pub fn submit_move(
    chain: &mut Chain,
    ended: bool,
    attempt: MoveAttempt,
) -> Result<ChainCheckResponse, ApiError> {
    if ended {
        return Err(ApiError::new(
            ApiErrorCode::GameEnded,
            "Game has ended",
            None,
        ));
    }

    // i want the ui to get the credit count or the reason the move failed
    Ok(ChainCheckResponse::from_result(chain.apply(attempt)))
}

pub fn end_game(ended: &mut bool) -> bool {
    // i want the caller to know if this req ended the game
    !std::mem::replace(ended, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::MoveRejectionCode;
    use game_engine::{CreditId, RecordingId, Song};
    use uuid::Uuid;

    fn attempt(id: u128) -> MoveAttempt {
        let credit = CreditId(Uuid::from_u128(1));
        MoveAttempt {
            song: Song::new(RecordingId(Uuid::from_u128(id)), [credit]),
            selected_credit: credit,
        }
    }

    #[test]
    fn submit_move_tracks_credit_uses_and_leaves_rejected_moves_out() {
        let mut chain = Chain::new(attempt(10).song);
        for uses in 1..=3 {
            let result = submit_move(&mut chain, false, attempt(10 + u128::from(uses))).unwrap();
            assert!(result.accepted);
            assert_eq!(result.uses_after, Some(uses));
        }

        let duplicate = submit_move(&mut chain, false, attempt(11)).unwrap();
        assert_eq!(duplicate.rejection, Some(MoveRejectionCode::SongAlreadyPlayed));
        let exhausted = submit_move(&mut chain, false, attempt(14)).unwrap();
        assert_eq!(exhausted.rejection, Some(MoveRejectionCode::CreditExhausted));
        assert_eq!(chain.songs().len(), 4);
    }

    #[test]
    fn ending_twice_only_changes_state_once_and_blocks_moves() {
        let mut chain = Chain::new(attempt(10).song);
        let mut ended = false;
        assert!(end_game(&mut ended));
        assert!(!end_game(&mut ended));
        assert!(ended);
        assert!(submit_move(&mut chain, ended, attempt(11)).is_err());
        assert_eq!(chain.songs().len(), 1);
    }
}
