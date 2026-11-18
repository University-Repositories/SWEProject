use chrono::{Duration, Utc};
use data_management::{
    CatalogImporter, CatalogRepo, GameRepo, ImportCredit, ImportRecording, MatchRepo, RepoError,
    ReportRepo, SessionRepo, UserRepo, create_pool, run_migrations, seed_dev_fixture,
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

// search returns an empty list for a word that is not in the catalog.
#[tokio::test]
async fn test_catalog_search_unrelated_word_returns_empty_list() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();
    let repo = CatalogRepo::new(pool.clone());

    // Verify there are recordings in the database so the empty result is due to filtering
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recordings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        count > 0,
        "Catalog must contain songs for this test to be meaningful"
    );

    let results = repo.search("nonexistentword", 10).await.unwrap();
    assert!(
        results.is_empty(),
        "A word that is not in the catalog must return an empty list"
    );
}

// search matches misspelled titles using trigram similarity (e.g. 'bohemian rapsody').
#[tokio::test]
async fn test_catalog_search_misspelled_title_matches() {
    let pool = setup_db().await;
    let mut tx = pool.begin().await.unwrap();

    let rec_id = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings (id, title, artist_name, year) VALUES ($1, $2, $3, $4)")
        .bind(rec_id)
        .bind("Bohemian Rhapsody")
        .bind("Queen")
        .bind(1975)
        .execute(&mut *tx)
        .await
        .unwrap();

    let results = CatalogRepo::search_with_executor(&mut *tx, "bohemian rapsody", 10)
        .await
        .unwrap();
    assert!(
        results.iter().any(|r| r.id == rec_id),
        "Expected Bohemian Rhapsody in search results for 'bohemian rapsody'"
    );

    // Partial word search also matches
    let results_partial = CatalogRepo::search_with_executor(&mut *tx, "bohem", 10)
        .await
        .unwrap();
    assert!(
        results_partial.iter().any(|r| r.id == rec_id),
        "Expected Bohemian Rhapsody in search results for 'bohem'"
    );

    // Misspelled artist name also matches
    let results_artist = CatalogRepo::search_with_executor(&mut *tx, "queeen", 10)
        .await
        .unwrap();
    assert!(
        results_artist.iter().any(|r| r.id == rec_id),
        "Expected Queen in search results for 'queeen'"
    );

    tx.rollback().await.unwrap();
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

// 13. GameRepo.create_solo creates game and player row in a single transaction,
// and get retrieves it.
#[tokio::test]
async fn test_game_create_solo_and_get() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    let user_repo = UserRepo::new(pool.clone());
    let catalog = CatalogRepo::new(pool.clone());
    let game_repo = GameRepo::new(pool.clone());

    let username = format!("game_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "password").await.unwrap();

    let opening_rec_id = catalog
        .random_playable_recording()
        .await
        .unwrap()
        .expect("Dev fixture must contain playable recordings");

    let game = game_repo
        .create_solo(user.id, opening_rec_id)
        .await
        .unwrap();

    assert_eq!(game.mode, "solo");
    assert_eq!(game.status, "active");
    assert_eq!(game.opening_recording_id, opening_rec_id);
    assert!(game.accepted_links.is_none());
    assert!(game.winner_user_id.is_none());
    assert!(game.ended_at.is_none());

    // Verify player row created in the same transaction
    let players = game_repo.get_players(game.id).await.unwrap();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].user_id, user.id);
    assert_eq!(players[0].game_id, game.id);
    assert!(players[0].lives_remaining.is_none());

    // Verify get
    let fetched = game_repo.get(game.id).await.unwrap();
    assert_eq!(fetched, Some(game));
}

// 14. GameRepo.add_move records moves and list_moves returns them in turn order.
#[tokio::test]
async fn test_game_add_move_and_list_moves() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    let user_repo = UserRepo::new(pool.clone());
    let catalog = CatalogRepo::new(pool.clone());
    let game_repo = GameRepo::new(pool.clone());

    let username = format!("move_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "password").await.unwrap();

    let opening_rec_id = catalog.random_playable_recording().await.unwrap().unwrap();
    let game = game_repo
        .create_solo(user.id, opening_rec_id)
        .await
        .unwrap();

    let rec_id = Uuid::new_v4();
    let credit_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO credit_entities (id, kind, name) VALUES ($1, 'person', 'Test Artist')",
    )
    .bind(credit_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO recordings (id, title, artist_name, year)
        VALUES ($1, 'Test Song', 'Test Artist', 2020)
        "#,
    )
    .bind(rec_id)
    .execute(&pool)
    .await
    .unwrap();

    let move1 = game_repo
        .add_move(game.id, user.id, 1, rec_id, credit_id)
        .await
        .unwrap();
    assert_eq!(move1.game_id, game.id);
    assert_eq!(move1.user_id, user.id);
    assert_eq!(move1.turn_number, 1);
    assert_eq!(move1.recording_id, rec_id);
    assert_eq!(move1.credit_id, credit_id);

    let move2 = game_repo
        .add_move(game.id, user.id, 2, rec_id, credit_id)
        .await
        .unwrap();
    assert_eq!(move2.turn_number, 2);

    let moves = game_repo.list_moves(game.id).await.unwrap();
    assert_eq!(moves.len(), 2);
    assert_eq!(moves[0].id, move1.id);
    assert_eq!(moves[1].id, move2.id);
}

