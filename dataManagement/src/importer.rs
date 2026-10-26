use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::RepoError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCredit {
    pub id: Uuid,
    pub kind: String, // "person", "label", "album"
    pub name: String,
    pub role: String, // "artist", "feature", "producer", "writer", "label", "album"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRecording {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
    pub credits: Vec<ImportCredit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub total_recordings: i64,
    pub total_credit_entities: i64,
    pub recordings_sharing_credits: i64,
}

impl std::fmt::Display for CoverageReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let percent = if self.total_recordings > 0 {
            (self.recordings_sharing_credits as f64 / self.total_recordings as f64) * 100.0
        } else {
            0.0
        };
        writeln!(
            f,
            "================ Catalog Coverage Report ================"
        )?;
        writeln!(f, "Total Recordings:        {}", self.total_recordings)?;
        writeln!(f, "Total Credit Entities:   {}", self.total_credit_entities)?;
        writeln!(
            f,
            "Shared Credit Songs:     {} ({:.1}%)",
            self.recordings_sharing_credits, percent
        )?;
        writeln!(
            f,
            "========================================================="
        )?;
        Ok(())
    }
}

pub struct CatalogImporter {
    min_credit_count: usize,
}

impl Default for CatalogImporter {
    fn default() -> Self {
        let min_count = std::env::var("MIN_CREDIT_COUNT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);
        Self {
            min_credit_count: min_count,
        }
    }
}

impl CatalogImporter {
    pub fn new(min_credit_count: usize) -> Self {
        Self { min_credit_count }
    }

    /// Imports a slice of `ImportRecording` items into the database.
    ///
    /// Stages:
    /// 1. Filter: Drops songs with fewer than `min_credit_count` distinct credit entities.
    /// 2. Load: Inserts recordings, credit_entities, and recording_credits.
    ///    Idempotent: Running twice on the same input leaves tables unchanged.
    pub async fn import_recordings(
        &self,
        pool: &PgPool,
        recordings: &[ImportRecording],
    ) -> Result<CoverageReport, RepoError> {
        let mut tx = pool.begin().await.map_err(RepoError::Database)?;

        for rec in recordings {
            // Count distinct credit entity IDs
            let distinct_credits: HashSet<Uuid> = rec.credits.iter().map(|c| c.id).collect();
            if distinct_credits.len() < self.min_credit_count {
                // Drop song: every playable song must have sufficient links
                continue;
            }

            // Insert recording
            sqlx::query(
                r#"
                INSERT INTO recordings (id, title, artist_name, year, release_id)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (id) DO UPDATE SET
                    title = EXCLUDED.title,
                    artist_name = EXCLUDED.artist_name,
                    year = EXCLUDED.year,
                    release_id = EXCLUDED.release_id
                "#,
            )
            .bind(rec.id)
            .bind(&rec.title)
            .bind(&rec.artist_name)
            .bind(rec.year)
            .bind(rec.release_id)
            .execute(&mut *tx)
            .await
            .map_err(RepoError::Database)?;

            // Insert credit entities and recording credits
            for credit in &rec.credits {
                sqlx::query(
                    r#"
                    INSERT INTO credit_entities (id, kind, name)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (id) DO UPDATE SET
                        kind = EXCLUDED.kind,
                        name = EXCLUDED.name
                    "#,
                )
                .bind(credit.id)
                .bind(&credit.kind)
                .bind(&credit.name)
                .execute(&mut *tx)
                .await
                .map_err(RepoError::Database)?;

                sqlx::query(
                    r#"
                    INSERT INTO recording_credits (recording_id, credit_id, role)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (recording_id, credit_id, role) DO NOTHING
                    "#,
                )
                .bind(rec.id)
                .bind(credit.id)
                .bind(&credit.role)
                .execute(&mut *tx)
                .await
                .map_err(RepoError::Database)?;
            }
        }

        tx.commit().await.map_err(RepoError::Database)?;

        Self::generate_coverage_report(pool).await
    }

    /// Imports recordings from a JSON string.
    pub async fn import_from_json(
        &self,
        pool: &PgPool,
        json_str: &str,
    ) -> Result<CoverageReport, RepoError> {
        let recordings: Vec<ImportRecording> = serde_json::from_str(json_str)
            .map_err(|e| RepoError::Database(sqlx::Error::Decode(Box::new(e))))?;
        self.import_recordings(pool, &recordings).await
    }

    /// Computes the coverage report directly from the database tables.
    pub async fn generate_coverage_report(pool: &PgPool) -> Result<CoverageReport, RepoError> {
        let mut conn = pool.acquire().await.map_err(RepoError::Database)?;
        Self::generate_coverage_report_with_conn(&mut conn).await
    }

