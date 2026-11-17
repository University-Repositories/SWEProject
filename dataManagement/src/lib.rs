use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

pub mod importer;
pub use importer::{
    CatalogImporter, CoverageReport, ImportCredit, ImportRecording, load_default_subset,
    populate_pop_songs,
};

// -----------------------------------------------------------------------------
// Database Helpers
// -----------------------------------------------------------------------------

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn seed_dev_fixture(pool: &PgPool) -> Result<(), sqlx::Error> {
    const FIXTURE_SQL: &str = include_str!("../fixtures/dev_fixture.sql");
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(FIXTURE_SQL).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Error Handling
// -----------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Conflict on constraint: {0}")]
    Conflict(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl RepoError {
    pub fn from_sqlx(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err
            && db_err.code().as_deref() == Some("23505")
        {
            let constraint = db_err
                .constraint()
                .unwrap_or("unique_violation")
                .to_string();
            return RepoError::Conflict(constraint);
        }
        RepoError::Database(err)
    }
}

// -----------------------------------------------------------------------------
// Models
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct SessionRow {
    pub token_hash: Vec<u8>,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct RecordingSummaryRow {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct CreditEntityRow {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingCredit {
    pub credit_id: Uuid,
    pub name: String,
    pub kind: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingDetailRow {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
    pub credits: Vec<RecordingCredit>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct GameRow {
    pub id: Uuid,
    pub mode: String,
    pub status: String,
    pub opening_recording_id: Uuid,
    pub accepted_links: Option<i32>,
    pub winner_user_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct GamePlayerRow {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub lives_remaining: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct MoveRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub turn_number: i32,
    pub recording_id: Uuid,
    pub credit_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct ReportRow {
    pub id: Uuid,
    pub reporter_id: Uuid,
    pub move_id: Option<Uuid>,
    pub credit_id: Option<Uuid>,
    pub explanation: String,
    pub status: String,
    pub resolution_reason: Option<String>,
    pub reviewer_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct UserGameRow {
    pub id: Uuid,
    pub mode: String,
    pub accepted_links: Option<i32>,
    pub opponent_username: Option<String>,
    pub won: Option<bool>,
    pub ended_at: Option<DateTime<Utc>>,
}

pub type UserGame = UserGameRow;
pub type UserEndedGame = UserGameRow;
pub type UserEndedGameRow = UserGameRow;

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct UserRecord {
    pub best_solo_chain: i32,
    pub matches_won: i64,
    pub matches_lost: i64,
}

pub type UserRecordRow = UserRecord;

// -----------------------------------------------------------------------------
// UserRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create(&self, username: &str, password_hash: &str) -> Result<UserRow, RepoError> {
        Self::create_with_executor(&self.pool, username, password_hash).await
    }

    pub async fn create_with_executor<'e, E>(
        executor: E,
        username: &str,
        password_hash: &str,
    ) -> Result<UserRow, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id, username, password_hash, role, display_name, created_at",
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserRow>, RepoError> {
        Self::find_by_username_with_executor(&self.pool, username).await
    }

    pub async fn find_by_username_with_executor<'e, E>(
        executor: E,
        username: &str,
    ) -> Result<Option<UserRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, role, display_name, created_at FROM users WHERE lower(username) = lower($1)",
        )
        .bind(username)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRow>, RepoError> {
        Self::find_by_id_with_executor(&self.pool, id).await
    }

    pub async fn find_by_id_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, role, display_name, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}

// -----------------------------------------------------------------------------
// SessionRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionRepo {
    pool: PgPool,
}

impl SessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepoError> {
        Self::create_with_executor(&self.pool, user_id, token_hash, expires_at).await
    }

    pub async fn create_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(token_hash)
            .bind(user_id)
            .bind(expires_at)
            .execute(executor)
            .await
            .map_err(RepoError::from_sqlx)?;
        Ok(())
    }

    pub async fn find_valid(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRow>, RepoError> {
        Self::find_valid_with_executor(&self.pool, token_hash, now).await
    }

    pub async fn find_valid_with_executor<'e, E>(
        executor: E,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, SessionRow>(
            "SELECT token_hash, user_id, created_at, expires_at FROM sessions WHERE token_hash = $1 AND expires_at > $2",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn delete(&self, token_hash: &[u8]) -> Result<(), RepoError> {
        Self::delete_with_executor(&self.pool, token_hash).await
    }

    pub async fn delete_with_executor<'e, E>(
        executor: E,
        token_hash: &[u8],
    ) -> Result<(), RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(executor)
            .await
            .map_err(RepoError::from_sqlx)?;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// CatalogRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CatalogRepo {
    pool: PgPool,
}

impl CatalogRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<RecordingSummaryRow>, RepoError> {
        Self::search_with_executor(&self.pool, query, limit).await
    }

    pub async fn search_with_executor<'e, E>(
        executor: E,
        query: &str,
        limit: i64,
    ) -> Result<Vec<RecordingSummaryRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, RecordingSummaryRow>(
            r#"
            SELECT id, title, artist_name, year, release_id
            FROM recordings
            WHERE $1 <% title OR $1 <% artist_name
            ORDER BY
                GREATEST(word_similarity($1, title), word_similarity($1, artist_name)) DESC,
                GREATEST(similarity(title, $1), similarity(artist_name, $1)) DESC,
                year ASC NULLS LAST,
                id ASC
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get_recording(&self, id: Uuid) -> Result<Option<RecordingDetailRow>, RepoError> {
        Self::get_recording_with_executor(&self.pool, id).await
    }

    pub async fn get_recording_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<RecordingDetailRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e> + Copy,
    {
        let rec = sqlx::query_as::<_, RecordingSummaryRow>(
            "SELECT id, title, artist_name, year, release_id FROM recordings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        let rec = match rec {
            Some(r) => r,
            None => return Ok(None),
        };

        let credits_rows = sqlx::query(
            r#"
            SELECT rc.credit_id, ce.name, ce.kind, rc.role
            FROM recording_credits rc
            JOIN credit_entities ce ON ce.id = rc.credit_id
            WHERE rc.recording_id = $1
            ORDER BY rc.role ASC, ce.name ASC
            "#,
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        let credits = credits_rows
            .into_iter()
            .map(|r| RecordingCredit {
                credit_id: r.get("credit_id"),
                name: r.get("name"),
                kind: r.get("kind"),
                role: r.get("role"),
            })
            .collect();

        Ok(Some(RecordingDetailRow {
            id: rec.id,
            title: rec.title,
            artist_name: rec.artist_name,
            year: rec.year,
            release_id: rec.release_id,
            credits,
        }))
    }

    pub async fn get_credit_ids(&self, id: Uuid) -> Result<Vec<Uuid>, RepoError> {
        Self::get_credit_ids_with_executor(&self.pool, id).await
    }

    pub async fn get_credit_ids_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Vec<Uuid>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let rows = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT credit_id FROM recording_credits WHERE recording_id = $1 ORDER BY credit_id ASC",
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(rows)
    }

    pub async fn random_playable_recording(&self) -> Result<Option<Uuid>, RepoError> {
        Self::random_playable_recording_with_executor(&self.pool).await
    }

    pub async fn random_playable_recording_with_executor<'e, E>(
        executor: E,
    ) -> Result<Option<Uuid>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT r.id
            FROM recordings r
            WHERE EXISTS (
                SELECT 1
                FROM recording_credits rc1
                JOIN recording_credits rc2
                    ON rc1.credit_id = rc2.credit_id
                    AND rc1.recording_id != rc2.recording_id
                WHERE rc1.recording_id = r.id
            )
            ORDER BY random()
            LIMIT 1
            "#,
        )
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}