// 15. GameRepo.add_move returns Conflict on duplicate turn number for the same game.
#[tokio::test]
async fn test_game_add_move_duplicate_turn_conflict() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    let user_repo = UserRepo::new(pool.clone());
    let catalog = CatalogRepo::new(pool.clone());
    let game_repo = GameRepo::new(pool.clone());

    let username = format!("dup_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "password").await.unwrap();

    let opening_rec_id = catalog.random_playable_recording().await.unwrap().unwrap();
    let game = game_repo
        .create_solo(user.id, opening_rec_id)
        .await
        .unwrap();

    let rec_id = Uuid::new_v4();
    let credit_id = Uuid::new_v4();
    sqlx::query("INSERT INTO credit_entities (id, kind, name) VALUES ($1, 'person', 'Dup Artist')")
        .bind(credit_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO recordings (id, title, artist_name, year)
        VALUES ($1, 'Dup Song', 'Dup Artist', 2021)
        "#,
    )
    .bind(rec_id)
    .execute(&pool)
    .await
    .unwrap();

    game_repo
        .add_move(game.id, user.id, 1, rec_id, credit_id)
        .await
        .unwrap();

    let dup_err = game_repo
        .add_move(game.id, user.id, 1, rec_id, credit_id)
        .await;

    match dup_err {
        Err(RepoError::Conflict(constraint)) => {
            assert!(
                constraint.contains("moves_game_id_turn_number_key")
                    || constraint.contains("unique"),
                "Expected conflict on (game_id, turn_number), got: {constraint}"
            );
        }
        other => panic!(
            "Expected RepoError::Conflict on duplicate turn, got: {:?}",
            other
        ),
    }
}

// 16. GameRepo.end updates only an active game, reports true, and a second end changes nothing.
#[tokio::test]
async fn test_game_end_active_and_second_end_changes_nothing() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    let user_repo = UserRepo::new(pool.clone());
    let catalog = CatalogRepo::new(pool.clone());
    let game_repo = GameRepo::new(pool.clone());

    let username = format!("end_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "password").await.unwrap();

    let opening_rec_id = catalog.random_playable_recording().await.unwrap().unwrap();
    let game = game_repo
        .create_solo(user.id, opening_rec_id)
        .await
        .unwrap();

    // First end: updates active game and reports true
    let updated = game_repo.end(game.id, 7).await.unwrap();
    assert!(updated, "First end on active game must return true");

    let ended_game = game_repo.get(game.id).await.unwrap().unwrap();
    assert_eq!(ended_game.status, "ended");
    assert_eq!(ended_game.accepted_links, Some(7));
    assert!(ended_game.ended_at.is_some());

    // Second end: already ended, returns false and changes nothing
    let updated_second = game_repo.end(game.id, 14).await.unwrap();
    assert!(
        !updated_second,
        "Second end on already ended game must return false"
    );

    let game_after_second = game_repo.get(game.id).await.unwrap().unwrap();
    assert_eq!(
        game_after_second.accepted_links,
        Some(7),
        "Second end must not overwrite accepted_links"
    );
    assert_eq!(
        game_after_second.ended_at, ended_game.ended_at,
        "Second end must not overwrite ended_at"
    );
}