    pub async fn generate_coverage_report_with_conn(
        conn: &mut sqlx::PgConnection,
    ) -> Result<CoverageReport, RepoError> {
        let total_rec: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings")
            .fetch_one(&mut *conn)
            .await
            .map_err(RepoError::Database)?;

        let total_credits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_entities")
            .fetch_one(&mut *conn)
            .await
            .map_err(RepoError::Database)?;

        let shared_rec: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT rc1.recording_id)
            FROM recording_credits rc1
            JOIN recording_credits rc2
                ON rc1.credit_id = rc2.credit_id
                AND rc1.recording_id != rc2.recording_id
            "#,
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(RepoError::Database)?;

        Ok(CoverageReport {
            total_recordings: total_rec,
            total_credit_entities: total_credits,
            recordings_sharing_credits: shared_rec,
        })
    }
}

/// Helper to load the bundled default MusicBrainz subset.
pub async fn load_default_subset(
    pool: &PgPool,
    min_credit_count: usize,
) -> Result<CoverageReport, RepoError> {
    const DEFAULT_DATA: &str = include_str!("../data/musicbrainz_subset.json");
    let importer = CatalogImporter::new(min_credit_count);
    importer.import_from_json(pool, DEFAULT_DATA).await
}

/// Optional live import from MusicBrainz API (fixed clippy warnings).
pub async fn populate_pop_songs(
    pool: &PgPool,
    target_count: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    use musicbrainz_rs::MusicBrainzClient;
    use musicbrainz_rs::entity::recording::{Recording, RecordingSearchQuery};
    use musicbrainz_rs::prelude::*;

    let client = MusicBrainzClient::new("Beat4Beat/0.1.0 ( contact@yourproject.org )");
    let pop_tags = ["pop", "dance-pop", "synth-pop", "pop-rock"];
    let page_size: u8 = 100;
    let max_offset_per_tag: u16 = 300;
    let mut total_inserted = 0;
    let importer = CatalogImporter::new(1);

    for tag in pop_tags {
        let mut offset: u16 = 0;
        while offset < max_offset_per_tag && total_inserted < target_count {
            let current_limit = ((target_count - total_inserted).min(page_size as usize)) as u8;
            let query_str = RecordingSearchQuery::query_builder().tag(tag).build();

            let mut search_query = Recording::search(query_str);
            search_query.limit(current_limit);
            search_query.offset(offset);

            let results = search_query.execute_with_client(&client)?;
            if results.entities.is_empty() {
                break;
            }

            let mut batch = Vec::new();
            for rec in results.entities {
                let rec_id = match Uuid::parse_str(&rec.id) {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                let mut credits = Vec::new();
                let artist_name = rec
                    .artist_credit
                    .as_ref()
                    .and_then(|ac| ac.first())
                    .map(|ac| ac.name.clone())
                    .unwrap_or_else(|| "Unknown Artist".to_string());

                if let Some(artist_credits) = &rec.artist_credit {
                    for (idx, ac) in artist_credits.iter().enumerate() {
                        let role = if idx == 0 { "artist" } else { "feature" };
                        let credit_id =
                            Uuid::parse_str(&ac.artist.id).unwrap_or_else(|_| Uuid::new_v4());
                        credits.push(ImportCredit {
                            id: credit_id,
                            kind: "person".to_string(),
                            name: ac.name.clone(),
                            role: role.to_string(),
                        });
                    }
                }

                let mut earliest_year: Option<i32> = None;
                let mut release_id: Option<Uuid> = None;

                if let Some(releases) = &rec.releases {
                    for rel in releases {
                        if release_id.is_none() {
                            release_id = Uuid::parse_str(&rel.id).ok();
                        }
                        if let Some(rg) = &rel.release_group
                            && let Ok(rg_id) = Uuid::parse_str(&rg.id)
                        {
                            credits.push(ImportCredit {
                                id: rg_id,
                                kind: "album".to_string(),
                                name: rg.title.clone(),
                                role: "album".to_string(),
                            });
                        }
                        if let Some(date_str) = &rel.date
                            && date_str.0.len() >= 4
                            && let Ok(year) = date_str.0[0..4].parse::<i32>()
                        {
                            earliest_year = match earliest_year {
                                Some(existing) => Some(existing.min(year)),
                                None => Some(year),
                            };
                        }
                    }
                }

                batch.push(ImportRecording {
                    id: rec_id,
                    title: rec.title,
                    artist_name,
                    year: earliest_year,
                    release_id,
                    credits,
                });
            }

            importer.import_recordings(pool, &batch).await?;
            total_inserted += batch.len();
            offset += page_size as u16;
        }
    }

    Ok(total_inserted)
}
