use axum::{Router, body::Body, http::Request, response::Response};
use cookie::Cookie;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use server::{
    app::{AppState, router},
    config::Config,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

struct TestApp {
    app: Router,
    pool: PgPool,
    admin: PgPool,
    schema: String,
}

impl TestApp {
    async fn new() -> Self {
        let url = std::env::var("ACCOUNT_TEST_DATABASE_URL")
            .expect("set ACCOUNT_TEST_DATABASE_URL to a local test database");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();

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

        let schema = format!("catalog_route_test_{}", Uuid::new_v4().simple());
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

        data_management::run_migrations(&pool).await.unwrap();
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some(url.clone()),
            "SESSION_TTL_SECONDS" => Some("60".into()),
            "COOKIE_SECURE" => Some("true".into()),
            _ => None,
        })
        .unwrap();

        let app = router(AppState {
            pool: pool.clone(),
            config,
        });

        Self {
            app,
            pool,
            admin,
            schema,
        }
    }

    async fn finish(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

async fn login_cookie(app: &Router) -> String {
    let creds = json!({"username": "CatalogTester", "password": "TestPass123!"});
    let reg = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(creds.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status().as_u16(), 201);

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(creds.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status().as_u16(), 204);

    let cookie = Cookie::parse(login.headers()["set-cookie"].to_str().unwrap()).unwrap();
    format!("{}={}", cookie.name(), cookie.value())
}

async fn get_json(app: &Router, uri: &str, cookie: &str) -> (u16, Value) {
    let response: Response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    (status, value)
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn search_enforces_query_char_bounds() {
    let db = TestApp::new().await;
    let cookie = login_cookie(&db.app).await;

    for q in ["a", "%C3%A9", ""] {
        let (status, body) =
            get_json(&db.app, &format!("/api/catalog/search?q={q}"), &cookie).await;
        assert_eq!(status, 200);
        assert_eq!(body["results"], json!([]));
    }

    let too_long = "a".repeat(101);
    let (status, body) = get_json(
        &db.app,
        &format!("/api/catalog/search?q={too_long}"),
        &cookie,
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "VALIDATION_FAILED");
    assert_eq!(body["error"]["field"], "q");

    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn recording_route_handles_missing_and_known_ids() {
    let db = TestApp::new().await;
    data_management::seed_dev_fixture(&db.pool).await.unwrap();
    let cookie = login_cookie(&db.app).await;

    let missing_id = Uuid::new_v4();
    let (status, body) = get_json(
        &db.app,
        &format!("/api/catalog/recordings/{missing_id}"),
        &cookie,
    )
    .await;
    assert_eq!(status, 404);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
    assert_eq!(body["error"]["field"], "id");

    let known_id: Uuid = sqlx::query_scalar("SELECT recording_id FROM recording_credits LIMIT 1")
        .fetch_one(&db.pool)
        .await
        .unwrap();

    let (status, body) = get_json(
        &db.app,
        &format!("/api/catalog/recordings/{known_id}"),
        &cookie,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["summary"]["id"], known_id.to_string());
    let credits = body["credits"].as_array().unwrap();
    assert!(!credits.is_empty());
    assert!(credits[0]["creditId"].is_string());
    assert!(credits[0]["name"].is_string());

    db.finish().await;
}