// 17. ReportRepo.create and get store and retrieve link and credit reports.
#[tokio::test]
async fn test_report_create_and_get() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();

    let user_repo = UserRepo::new(pool.clone());
    let catalog = CatalogRepo::new(pool.clone());
    let game_repo = GameRepo::new(pool.clone());
    let report_repo = ReportRepo::new(pool.clone());

    let username = format!("rep_user_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "password").await.unwrap();

    let opening_rec_id = catalog.random_playable_recording().await.unwrap().unwrap();
    let game = game_repo
        .create_solo(user.id, opening_rec_id)
        .await
        .unwrap();

    let rec_id = Uuid::new_v4();
    let credit_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO credit_entities (id, kind, name) VALUES ($1, 'person', 'Report Artist')",
    )
    .bind(credit_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO recordings (id, title, artist_name, year)
        VALUES ($1, 'Report Song', 'Report Artist', 2022)
        "#,
    )
    .bind(rec_id)
    .execute(&pool)
    .await
    .unwrap();

    let move_row = game_repo
        .add_move(game.id, user.id, 1, rec_id, credit_id)
        .await
        .unwrap();

    // 1. Report targeting a move
    let report_move = report_repo
        .create(
            user.id,
            Some(move_row.id),
            None,
            "Invalid song link reported",
        )
        .await
        .unwrap();
    assert_eq!(report_move.reporter_id, user.id);
    assert_eq!(report_move.move_id, Some(move_row.id));
    assert_eq!(report_move.credit_id, None);
    assert_eq!(report_move.explanation, "Invalid song link reported");
    assert_eq!(report_move.status, "open");
    assert!(report_move.resolution_reason.is_none());
    assert!(report_move.reviewer_id.is_none());
    assert!(report_move.resolved_at.is_none());

    let fetched = report_repo.get(report_move.id).await.unwrap();
    assert_eq!(fetched, Some(report_move));

    // 2. Report targeting a credit entity
    let report_credit = report_repo
        .create(
            user.id,
            None,
            Some(credit_id),
            "Misattributed credit entity",
        )
        .await
        .unwrap();
    assert_eq!(report_credit.move_id, None);
    assert_eq!(report_credit.credit_id, Some(credit_id));

    let fetched_credit = report_repo.get(report_credit.id).await.unwrap();
    assert_eq!(fetched_credit, Some(report_credit));
}

// 18. CatalogRepo.random_playable_recording selects a recording that shares credits.
#[tokio::test]
async fn test_catalog_random_playable_recording() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();
    let catalog = CatalogRepo::new(pool.clone());

    let rec_id = catalog
        .random_playable_recording()
        .await
        .unwrap()
        .expect("Dev fixture must contain playable recordings");

    // Verify it shares at least one credit with another recording
    let shared_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT rc2.recording_id)
        FROM recording_credits rc1
        JOIN recording_credits rc2
            ON rc1.credit_id = rc2.credit_id
            AND rc1.recording_id != rc2.recording_id
        WHERE rc1.recording_id = $1
        "#,
    )
    .bind(rec_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        shared_count >= 1,
        "Random playable recording must share at least 1 credit with another recording"
    );
}

