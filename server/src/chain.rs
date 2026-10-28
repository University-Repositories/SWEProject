//! Demo song chain routes for the Sprint 1 review.
//!
//! They run the game engine against catalog credits and save nothing. The client
//! sends the whole chain on every check and the server replays it through the
//! engine, so an invalid earlier move is still caught. Sprint 2 replaces these
//! routes with the game sessions service and the real game routes.

use axum::{
    Json, Router,
    extract::{Query, State},
    middleware,
    routing::{get, post},
};
use data_management::CatalogRepo;
use game_engine::{AcceptedLink, Chain, CreditId, MoveAttempt, MoveRejection, RecordingId, Song};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    api_types::{ApiError, ApiErrorCode},
    app::AppState,
    auth::require_session,
    json::ApiJson,
};

/// One link in the chain: the song played and the credit that linked it.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainMove {
    pub recording_id: Uuid,
    pub credit_id: Uuid,
}

/// The opening song, the moves accepted so far (oldest first), and the move to check.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ChainCheckRequest {
    pub opening_id: Uuid,
    pub moves: Vec<ChainMove>,
    pub next: ChainMove,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainStartResponse {
    pub recording_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum MoveRejectionCode {
    SongAlreadyPlayed,
    CreditNotOnPreviousSong,
    CreditNotOnSubmittedSong,
    CreditExhausted,
}

impl From<MoveRejection> for MoveRejectionCode {
    fn from(rejection: MoveRejection) -> Self {
        match rejection {
            MoveRejection::SongAlreadyPlayed => Self::SongAlreadyPlayed,
            MoveRejection::CreditNotOnPreviousSong => Self::CreditNotOnPreviousSong,
            MoveRejection::CreditNotOnSubmittedSong => Self::CreditNotOnSubmittedSong,
            MoveRejection::CreditExhausted => Self::CreditExhausted,
        }
    }
}

/// Accepted moves carry the credit's use count after the move.
/// Rejected moves carry the engine's reason.
#[derive(Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainCheckResponse {
    pub accepted: bool,
    pub uses_after: Option<u8>,
    pub rejection: Option<MoveRejectionCode>,
    pub message: Option<String>,
}

impl ChainCheckResponse {
    fn from_result(result: Result<AcceptedLink, MoveRejection>) -> Self {
        match result {
            Ok(link) => Self {
                accepted: true,
                uses_after: Some(link.uses_after),
                rejection: None,
                message: None,
            },
            Err(rejection) => Self {
                accepted: false,
                uses_after: None,
                rejection: Some(rejection.into()),
                message: Some(rejection.to_string()),
            },
        }
    }
}

/// An earlier move in the request breaks the rules.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidHistory;

/// Replays the chain through the engine, then checks the next move.
pub fn evaluate(
    opening: Song,
    history: Vec<MoveAttempt>,
    next: MoveAttempt,
) -> Result<Result<AcceptedLink, MoveRejection>, InvalidHistory> {
    let mut chain = Chain::new(opening);
    for attempt in history {
        chain.apply(attempt).map_err(|_| InvalidHistory)?;
    }
    Ok(chain.apply(next))
}

#[derive(Debug, Deserialize)]
pub struct StartParams {
    pub q: Option<String>,
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/start", get(start))
        .route("/check", post(check))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ))
        .with_state(state)
}

/// Picks the opening song: the best search match for `q`, or a random song.
async fn start(
    State(state): State<AppState>,
    Query(params): Query<StartParams>,
) -> Result<Json<ChainStartResponse>, ApiError> {
    let query = params.q.unwrap_or_default();
    let query = query.trim();
    let recording_id = if query.chars().count() >= 2 {
        CatalogRepo::new(state.pool.clone())
            .search(query, 1)
            .await
            .map_err(|_| ApiError::internal())?
            .into_iter()
            .next()
            .map(|row| row.id)
    } else {
        random_recording_id(&state.pool).await?
    };
    let recording_id = recording_id
        .ok_or_else(|| ApiError::new(ApiErrorCode::NotFound, "No songs in the catalog", None))?;
    Ok(Json(ChainStartResponse { recording_id }))
}