// -----------------------------------------------------------------------------
// GameRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GameRepo {
    pool: PgPool,
}

impl GameRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_solo(
        &self,
        user_id: Uuid,
        opening_recording_id: Uuid,
    ) -> Result<GameRow, RepoError> {
        let mut tx = self.pool.begin().await.map_err(RepoError::Database)?;
        let game = Self::create_solo_with_tx(&mut tx, user_id, opening_recording_id).await?;
        tx.commit().await.map_err(RepoError::Database)?;
        Ok(game)
    }

    pub async fn create_solo_with_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        opening_recording_id: Uuid,
    ) -> Result<GameRow, RepoError> {
        let game = sqlx::query_as::<_, GameRow>(
            r#"
            INSERT INTO games (id, mode, status, opening_recording_id)
            VALUES (gen_random_uuid(), 'solo', 'active', $1)
            RETURNING
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at
            "#,
        )
        .bind(opening_recording_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        sqlx::query(
            "INSERT INTO game_players (game_id, user_id, lives_remaining) VALUES ($1, $2, NULL)",
        )
        .bind(game.id)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(game)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<GameRow>, RepoError> {
        Self::get_with_executor(&self.pool, id).await
    }

    pub async fn get_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<GameRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, GameRow>(
            r#"
            SELECT
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at
            FROM games
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get_players(&self, game_id: Uuid) -> Result<Vec<GamePlayerRow>, RepoError> {
        Self::get_players_with_executor(&self.pool, game_id).await
    }

    pub async fn get_players_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
    ) -> Result<Vec<GamePlayerRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, GamePlayerRow>(
            r#"
            SELECT game_id, user_id, lives_remaining
            FROM game_players
            WHERE game_id = $1
            ORDER BY user_id ASC
            "#,
        )
        .bind(game_id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn list_moves(&self, game_id: Uuid) -> Result<Vec<MoveRow>, RepoError> {
        Self::list_moves_with_executor(&self.pool, game_id).await
    }

    pub async fn list_moves_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
    ) -> Result<Vec<MoveRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, MoveRow>(
            r#"
            SELECT id, game_id, user_id, turn_number, recording_id, credit_id, created_at
            FROM moves
            WHERE game_id = $1
            ORDER BY turn_number ASC
            "#,
        )
        .bind(game_id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn add_move(
        &self,
        game_id: Uuid,
        user_id: Uuid,
        turn_number: i32,
        recording_id: Uuid,
        credit_id: Uuid,
    ) -> Result<MoveRow, RepoError> {
        Self::add_move_with_executor(
            &self.pool,
            game_id,
            user_id,
            turn_number,
            recording_id,
            credit_id,
        )
        .await
    }

    pub async fn add_move_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
        user_id: Uuid,
        turn_number: i32,
        recording_id: Uuid,
        credit_id: Uuid,
    ) -> Result<MoveRow, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, MoveRow>(
            r#"
            INSERT INTO moves (id, game_id, user_id, turn_number, recording_id, credit_id)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
            RETURNING id, game_id, user_id, turn_number, recording_id, credit_id, created_at
            "#,
        )
        .bind(game_id)
        .bind(user_id)
        .bind(turn_number)
        .bind(recording_id)
        .bind(credit_id)
        .fetch_one(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn end(&self, game_id: Uuid, accepted_links: i32) -> Result<bool, RepoError> {
        Self::end_with_executor(&self.pool, game_id, accepted_links, None).await
    }

    pub async fn end_with_winner(
        &self,
        game_id: Uuid,
        accepted_links: i32,
        winner_user_id: Option<Uuid>,
    ) -> Result<bool, RepoError> {
        Self::end_with_executor(&self.pool, game_id, accepted_links, winner_user_id).await
    }

    pub async fn end_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
        accepted_links: i32,
        winner_user_id: Option<Uuid>,
    ) -> Result<bool, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let res = sqlx::query(
            r#"
            UPDATE games
            SET status = 'ended',
                accepted_links = $2,
                winner_user_id = COALESCE($3, winner_user_id),
                ended_at = now()
            WHERE id = $1 AND status = 'active'
            "#,
        )
        .bind(game_id)
        .bind(accepted_links)
        .bind(winner_user_id)
        .execute(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn create_match(
        &self,
        player_one_id: Uuid,
        player_two_id: Uuid,
        opening_recording_id: Uuid,
    ) -> Result<GameRow, RepoError> {
        let mut tx = self.pool.begin().await.map_err(RepoError::Database)?;
        let game =
            Self::create_match_with_tx(&mut tx, player_one_id, player_two_id, opening_recording_id)
                .await?;
        tx.commit().await.map_err(RepoError::Database)?;
        Ok(game)
    }

    pub async fn create_match_with_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        player_one_id: Uuid,
        player_two_id: Uuid,
        opening_recording_id: Uuid,
    ) -> Result<GameRow, RepoError> {
        let game = sqlx::query_as::<_, GameRow>(
            r#"
            INSERT INTO games (id, mode, status, opening_recording_id)
            VALUES (gen_random_uuid(), 'live', 'active', $1)
            RETURNING
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at
            "#,
        )
        .bind(opening_recording_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        sqlx::query(
            "INSERT INTO game_players (game_id, user_id, lives_remaining) VALUES ($1, $2, 3), ($1, $3, 3)",
        )
        .bind(game.id)
        .bind(player_one_id)
        .bind(player_two_id)
        .execute(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(game)
    }

    pub async fn list_user_games(
        &self,
        user_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<UserGameRow>, RepoError> {
        Self::list_user_games_with_executor(&self.pool, user_id, limit, before).await
    }

    pub async fn list_ended_games(
        &self,
        user_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<UserGameRow>, RepoError> {
        Self::list_user_games_with_executor(&self.pool, user_id, limit, before).await
    }

    pub async fn list_user_ended_games(
        &self,
        user_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<UserGameRow>, RepoError> {
        Self::list_user_games_with_executor(&self.pool, user_id, limit, before).await
    }

    pub async fn list_user_games_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<UserGameRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserGameRow>(
            r#"
            SELECT
                g.id,
                g.mode,
                g.accepted_links,
                opponent_u.username AS opponent_username,
                CASE
                    WHEN g.mode = 'solo' THEN NULL
                    WHEN g.winner_user_id = $1 THEN TRUE
                    WHEN g.winner_user_id IS NOT NULL THEN FALSE
                    ELSE NULL
                END AS won,
                g.ended_at
            FROM games g
            JOIN game_players gp ON g.id = gp.game_id
            LEFT JOIN game_players opponent_gp
                ON opponent_gp.game_id = g.id AND opponent_gp.user_id != $1
            LEFT JOIN users opponent_u
                ON opponent_u.id = opponent_gp.user_id
            WHERE gp.user_id = $1
              AND g.status = 'ended'
              AND g.ended_at IS NOT NULL
              AND ($2::TIMESTAMPTZ IS NULL OR g.ended_at < $2)
            ORDER BY g.ended_at DESC, g.id DESC
            LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(before)
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get_user_record(&self, user_id: Uuid) -> Result<UserRecord, RepoError> {
        Self::get_user_record_with_executor(&self.pool, user_id).await
    }

    pub async fn get_record(&self, user_id: Uuid) -> Result<UserRecord, RepoError> {
        Self::get_user_record_with_executor(&self.pool, user_id).await
    }

    pub async fn get_user_record_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
    ) -> Result<UserRecord, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, UserRecord>(
            r#"
            SELECT
                COALESCE(MAX(CASE WHEN g.mode = 'solo' THEN g.accepted_links ELSE NULL END), 0)::INT AS best_solo_chain,
                COUNT(CASE WHEN g.mode = 'live' AND g.winner_user_id = $1 THEN 1 ELSE NULL END)::BIGINT AS matches_won,
                COUNT(CASE WHEN g.mode = 'live' AND g.winner_user_id IS NOT NULL AND g.winner_user_id != $1 THEN 1 ELSE NULL END)::BIGINT AS matches_lost
            FROM games g
            JOIN game_players gp ON g.id = gp.game_id
            WHERE gp.user_id = $1
              AND g.status = 'ended'
            "#,
        )
        .bind(user_id)
        .fetch_one(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}

// -----------------------------------------------------------------------------
// ReportRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReportRepo {
    pool: PgPool,
}

impl ReportRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create(
        &self,
        reporter_id: Uuid,
        move_id: Option<Uuid>,
        credit_id: Option<Uuid>,
        explanation: &str,
    ) -> Result<ReportRow, RepoError> {
        Self::create_with_executor(&self.pool, reporter_id, move_id, credit_id, explanation).await
    }

    pub async fn create_with_executor<'e, E>(
        executor: E,
        reporter_id: Uuid,
        move_id: Option<Uuid>,
        credit_id: Option<Uuid>,
        explanation: &str,
    ) -> Result<ReportRow, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, ReportRow>(
            r#"
            INSERT INTO reports (id, reporter_id, move_id, credit_id, explanation, status)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, 'open')
            RETURNING
                id, reporter_id, move_id, credit_id, explanation,
                status, resolution_reason, reviewer_id, created_at, resolved_at
            "#,
        )
        .bind(reporter_id)
        .bind(move_id)
        .bind(credit_id)
        .bind(explanation)
        .fetch_one(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<ReportRow>, RepoError> {
        Self::get_with_executor(&self.pool, id).await
    }

    pub async fn get_with_executor<'e, E>(
        executor: E,
        id: Uuid,
    ) -> Result<Option<ReportRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, ReportRow>(
            r#"
            SELECT
                id, reporter_id, move_id, credit_id, explanation,
                status, resolution_reason, reviewer_id, created_at, resolved_at
            FROM reports
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}
