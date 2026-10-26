use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::RepoError;
use crate::models::{RecordingCredit, RecordingDetailRow, RecordingSummaryRow};

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
            ORDER BY
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
            r#"
            SELECT id, title, artist_name, year, release_id
            FROM recordings
            WHERE id = $1
            "#,
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
            r#"
            SELECT DISTINCT credit_id
            FROM recording_credits
            WHERE recording_id = $1
            ORDER BY credit_id ASC
            "#,
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(RepoError::from_sqlx)?;

        Ok(rows)
    }
}