// Demo shortcut. Sprint 2 moves opening song selection into the game sessions service.
async fn random_recording_id(pool: &PgPool) -> Result<Option<Uuid>, ApiError> {
    let id: Option<String> =
        sqlx::query_scalar("SELECT id::text FROM recordings ORDER BY random() LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::internal())?;
    Ok(id.and_then(|value| Uuid::parse_str(&value).ok()))
}

async fn check(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ChainCheckRequest>,
) -> Result<Json<ChainCheckResponse>, ApiError> {
    let repo = CatalogRepo::new(state.pool.clone());
    let opening = load_song(&repo, request.opening_id).await?;
    let mut history = Vec::with_capacity(request.moves.len());
    for step in &request.moves {
        history.push(load_attempt(&repo, step).await?);
    }
    let next = load_attempt(&repo, &request.next).await?;
    let result =
        evaluate(opening, history, next).map_err(|_| ApiError::validation(Some("moves")))?;
    Ok(Json(ChainCheckResponse::from_result(result)))
}

async fn load_song(repo: &CatalogRepo, id: Uuid) -> Result<Song, ApiError> {
    let credits = repo
        .get_credit_ids(id)
        .await
        .map_err(|_| ApiError::internal())?;
    if credits.is_empty() {
        return Err(ApiError::new(
            ApiErrorCode::NotFound,
            "Recording not found",
            None,
        ));
    }
    Ok(Song::new(
        RecordingId(id),
        credits.into_iter().map(CreditId),
    ))
}

async fn load_attempt(repo: &CatalogRepo, step: &ChainMove) -> Result<MoveAttempt, ApiError> {
    Ok(MoveAttempt {
        song: load_song(repo, step.recording_id).await?,
        selected_credit: CreditId(step.credit_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn song(id: u128, credits: &[u128]) -> Song {
        Song::new(
            RecordingId(Uuid::from_u128(id)),
            credits
                .iter()
                .map(|&c| CreditId(Uuid::from_u128(1_000 + c))),
        )
    }

    fn attempt(next: Song, via: u128) -> MoveAttempt {
        MoveAttempt {
            song: next,
            selected_credit: CreditId(Uuid::from_u128(1_000 + via)),
        }
    }

    #[test]
    fn accepts_a_link_through_a_shared_credit() {
        let result = evaluate(song(1, &[10, 11]), vec![], attempt(song(2, &[11]), 11)).unwrap();
        let response = ChainCheckResponse::from_result(result);
        assert!(response.accepted);
        assert_eq!(response.uses_after, Some(1));
    }

    #[test]
    fn replays_earlier_moves_so_the_fourth_use_is_rejected() {
        let history = vec![
            attempt(song(2, &[10]), 10),
            attempt(song(3, &[10]), 10),
            attempt(song(4, &[10]), 10),
        ];
        let result = evaluate(song(1, &[10]), history, attempt(song(5, &[10]), 10)).unwrap();
        let response = ChainCheckResponse::from_result(result);
        assert!(!response.accepted);
        assert_eq!(response.rejection, Some(MoveRejectionCode::CreditExhausted));
        assert_eq!(
            response.message.as_deref(),
            Some("that credit has no uses left")
        );
    }

    #[test]
    fn reports_the_rejection_reason() {
        let result = evaluate(song(1, &[10]), vec![], attempt(song(2, &[20]), 20)).unwrap();
        let response = ChainCheckResponse::from_result(result);
        assert_eq!(
            response.rejection,
            Some(MoveRejectionCode::CreditNotOnPreviousSong)
        );
    }

    #[test]
    fn rejects_a_history_that_breaks_the_rules() {
        let history = vec![attempt(song(2, &[20]), 20)];
        let result = evaluate(song(1, &[10]), history, attempt(song(3, &[10]), 10));
        assert_eq!(result, Err(InvalidHistory));
    }

    #[tokio::test]
    async fn chain_routes_require_a_session() {
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/unused".into()),
            "SESSION_TTL_SECONDS" => Some("604800".into()),
            "COOKIE_SECURE" => Some("false".into()),
            _ => None,
        })
        .unwrap();
        let pool = PgPoolOptions::new()
            .connect_lazy(&config.database_url)
            .unwrap();
        let app = crate::app::router(AppState { pool, config });
        for (method, path) in [("GET", "/api/chain/start"), ("POST", "/api/chain/check")] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
}
