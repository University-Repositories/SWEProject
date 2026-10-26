use authentication::{AccountError, service};
use data_management::{UserRepo, run_migrations};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

struct TestDb {
    pool: PgPool,
    admin: PgPool,
    schema: String,
}

impl TestDb {
    async fn new() -> Self {
        let url = std::env::var("ACCOUNT_TEST_DATABASE_URL")
            .expect("set ACCOUNT_TEST_DATABASE_URL to a local test database");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        // i want pg_trgm in public so every test schema can use it
        let mut setup = admin.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(4242)")
            .execute(&mut *setup)
            .await
            .unwrap();
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public")
            .execute(&mut *setup)
            .await
            .unwrap();
        setup.commit().await.unwrap();
        let schema = format!("account_test_{}", Uuid::new_v4().simple());
        // i want each db test in its own schema
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let search_path = format!("SET search_path TO {schema}, public");
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .after_connect(move |connection, _| {
                let search_path = search_path.clone();
                Box::pin(async move {
                    sqlx::query(&search_path).execute(connection).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        Self {
            pool,
            admin,
            schema,
        }
    }

    async fn finish(self) {
        self.pool.close().await;
        // i want only this test schema removed from the db
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn saves_accounts_and_checks_login_credentials() {
    let db = TestDb::new().await;
    run_migrations(&db.pool).await.unwrap();
    let user = service::register(&db.pool, "Sam", "TestPass123!")
        .await
        .unwrap();
    assert_eq!(user.username, "Sam");
    assert_eq!(user.role, "player");
    assert!(user.display_name.is_none());
    let saved = UserRepo::new(db.pool.clone())
        .find_by_username("Sam")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.id, user.id);
    assert!(saved.password_hash.starts_with("$argon2id$"));
    assert!(!saved.password_hash.contains("TestPass123!"));
    assert!(matches!(
        service::register(&db.pool, "Sam", "OtherPass123!").await,
        Err(AccountError::UsernameTaken)
    ));
    assert!(matches!(
        service::register(&db.pool, "sam", "OtherPass123!").await,
        Err(AccountError::UsernameTaken)
    ));
    for (name, password) in [("Sam", "WrongPass123!"), ("Nobody", "TestPass123!")] {
        assert!(matches!(
            service::login(&db.pool, name, password, Duration::from_secs(60)).await,
            Err(AccountError::InvalidCredentials)
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let session = service::login(&db.pool, "Sam", "TestPass123!", Duration::from_secs(60))
        .await
        .unwrap();
    let current = service::authenticate(&db.pool, session.token.as_str())
        .await
        .unwrap();
    assert_eq!(current.id, user.id);
    let stored: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(stored, session.token.hash());
    let expiry: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT expires_at FROM sessions")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(
        (expiry - session.expires_at)
            .num_microseconds()
            .unwrap()
            .abs()
            < 2
    );
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn expires_and_revokes_only_the_selected_session() {
    let db = TestDb::new().await;
    service::register(&db.pool, "Sam", "TestPass123!")
        .await
        .unwrap();
    let first = service::login(&db.pool, "Sam", "TestPass123!", Duration::from_secs(60))
        .await
        .unwrap();
    let second = service::login(&db.pool, "Sam", "TestPass123!", Duration::from_secs(60))
        .await
        .unwrap();
    service::logout(&db.pool, first.token.as_str())
        .await
        .unwrap();
    service::logout(&db.pool, first.token.as_str())
        .await
        .unwrap();
    assert!(matches!(
        service::authenticate(&db.pool, first.token.as_str()).await,
        Err(AccountError::SessionInvalid)
    ));
    assert!(
        service::authenticate(&db.pool, second.token.as_str())
            .await
            .is_ok()
    );
    sqlx::query("UPDATE sessions SET created_at = clock_timestamp() - INTERVAL '2 hours', expires_at = clock_timestamp() - INTERVAL '1 hour'")
        .execute(&db.pool).await.unwrap();
    assert!(matches!(
        service::authenticate(&db.pool, second.token.as_str()).await,
        Err(AccountError::SessionInvalid)
    ));
    let unknown = authentication::session::SessionToken::generate().unwrap();
    assert!(matches!(
        service::authenticate(&db.pool, unknown.as_str()).await,
        Err(AccountError::SessionInvalid)
    ));
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn handles_racing_registration_and_user_deletion() {
    let db = TestDb::new().await;
    let (first, second) = tokio::join!(
        service::register(&db.pool, "Sam", "TestPass123!"),
        service::register(&db.pool, "Sam", "TestPass123!")
    );
    let user = match (first, second) {
        (Ok(user), Err(AccountError::UsernameTaken))
        | (Err(AccountError::UsernameTaken), Ok(user)) => user,
        _ => panic!("expected one account and one username conflict"),
    };
    let session = service::login(&db.pool, "Sam", "TestPass123!", Duration::from_secs(60))
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(
        service::authenticate(&db.pool, session.token.as_str()).await,
        Err(AccountError::SessionInvalid)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn rejects_invalid_session_lifetime_and_hides_db_failures() {
    let db = TestDb::new().await;
    service::register(&db.pool, "Sam", "TestPass123!")
        .await
        .unwrap();
    for ttl in [Duration::ZERO, Duration::MAX] {
        assert!(matches!(
            service::login(&db.pool, "Sam", "TestPass123!", ttl).await,
            Err(AccountError::Internal)
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let token = authentication::session::SessionToken::generate().unwrap();
    db.pool.close().await;
    assert!(matches!(
        service::register(&db.pool, "Other", "TestPass123!").await,
        Err(AccountError::Internal)
    ));
    assert!(matches!(
        service::login(&db.pool, "Sam", "TestPass123!", Duration::from_secs(60)).await,
        Err(AccountError::Internal)
    ));
    assert!(matches!(
        service::authenticate(&db.pool, token.as_str()).await,
        Err(AccountError::Internal)
    ));
    assert_eq!(
        service::logout(&db.pool, token.as_str()).await,
        Err(AccountError::Internal)
    );
    db.finish().await;
}
