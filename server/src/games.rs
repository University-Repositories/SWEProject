use crate::{
    api_types::{
        ApiError, ApiErrorCode, ChainLink, CreditUse, GameState, GameStatus, MoveRejectionDetail,
        MoveResult, StartGameRequest, SubmitMoveRequest, User,
    },
    app::AppState,
    auth::require_session,
    game_sessions::{
        GameError, GameMode, GameRecord, GameSessions, GameStatus as StoredStatus, GameStore,
        GameView, MoveRecord, NewMove, PgSongCatalog,
    },
    json::ApiJson,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use data_management::{GameRepo, GameRow, RepoError};
use uuid::Uuid;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(start))
        .route("/{id}", get(read))
        .route("/{id}/moves", post(submit))
        .route("/{id}/end", post(end))
        .route_layer(middleware::from_fn_with_state(state, require_session))
}

pub(crate) fn request_id(id: &str) -> Result<Uuid, ApiError> {
    id.parse().map_err(|_| ApiError::validation(Some("id")))
}

fn sessions(state: AppState) -> GameSessions<PgGames, PgSongCatalog> {
    GameSessions::new(
        PgGames(GameRepo::new(state.pool.clone())),
        PgSongCatalog::new(state.pool),
    )
}

async fn start(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    ApiJson(input): ApiJson<StartGameRequest>,
) -> Result<(StatusCode, Json<GameState>), ApiError> {
    let game = sessions(state)
        .start_solo(user.id, input.opening_recording_id)
        .await?;
    Ok((StatusCode::CREATED, Json(game.into())))
}

async fn read(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<GameState>, ApiError> {
    let game = sessions(state).get_state(user.id, request_id(&id)?).await?;
    Ok(Json(game.into()))
}

async fn submit(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<SubmitMoveRequest>,
) -> Result<Json<MoveResult>, ApiError> {
    let outcome = sessions(state)
        .submit_move(
            user.id,
            request_id(&id)?,
            input.recording_id,
            input.credit_id,
        )
        .await?;
    Ok(Json(MoveResult {
        accepted: outcome.accepted(),
        rejection: outcome.rejection.map(|why| MoveRejectionDetail {
            code: why.into(),
            message: why.to_string(),
        }),
        game: outcome.game.into(),
    }))
}

async fn end(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<GameState>, ApiError> {
    let game = sessions(state).end(user.id, request_id(&id)?).await?;
    Ok(Json(game.into()))
}

impl From<GameError> for ApiError {
    fn from(error: GameError) -> Self {
        let code = match error {
            GameError::NotFound => ApiErrorCode::NotFound,
            GameError::GameEnded => ApiErrorCode::GameEnded,
            GameError::TurnTaken => ApiErrorCode::TurnTaken,
            GameError::UnknownRecording | GameError::NoPlayableSongs => {
                ApiErrorCode::ValidationFailed
            }
            GameError::BrokenHistory | GameError::Repo(_) => return Self::internal(),
        };
        Self::new(code, &error.to_string(), None)
    }
}

impl From<GameView> for GameState {
    fn from(game: GameView) -> Self {
        Self {
            id: game.id,
            status: match game.status {
                StoredStatus::Active => GameStatus::Active,
                StoredStatus::Ended => GameStatus::Ended,
            },
            opening: game.opening,
            links: game
                .links
                .into_iter()
                .map(|link| ChainLink {
                    position: link.position,
                    recording: link.recording,
                    credit: link.credit,
                    uses_after: link.uses_after,
                })
                .collect(),
            credit_uses: game
                .credit_uses
                .into_iter()
                .map(|credit| CreditUse {
                    credit: credit.credit,
                    used: credit.used,
                    remaining: credit.remaining,
                })
                .collect(),
            accepted_links: game.accepted_links,
            started_at: game.started_at,
            ended_at: game.ended_at,
        }
    }
}

struct PgGames(GameRepo);

fn game_record(row: GameRow, player_ids: Vec<Uuid>) -> GameRecord {
    GameRecord {
        id: row.id,
        mode: GameMode::Solo,
        status: if row.status == "ended" {
            StoredStatus::Ended
        } else {
            StoredStatus::Active
        },
        player_ids,
        opening_recording_id: row.opening_recording_id,
        accepted_links: row.accepted_links,
        started_at: row.started_at,
        ended_at: row.ended_at,
    }
}

impl GameStore for PgGames {
    async fn create_solo(&self, user_id: Uuid, opening: Uuid) -> Result<GameRecord, RepoError> {
        let row = self.0.create_solo(user_id, opening).await?;
        Ok(game_record(row, vec![user_id]))
    }

    async fn get(&self, id: Uuid) -> Result<Option<GameRecord>, RepoError> {
        let Some(row) = self.0.get(id).await?.filter(|row| row.mode == "solo") else {
            return Ok(None);
        };
        let players = self.0.get_players(id).await?;
        Ok(Some(game_record(
            row,
            players.into_iter().map(|p| p.user_id).collect(),
        )))
    }

    async fn list_moves(&self, id: Uuid) -> Result<Vec<MoveRecord>, RepoError> {
        Ok(self
            .0
            .list_moves(id)
            .await?
            .into_iter()
            .map(|row| MoveRecord {
                turn_number: row.turn_number,
                user_id: row.user_id,
                recording_id: row.recording_id,
                credit_id: row.credit_id,
            })
            .collect())
    }

    async fn add_move(&self, input: NewMove) -> Result<(), RepoError> {
        let mut tx = self.0.pool().begin().await.map_err(RepoError::from_sqlx)?;
        // hold the row lock till the move is saved
        if lock_game(&mut tx, input.game_id).await?.as_deref() != Some("active") {
            return Err(RepoError::Conflict("game ended".into()));
        }
        GameRepo::add_move_with_executor(
            &mut *tx,
            input.game_id,
            input.user_id,
            input.turn_number,
            input.recording_id,
            input.credit_id,
        )
        .await?;
        tx.commit().await.map_err(RepoError::from_sqlx)
    }

    async fn end(&self, id: Uuid, count: i32, ended_at: DateTime<Utc>) -> Result<bool, RepoError> {
        let mut tx = self.0.pool().begin().await.map_err(RepoError::from_sqlx)?;
        if lock_game(&mut tx, id).await?.as_deref() != Some("active") {
            return Ok(false);
        }
        let updated = sqlx::query(
            "UPDATE games SET status = 'ended', accepted_links = $2, ended_at = $3
             WHERE id = $1 AND $2 = (SELECT count(*) FROM moves WHERE game_id = $1)",
        )
        .bind(id)
        .bind(count)
        .bind(ended_at)
        .execute(&mut *tx)
        .await
        .map_err(RepoError::from_sqlx)?;
        if updated.rows_affected() == 0 {
            return Err(RepoError::Conflict("move saved before end".into()));
        }
        tx.commit().await.map_err(RepoError::from_sqlx)?;
        Ok(true)
    }

    async fn random_playable_recording(&self) -> Result<Option<Uuid>, RepoError> {
        sqlx::query_scalar(
            "SELECT r.id FROM recordings r
             WHERE EXISTS (SELECT 1 FROM recording_credits rc WHERE rc.recording_id = r.id)
             ORDER BY random() LIMIT 1",
        )
        .fetch_optional(self.0.pool())
        .await
        .map_err(RepoError::from_sqlx)
    }
}

async fn lock_game(pool: &mut sqlx::PgConnection, id: Uuid) -> Result<Option<String>, RepoError> {
    sqlx::query_scalar("SELECT status FROM games WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(RepoError::from_sqlx)
}
