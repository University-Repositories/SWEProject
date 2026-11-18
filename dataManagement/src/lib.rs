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
    pub bio: Option<String>,
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
    pub invite_code: Option<String>,
    pub turn_user_id: Option<Uuid>,
    pub turn_number: i32,
    pub turn_deadline: Option<DateTime<Utc>>,
    pub version: i32,
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
    pub decision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct MatchQueueRow {
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchDetails {
    pub game: GameRow,
    pub players: Vec<GamePlayerRow>,
    pub moves: Vec<MoveRow>,
}

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
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id, username, password_hash, role, display_name, bio, created_at",
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
            "SELECT id, username, password_hash, role, display_name, bio, created_at FROM users WHERE lower(username) = lower($1)",
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
            "SELECT id, username, password_hash, role, display_name, bio, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn get_role(&self, user_id: Uuid) -> Result<Option<String>, RepoError> {
        Self::get_role_with_executor(&self.pool, user_id).await
    }

    pub async fn get_role_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
    ) -> Result<Option<String>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(executor)
            .await
            .map_err(RepoError::from_sqlx)
    }

    pub async fn set_role(&self, user_id: Uuid, role: &str) -> Result<bool, RepoError> {
        Self::set_role_with_executor(&self.pool, user_id, role).await
    }

    pub async fn set_role_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
        role: &str,
    ) -> Result<bool, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let res = sqlx::query("UPDATE users SET role = $2 WHERE id = $1")
            .bind(user_id)
            .bind(role)
            .execute(executor)
            .await
            .map_err(RepoError::from_sqlx)?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn update_profile(
        &self,
        user_id: Uuid,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool, RepoError> {
        Self::update_profile_with_executor(&self.pool, user_id, display_name, bio).await
    }

    pub async fn update_display_name_and_bio(
        &self,
        user_id: Uuid,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool, RepoError> {
        Self::update_profile_with_executor(&self.pool, user_id, display_name, bio).await
    }

    pub async fn update_profile_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let res = sqlx::query("UPDATE users SET display_name = $2, bio = $3 WHERE id = $1")
            .bind(user_id)
            .bind(display_name)
            .bind(bio)
            .execute(executor)
            .await
            .map_err(RepoError::from_sqlx)?;
        Ok(res.rows_affected() > 0)
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
            INSERT INTO games (id, mode, status, opening_recording_id, version)
            VALUES (gen_random_uuid(), 'solo', 'active', $1, 0)
            RETURNING
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at,
                invite_code, turn_user_id, turn_number, turn_deadline, version
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
                accepted_links, winner_user_id, started_at, ended_at,
                invite_code, turn_user_id, turn_number, turn_deadline, version
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

    pub async fn create_waiting_match(
        &self,
        creator_user_id: Uuid,
        opening_recording_id: Uuid,
        invite_code: &str,
    ) -> Result<GameRow, RepoError> {
        MatchRepo::new(self.pool.clone())
            .create_waiting_match(creator_user_id, opening_recording_id, invite_code)
            .await
    }

    pub async fn find_match_by_invite_code(
        &self,
        invite_code: &str,
    ) -> Result<Option<GameRow>, RepoError> {
        MatchRepo::new(self.pool.clone())
            .find_by_invite_code(invite_code)
            .await
    }

    pub async fn add_second_player(
        &self,
        game_id: Uuid,
        second_player_user_id: Uuid,
    ) -> Result<bool, RepoError> {
        MatchRepo::new(self.pool.clone())
            .add_second_player(game_id, second_player_user_id)
            .await
    }

    pub async fn load_match(&self, game_id: Uuid) -> Result<Option<MatchDetails>, RepoError> {
        MatchRepo::new(self.pool.clone()).load_match(game_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_match(
        &self,
        game_id: Uuid,
        expected_version: i32,
        status: &str,
        turn_user_id: Option<Uuid>,
        turn_number: i32,
        turn_deadline: Option<DateTime<Utc>>,
        accepted_links: Option<i32>,
        winner_user_id: Option<Uuid>,
    ) -> Result<bool, RepoError> {
        MatchRepo::new(self.pool.clone())
            .save_match(
                game_id,
                expected_version,
                status,
                turn_user_id,
                turn_number,
                turn_deadline,
                accepted_links,
                winner_user_id,
            )
            .await
    }

    pub async fn find_user_waiting_or_active_match(
        &self,
        user_id: Uuid,
    ) -> Result<Option<GameRow>, RepoError> {
        MatchRepo::new(self.pool.clone())
            .find_user_waiting_or_active_match(user_id)
            .await
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
                status, resolution_reason, reviewer_id, created_at, resolved_at, decision
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
                status, resolution_reason, reviewer_id, created_at, resolved_at, decision
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

// -----------------------------------------------------------------------------
// MatchRepo
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MatchRepo {
    pool: PgPool,
}

impl MatchRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_waiting_match(
        &self,
        creator_user_id: Uuid,
        opening_recording_id: Uuid,
        invite_code: &str,
    ) -> Result<GameRow, RepoError> {
        let mut tx = self.pool.begin().await.map_err(RepoError::Database)?;
        let game = Self::create_waiting_match_with_tx(
            &mut tx,
            creator_user_id,
            opening_recording_id,
            invite_code,
        )
        .await?;
        tx.commit().await.map_err(RepoError::Database)?;
        Ok(game)
    }

    pub async fn create_waiting_match_with_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        creator_user_id: Uuid,
        opening_recording_id: Uuid,
        invite_code: &str,
    ) -> Result<GameRow, RepoError> {
        let game = sqlx::query_as::<_, GameRow>(
            r#"
            INSERT INTO games (
                id, mode, status, opening_recording_id,
                invite_code, turn_user_id, turn_number, version
            )
            VALUES (gen_random_uuid(), 'live', 'waiting', $1, $2, $3, 0, 0)
            RETURNING
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at,
                invite_code, turn_user_id, turn_number, turn_deadline, version
            "#,
        )
        .bind(opening_recording_id)
        .bind(invite_code)
        .bind(creator_user_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        sqlx::query(
            "INSERT INTO game_players (game_id, user_id, lives_remaining) VALUES ($1, $2, 3)",
        )
        .bind(game.id)
        .bind(creator_user_id)
        .execute(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(game)
    }

    pub async fn find_by_invite_code(
        &self,
        invite_code: &str,
    ) -> Result<Option<GameRow>, RepoError> {
        Self::find_by_invite_code_with_executor(&self.pool, invite_code).await
    }

    pub async fn find_by_invite_code_with_executor<'e, E>(
        executor: E,
        invite_code: &str,
    ) -> Result<Option<GameRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, GameRow>(
            r#"
            SELECT
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at,
                invite_code, turn_user_id, turn_number, turn_deadline, version
            FROM games
            WHERE invite_code = $1
            "#,
        )
        .bind(invite_code)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }

    pub async fn add_second_player(
        &self,
        game_id: Uuid,
        second_player_user_id: Uuid,
    ) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(RepoError::Database)?;
        let added =
            Self::add_second_player_with_tx(&mut tx, game_id, second_player_user_id).await?;
        tx.commit().await.map_err(RepoError::Database)?;
        Ok(added)
    }

    pub async fn add_second_player_with_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        game_id: Uuid,
        second_player_user_id: Uuid,
    ) -> Result<bool, RepoError> {
        let res = sqlx::query(
            r#"
            UPDATE games
            SET status = 'active',
                turn_number = 1,
                version = version + 1
            WHERE id = $1 AND status = 'waiting'
            "#,
        )
        .bind(game_id)
        .execute(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        if res.rows_affected() == 0 {
            return Ok(false);
        }

        sqlx::query(
            "INSERT INTO game_players (game_id, user_id, lives_remaining) VALUES ($1, $2, 3)",
        )
        .bind(game_id)
        .bind(second_player_user_id)
        .execute(&mut **tx)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(true)
    }

    pub async fn load_match(&self, game_id: Uuid) -> Result<Option<MatchDetails>, RepoError> {
        Self::load_match_with_executor(&self.pool, game_id).await
    }

    pub async fn load_match_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
    ) -> Result<Option<MatchDetails>, RepoError>
    where
        E: sqlx::PgExecutor<'e> + Copy,
    {
        let game = sqlx::query_as::<_, GameRow>(
            r#"
            SELECT
                id, mode, status, opening_recording_id,
                accepted_links, winner_user_id, started_at, ended_at,
                invite_code, turn_user_id, turn_number, turn_deadline, version
            FROM games
            WHERE id = $1
            "#,
        )
        .bind(game_id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        let Some(game) = game else {
            return Ok(None);
        };

        let players = sqlx::query_as::<_, GamePlayerRow>(
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
        .map_err(RepoError::from_sqlx)?;

        let moves = sqlx::query_as::<_, MoveRow>(
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
        .map_err(RepoError::from_sqlx)?;

        Ok(Some(MatchDetails {
            game,
            players,
            moves,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_match(
        &self,
        game_id: Uuid,
        expected_version: i32,
        status: &str,
        turn_user_id: Option<Uuid>,
        turn_number: i32,
        turn_deadline: Option<DateTime<Utc>>,
        accepted_links: Option<i32>,
        winner_user_id: Option<Uuid>,
    ) -> Result<bool, RepoError> {
        Self::save_match_with_executor(
            &self.pool,
            game_id,
            expected_version,
            status,
            turn_user_id,
            turn_number,
            turn_deadline,
            accepted_links,
            winner_user_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_match_with_executor<'e, E>(
        executor: E,
        game_id: Uuid,
        expected_version: i32,
        status: &str,
        turn_user_id: Option<Uuid>,
        turn_number: i32,
        turn_deadline: Option<DateTime<Utc>>,
        accepted_links: Option<i32>,
        winner_user_id: Option<Uuid>,
    ) -> Result<bool, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let res = sqlx::query(
            r#"
            UPDATE games
            SET status = $3,
                turn_user_id = $4,
                turn_number = $5,
                turn_deadline = $6,
                accepted_links = COALESCE($7, accepted_links),
                winner_user_id = COALESCE($8, winner_user_id),
                ended_at = CASE WHEN $3 = 'ended' AND ended_at IS NULL THEN now() ELSE ended_at END,
                version = version + 1
            WHERE id = $1 AND version = $2
            "#,
        )
        .bind(game_id)
        .bind(expected_version)
        .bind(status)
        .bind(turn_user_id)
        .bind(turn_number)
        .bind(turn_deadline)
        .bind(accepted_links)
        .bind(winner_user_id)
        .execute(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn find_user_waiting_or_active_match(
        &self,
        user_id: Uuid,
    ) -> Result<Option<GameRow>, RepoError> {
        Self::find_user_waiting_or_active_match_with_executor(&self.pool, user_id).await
    }

    pub async fn find_user_waiting_or_active_match_with_executor<'e, E>(
        executor: E,
        user_id: Uuid,
    ) -> Result<Option<GameRow>, RepoError>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query_as::<_, GameRow>(
            r#"
            SELECT
                g.id, g.mode, g.status, g.opening_recording_id,
                g.accepted_links, g.winner_user_id, g.started_at, g.ended_at,
                g.invite_code, g.turn_user_id, g.turn_number, g.turn_deadline, g.version
            FROM games g
            JOIN game_players gp ON g.id = gp.game_id
            WHERE gp.user_id = $1
              AND g.mode = 'live'
              AND g.status IN ('waiting', 'active')
            ORDER BY g.started_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(executor)
        .await
        .map_err(RepoError::from_sqlx)
    }
}
