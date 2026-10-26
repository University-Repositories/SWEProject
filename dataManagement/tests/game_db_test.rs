use chrono::{Duration, Utc};
use data_management::{
    CatalogImporter, CatalogRepo, ImportCredit, ImportRecording, RepoError, SessionRepo, UserRepo,
    create_pool, run_migrations, seed_dev_fixture,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

async fn setup_db() -> sqlx::PgPool {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    let pool = create_pool(&db_url)
        .await
        .expect("Failed to connect to database");
    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");
    pool
}

fn hash_token(token: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().to_vec()
}

// 1. UserRepo.create with an existing username in different case returns Conflict.
#[tokio::test]
async fn test_user_create_case_insensitive_conflict() {
    let pool = setup_db().await;
    let repo = UserRepo::new(pool);
    let username = format!("TestUser_{}", Uuid::new_v4().simple());

    let created = repo.create(&username, "hashed_pw_1").await.unwrap();
    assert_eq!(created.username, username);

    let conflict_err = repo.create(&username.to_uppercase(), "hashed_pw_2").await;
    match conflict_err {
        Err(RepoError::Conflict(constraint)) => {
            assert!(
                constraint.contains("users_lower_username_idx") || constraint.contains("unique")
            );
        }
        other => panic!("Expected RepoError::Conflict, got: {:?}", other),
    }
}

// 2. find_by_username matches ignoring case.
#[tokio::test]
async fn test_user_find_by_username_case_insensitive() {
    let pool = setup_db().await;
    let repo = UserRepo::new(pool);
    let base_name = format!("case_user_{}", Uuid::new_v4().simple());

    repo.create(&base_name, "pw").await.unwrap();

    let found_upper = repo
        .find_by_username(&base_name.to_uppercase())
        .await
        .unwrap();
    assert!(found_upper.is_some());
    assert_eq!(found_upper.unwrap().username, base_name);

    let found_lower = repo
        .find_by_username(&base_name.to_lowercase())
        .await
        .unwrap();
    assert!(found_lower.is_some());
}

// 3. SessionRepo.find_valid returns None for an expired session.
#[tokio::test]
async fn test_session_find_valid_expired() {
    let pool = setup_db().await;
    let user_repo = UserRepo::new(pool.clone());
    let session_repo = SessionRepo::new(pool);

    let username = format!("sess_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "pw").await.unwrap();

    let token_hash = hash_token(&format!("expired_token_{}", Uuid::new_v4()));
    let expired_at = Utc::now() - Duration::hours(2);

    session_repo
        .create(user.id, &token_hash, expired_at)
        .await
        .unwrap();

    let valid = session_repo
        .find_valid(&token_hash, Utc::now())
        .await
        .unwrap();
    assert!(valid.is_none(), "Expired session must return None");
}

// 4. SessionRepo.delete on a missing token returns Ok.
#[tokio::test]
async fn test_session_delete_missing_token_returns_ok() {
    let pool = setup_db().await;
    let session_repo = SessionRepo::new(pool);

    let missing_hash = hash_token("non_existent_token_12345");
    let result = session_repo.delete(&missing_hash).await;
    assert!(result.is_ok(), "Deleting missing token must return Ok(())");
}

// 5. Deleting a user deletes their sessions (ON DELETE CASCADE).
#[tokio::test]
async fn test_cascade_delete_user_deletes_sessions() {
    let pool = setup_db().await;
    let user_repo = UserRepo::new(pool.clone());
    let session_repo = SessionRepo::new(pool.clone());

    let username = format!("cascade_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "pw").await.unwrap();

    let token_hash = hash_token(&format!("cascade_token_{}", Uuid::new_v4()));
    let expires_at = Utc::now() + Duration::hours(24);
    session_repo
        .create(user.id, &token_hash, expires_at)
        .await
        .unwrap();

    // Verify session exists
    let valid = session_repo
        .find_valid(&token_hash, Utc::now())
        .await
        .unwrap();
    assert!(valid.is_some());

    // Delete user directly
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();

    // Session should be cascade deleted
    let after_delete = session_repo
        .find_valid(&token_hash, Utc::now())
        .await
        .unwrap();
    assert!(
        after_delete.is_none(),
        "Sessions must be cascade deleted when user is deleted"
    );
}

// 6. search ranks a closer title match above a partial one, and breaks ties by year then id.
#[tokio::test]
async fn test_catalog_search_ranking_and_tie_breaking() {
    let pool = setup_db().await;
    let repo = CatalogRepo::new(pool.clone());

    let id1 = Uuid::parse_str("d1000000-0000-0000-0000-000000000001").unwrap();
    let id2 = Uuid::parse_str("d1000000-0000-0000-0000-000000000002").unwrap();
    let id3 = Uuid::parse_str("d1000000-0000-0000-0000-000000000003").unwrap();
    let id4 = Uuid::parse_str("d1000000-0000-0000-0000-000000000004").unwrap();

    // Exact title "Yesterday", different years & ids
    // id1: "Yesterday", year 1970
    // id2: "Yesterday", year 1965 (earlier year, breaks tie first)
    // id3: "Yesterday Today and Tomorrow", year 1960 (partial match, lower similarity)
    // id4: "Yesterday", year 1965 (same year as id2, id tie-breaker: id2 < id4)
    for (id, title, artist, year) in [
        (id1, "Yesterday", "Artist A", 1970),
        (id2, "Yesterday", "Artist B", 1965),
        (id3, "Yesterday Today and Tomorrow", "Artist C", 1960),
        (id4, "Yesterday", "Artist D", 1965),
    ] {
        sqlx::query(
            "INSERT INTO recordings (id, title, artist_name, year) VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, year = EXCLUDED.year",
        )
        .bind(id)
        .bind(title)
        .bind(artist)
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    }

    let results = repo.search("Yesterday", 10).await.unwrap();
    let ids: Vec<Uuid> = results.into_iter().map(|r| r.id).collect();

    // The 3 exact matches must rank before the partial match id3
    let pos_id2 = ids
        .iter()
        .position(|&id| id == id2)
        .expect("id2 must be in results");
    let pos_id4 = ids
        .iter()
        .position(|&id| id == id4)
        .expect("id4 must be in results");
    let pos_id1 = ids
        .iter()
        .position(|&id| id == id1)
        .expect("id1 must be in results");
    let pos_id3 = ids
        .iter()
        .position(|&id| id == id3)
        .expect("id3 must be in results");

    // id2 and id4 have year 1965 and higher similarity than id3.
    // id2 < id4 by UUID.
    assert!(
        pos_id2 < pos_id4,
        "Tie between id2 and id4 broken by id ASC"
    );
    assert!(
        pos_id2 < pos_id1,
        "Tie broken by year ASC (1965 before 1970)"
    );
    assert!(
        pos_id1 < pos_id3,
        "Closer match must rank above partial match"
    );
}

// 7. get_recording returns None for an unknown id.
#[tokio::test]
async fn test_catalog_get_recording_unknown_id() {
    let pool = setup_db().await;
    let repo = CatalogRepo::new(pool);

    let unknown_id = Uuid::new_v4();
    let res = repo.get_recording(unknown_id).await.unwrap();
    assert!(res.is_none());
}

// 8. get_credit_ids returns each entity once when it has two roles on the same recording.
#[tokio::test]
async fn test_get_credit_ids_deduplication() {
    let pool = setup_db().await;
    let repo = CatalogRepo::new(pool.clone());

    let rec_id = Uuid::new_v4();
    let credit_id = Uuid::new_v4();

    sqlx::query("INSERT INTO credit_entities (id, kind, name) VALUES ($1, 'person', 'Multi Role Artist') ON CONFLICT DO NOTHING")
        .bind(credit_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO recordings (id, title, artist_name, year) VALUES ($1, 'Multi Role Track', 'Multi Role Artist', 2020) ON CONFLICT DO NOTHING")
        .bind(rec_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert two roles for the same credit entity on this recording
    sqlx::query("INSERT INTO recording_credits (recording_id, credit_id, role) VALUES ($1, $2, 'artist') ON CONFLICT DO NOTHING")
        .bind(rec_id)
        .bind(credit_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO recording_credits (recording_id, credit_id, role) VALUES ($1, $2, 'producer') ON CONFLICT DO NOTHING")
        .bind(rec_id)
        .bind(credit_id)
        .execute(&pool)
        .await
        .unwrap();

    let credit_ids = repo.get_credit_ids(rec_id).await.unwrap();
    let count = credit_ids.iter().filter(|&&id| id == credit_id).count();
    assert_eq!(
        count, 1,
        "Credit entity must be returned only once despite two roles"
    );
}

// 9. The dev fixture applies cleanly to a fresh database after the catalog migration.
#[tokio::test]
async fn test_dev_fixture_applies_cleanly() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    // Verify at least 10 recordings are loaded
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count >= 10, "Dev fixture must load 10-20 songs");

    // Verify two recordings have the exact same title ("Hold On")
    let hold_on_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM recordings WHERE title = 'Hold On'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        hold_on_count, 2,
        "Dev fixture must include two recordings with the same title"
    );
}

// 10. Running catalog_import twice on the same input leaves the tables unchanged.
#[tokio::test]
async fn test_catalog_import_idempotency() {
    let pool = setup_db().await;
    let importer = CatalogImporter::new(2);

    let rec_id = Uuid::new_v4();
    let cred1 = Uuid::new_v4();
    let cred2 = Uuid::new_v4();

    let rec = ImportRecording {
        id: rec_id,
        title: "Idempotent Song".to_string(),
        artist_name: "Idempotent Artist".to_string(),
        year: Some(2024),
        release_id: None,
        credits: vec![
            ImportCredit {
                id: cred1,
                kind: "person".to_string(),
                name: "Artist 1".to_string(),
                role: "artist".to_string(),
            },
            ImportCredit {
                id: cred2,
                kind: "label".to_string(),
                name: "Label 1".to_string(),
                role: "label".to_string(),
            },
        ],
    };

    importer
        .import_recordings(&pool, std::slice::from_ref(&rec))
        .await
        .unwrap();

    let rec_count_1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings WHERE id = $1")
        .bind(rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cred_count_1: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM recording_credits WHERE recording_id = $1")
            .bind(rec_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Run import a second time on the same input
    importer.import_recordings(&pool, &[rec]).await.unwrap();

    let rec_count_2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings WHERE id = $1")
        .bind(rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cred_count_2: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM recording_credits WHERE recording_id = $1")
            .bind(rec_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(rec_count_1, 1);
    assert_eq!(
        rec_count_2, 1,
        "Recording count must remain 1 after second import"
    );
    assert_eq!(cred_count_1, 2);
    assert_eq!(
        cred_count_2, 2,
        "Credits count must remain 2 after second import"
    );
}

// 11. Songs with fewer than min_credit_count distinct credit entities are not loaded.
#[tokio::test]
async fn test_catalog_import_drops_songs_below_min_credit_count() {
    let pool = setup_db().await;
    let importer = CatalogImporter::new(2);

    let rec_id = Uuid::new_v4();
    let cred1 = Uuid::new_v4();

    // Only 1 distinct credit entity
    let rec = ImportRecording {
        id: rec_id,
        title: "Lone Credit Song".to_string(),
        artist_name: "Lonely Artist".to_string(),
        year: Some(2023),
        release_id: None,
        credits: vec![
            ImportCredit {
                id: cred1,
                kind: "person".to_string(),
                name: "Lonely Artist".to_string(),
                role: "artist".to_string(),
            },
            ImportCredit {
                id: cred1, // duplicate credit entity id
                kind: "person".to_string(),
                name: "Lonely Artist".to_string(),
                role: "producer".to_string(),
            },
        ],
    };

    importer.import_recordings(&pool, &[rec]).await.unwrap();

    let catalog = CatalogRepo::new(pool);
    let found = catalog.get_recording(rec_id).await.unwrap();
    assert!(
        found.is_none(),
        "Song with fewer than min_credit_count distinct credits must not be loaded"
    );
}

// 12. The coverage report numbers match a direct count of the tables.
#[tokio::test]
async fn test_coverage_report_matches_direct_table_counts() {
    let pool = setup_db().await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await
        .unwrap();

    let report = CatalogImporter::generate_coverage_report_with_conn(&mut tx)
        .await
        .unwrap();

    let direct_rec: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let direct_cred: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_entities")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let direct_shared: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT rc1.recording_id)
        FROM recording_credits rc1
        JOIN recording_credits rc2
            ON rc1.credit_id = rc2.credit_id
            AND rc1.recording_id != rc2.recording_id
        "#,
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(report.total_recordings, direct_rec);
    assert_eq!(report.total_credit_entities, direct_cred);
    assert_eq!(report.recordings_sharing_credits, direct_shared);
}

// MusicBrainz test: marked #[ignore] so CI isn't hitting their API.
#[tokio::test]
#[ignore = "Hits external MusicBrainz API; ignored in CI"]
async fn test_populate_pop_songs_small() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://beat4beat:beat4beat@localhost:5432/beat4beat".to_string());

    let pool = create_pool(&db_url).await?;
    let imported = data_management::populate_pop_songs(&pool, 3).await?;
    assert_eq!(imported, 3, "Expected 3 songs to be imported");
    Ok(())
}