// 19. MatchRepo.save_match with an old version changes nothing and returns false.
#[tokio::test]
async fn test_match_save_with_old_version_changes_nothing_and_returns_false() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();
    let user_repo = UserRepo::new(pool.clone());
    let match_repo = MatchRepo::new(pool.clone());

    let u1 = user_repo
        .create(&format!("VTest1_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();
    let u2 = user_repo
        .create(&format!("VTest2_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();

    let opening_rec_id: Uuid = sqlx::query_scalar("SELECT id FROM recordings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let code = format!("INV_{}", Uuid::new_v4().simple());
    let match_game = match_repo
        .create_waiting_match(u1.id, opening_rec_id, &code)
        .await
        .unwrap();
    assert_eq!(match_game.status, "waiting");
    assert_eq!(match_game.version, 0);

    // Second player joins -> version becomes 1, status becomes active
    let joined = match_repo
        .add_second_player(match_game.id, u2.id)
        .await
        .unwrap();
    assert!(joined);

    let loaded = match_repo.load_match(match_game.id).await.unwrap().unwrap();
    assert_eq!(loaded.game.status, "active");
    assert_eq!(loaded.game.version, 1);
    assert_eq!(loaded.players.len(), 2);

    // Save with expected_version = 1 succeeds, increments version to 2
    let saved = match_repo
        .save_match(
            match_game.id,
            1,
            "active",
            Some(u1.id),
            2,
            None,
            Some(1),
            None,
        )
        .await
        .unwrap();
    assert!(saved, "Save with matching version must succeed");

    let updated = match_repo.load_match(match_game.id).await.unwrap().unwrap();
    assert_eq!(updated.game.version, 2);
    assert_eq!(updated.game.turn_number, 2);

    // Save with stale expected_version = 1 fails and changes nothing
    let stale_saved = match_repo
        .save_match(
            match_game.id,
            1,
            "ended",
            Some(u2.id),
            99,
            None,
            Some(50),
            Some(u2.id),
        )
        .await
        .unwrap();
    assert!(!stale_saved, "Save with old version must return false");

    // Verify database state is untouched
    let unchanged = match_repo.load_match(match_game.id).await.unwrap().unwrap();
    assert_eq!(unchanged.game.version, 2, "Version must remain at 2");
    assert_eq!(unchanged.game.status, "active", "Status must not change");
    assert_eq!(unchanged.game.turn_number, 2, "Turn number must not change");
}

// 20. Match invite codes are unique and return Conflict on duplicate.
#[tokio::test]
async fn test_match_invite_codes_are_unique() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();
    let user_repo = UserRepo::new(pool.clone());
    let match_repo = MatchRepo::new(pool.clone());

    let u1 = user_repo
        .create(&format!("CodeUser1_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();
    let u2 = user_repo
        .create(&format!("CodeUser2_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();

    let opening_rec_id: Uuid = sqlx::query_scalar("SELECT id FROM recordings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let code = format!("UNIQUE_{}", Uuid::new_v4().simple());
    let first = match_repo
        .create_waiting_match(u1.id, opening_rec_id, &code)
        .await
        .unwrap();
    assert_eq!(first.invite_code.as_deref(), Some(code.as_str()));

    // Finding by invite code works
    let found = match_repo.find_by_invite_code(&code).await.unwrap();
    assert_eq!(found.map(|g| g.id), Some(first.id));

    // Duplicate invite code returns Conflict
    let duplicate_res = match_repo
        .create_waiting_match(u2.id, opening_rec_id, &code)
        .await;
    match duplicate_res {
        Err(RepoError::Conflict(_)) => {}
        other => panic!(
            "Expected Conflict on duplicate invite code, got: {:?}",
            other
        ),
    }
}

// 21. MatchRepo.find_user_waiting_or_active_match finds a user's match.
#[tokio::test]
async fn test_find_user_waiting_or_active_match() {
    let pool = setup_db().await;
    seed_dev_fixture(&pool).await.unwrap();
    let user_repo = UserRepo::new(pool.clone());
    let match_repo = MatchRepo::new(pool.clone());

    let u1 = user_repo
        .create(&format!("ActiveMatch1_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();
    let u2 = user_repo
        .create(&format!("ActiveMatch2_{}", Uuid::new_v4().simple()), "pw")
        .await
        .unwrap();

    let opening_rec_id: Uuid = sqlx::query_scalar("SELECT id FROM recordings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Initially no match
    let none_found = match_repo
        .find_user_waiting_or_active_match(u1.id)
        .await
        .unwrap();
    assert!(none_found.is_none());

    // Create waiting match -> found
    let code = format!("CODE_{}", Uuid::new_v4().simple());
    let m = match_repo
        .create_waiting_match(u1.id, opening_rec_id, &code)
        .await
        .unwrap();
    let waiting_found = match_repo
        .find_user_waiting_or_active_match(u1.id)
        .await
        .unwrap();
    assert_eq!(waiting_found.map(|g| g.id), Some(m.id));

    // Second player joins -> active match found for both users
    match_repo.add_second_player(m.id, u2.id).await.unwrap();
    let active_u1 = match_repo
        .find_user_waiting_or_active_match(u1.id)
        .await
        .unwrap();
    assert_eq!(active_u1.map(|g| g.id), Some(m.id));

    let active_u2 = match_repo
        .find_user_waiting_or_active_match(u2.id)
        .await
        .unwrap();
    assert_eq!(active_u2.map(|g| g.id), Some(m.id));

    // End match -> neither finds it
    match_repo
        .save_match(m.id, 1, "ended", None, 1, None, Some(0), Some(u1.id))
        .await
        .unwrap();
    let ended_u1 = match_repo
        .find_user_waiting_or_active_match(u1.id)
        .await
        .unwrap();
    assert!(ended_u1.is_none(), "Ended match must not be returned");
}

// 22. UserRepo: get/set role and update display_name and bio.
#[tokio::test]
async fn test_user_repo_roles_display_name_and_bio() {
    let pool = setup_db().await;
    let user_repo = UserRepo::new(pool.clone());

    let username = format!("ProfileUser_{}", Uuid::new_v4().simple());
    let user = user_repo.create(&username, "pw").await.unwrap();
    assert_eq!(user.role, "player");
    assert_eq!(user.display_name, None);
    assert_eq!(user.bio, None);

    // Get role
    let role = user_repo.get_role(user.id).await.unwrap();
    assert_eq!(role.as_deref(), Some("player"));

    // Set role to moderator
    let updated_role = user_repo.set_role(user.id, "moderator").await.unwrap();
    assert!(updated_role);
    let role = user_repo.get_role(user.id).await.unwrap();
    assert_eq!(role.as_deref(), Some("moderator"));

    // Update display name and bio
    let updated_profile = user_repo
        .update_profile(user.id, Some("DJ Beat"), Some("Music lover & trivia fan"))
        .await
        .unwrap();
    assert!(updated_profile);

    let fetched = user_repo.find_by_id(user.id).await.unwrap().unwrap();
    assert_eq!(fetched.display_name.as_deref(), Some("DJ Beat"));
    assert_eq!(fetched.bio.as_deref(), Some("Music lover & trivia fan"));
    assert_eq!(fetched.role, "moderator");
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
