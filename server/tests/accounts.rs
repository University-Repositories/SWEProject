use axum::{Router, body::Body, http::Request, response::Response};
use cookie::{Cookie, SameSite};
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
        let schema = format!("route_test_{}", Uuid::new_v4().simple());
        // i want route tests isolated from other db tables
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let search_path = format!("SET search_path TO {schema}");
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
        data_management::accounts::migrate(&pool).await.unwrap();
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
        // i want only this route test schema removed from the db
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

async fn send(app: &Router, path: &str, body: Option<Value>, cookie: Option<&str>) -> Response {
    let method = if path == "/me" { "GET" } else { "POST" };
    let mut request = Request::builder()
        .method(method)
        .uri(format!("/api/auth{path}"));
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let body = match body {
        Some(value) => {
            request = request.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}

fn input(username: &str, password: &str) -> Value {
    json!({"username": username, "password": password})
}

async fn check(response: Response, status: u16) -> Value {
    assert_eq!(response.status().as_u16(), status);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    if status == 204 {
        assert!(bytes.is_empty());
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    }
}

fn session_cookie(response: &Response) -> String {
    let cookie = Cookie::parse(response.headers()["set-cookie"].to_str().unwrap()).unwrap();
    assert_eq!(cookie.name(), "b4b_session");
    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.secure(), Some(true));
    assert_eq!(cookie.same_site(), Some(SameSite::Lax));
    assert_eq!(cookie.path(), Some("/"));
    assert_eq!(cookie.max_age().unwrap().whole_seconds(), 60);
    format!("{}={}", cookie.name(), cookie.value())
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn registers_logs_in_reads_user_and_logs_out_through_routes() {
    let db = TestApp::new().await;
    let response = send(
        &db.app,
        "/register",
        Some(input("Sam", "TestPass123!")),
        None,
    )
    .await;
    assert!(!response.headers().contains_key("set-cookie"));
    let user = check(response, 201).await;
    assert_eq!(user["username"], "Sam");
    assert_eq!(user["role"], "player");
    assert!(user["displayName"].is_null());
    assert!(user["createdAt"].is_string());
    assert_eq!(user.as_object().unwrap().len(), 5);
    let response = send(&db.app, "/login", Some(input("Sam", "TestPass123!")), None).await;
    let first_cookie = session_cookie(&response);
    check(response, 204).await;
    let response = send(&db.app, "/login", Some(input("Sam", "TestPass123!")), None).await;
    let second_cookie = session_cookie(&response);
    check(response, 204).await;
    assert!(first_cookie != second_cookie);
    assert_eq!(
        check(send(&db.app, "/me", None, Some(&first_cookie)).await, 200).await,
        user
    );
    let response = send(&db.app, "/logout", None, Some(&first_cookie)).await;
    let cleared = Cookie::parse(response.headers()["set-cookie"].to_str().unwrap()).unwrap();
    assert_eq!(cleared.name(), "b4b_session");
    assert_eq!(cleared.path(), Some("/"));
    assert_eq!(cleared.http_only(), Some(true));
    assert_eq!(cleared.secure(), Some(true));
    assert_eq!(cleared.max_age().unwrap().whole_seconds(), 0);
    check(response, 204).await;
    assert_eq!(
        check(send(&db.app, "/me", None, Some(&first_cookie)).await, 401).await["error"]["code"],
        "UNAUTHENTICATED"
    );
    assert_eq!(
        check(send(&db.app, "/me", None, Some(&second_cookie)).await, 200).await,
        user
    );
    check(
        send(&db.app, "/logout", None, Some(&first_cookie)).await,
        204,
    )
    .await;
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn returns_shared_errors_for_duplicate_names_and_bad_logins() {
    let db = TestApp::new().await;
    check(
        send(
            &db.app,
            "/register",
            Some(input("Sam", "TestPass123!")),
            None,
        )
        .await,
        201,
    )
    .await;
    let value = check(
        send(
            &db.app,
            "/register",
            Some(input("Sam", "OtherPass123!")),
            None,
        )
        .await,
        409,
    )
    .await;
    assert_eq!(value["error"]["code"], "USERNAME_TAKEN");
    assert_eq!(value["error"]["field"], "username");
    for (username, password) in [("Sam", "WrongPass123!"), ("Missing", "TestPass123!")] {
        let response = send(&db.app, "/login", Some(input(username, password)), None).await;
        assert!(!response.headers().contains_key("set-cookie"));
        let value = check(response, 401).await;
        assert_eq!(value["error"]["code"], "INVALID_CREDENTIALS");
        assert_eq!(value["error"]["message"], "Invalid username or password");
        assert!(value["error"]["field"].is_null());
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn rejects_expired_duplicate_and_unknown_sessions_and_hides_db_errors() {
    let db = TestApp::new().await;
    check(
        send(
            &db.app,
            "/register",
            Some(input("Sam", "TestPass123!")),
            None,
        )
        .await,
        201,
    )
    .await;
    let response = send(&db.app, "/login", Some(input("Sam", "TestPass123!")), None).await;
    let cookie = session_cookie(&response);
    check(response, 204).await;
    let duplicate = format!("{cookie}; {cookie}");
    let unknown = format!(
        "b4b_session={}",
        authentication::session::SessionToken::generate()
            .unwrap()
            .as_str()
    );
    for cookie in [&duplicate, &unknown] {
        assert_eq!(
            check(send(&db.app, "/me", None, Some(cookie)).await, 401).await["error"]["code"],
            "UNAUTHENTICATED"
        );
    }
    sqlx::query("UPDATE sessions SET created_at = clock_timestamp() - INTERVAL '2 hours', expires_at = clock_timestamp() - INTERVAL '1 hour'")
        .execute(&db.pool).await.unwrap();
    assert_eq!(
        check(send(&db.app, "/me", None, Some(&cookie)).await, 401).await["error"]["code"],
        "UNAUTHENTICATED"
    );
    check(send(&db.app, "/logout", None, Some(&cookie)).await, 204).await;
    db.pool.close().await;
    for path in ["/register", "/login", "/me", "/logout"] {
        let body = if path == "/register" || path == "/login" {
            Some(input("Sam", "TestPass123!"))
        } else {
            None
        };
        let response = send(&db.app, path, body, Some(&cookie)).await;
        assert!(!response.headers().contains_key("set-cookie"));
        let value = check(response, 500).await;
        assert_eq!(value["error"]["code"], "INTERNAL");
        assert_eq!(value["error"]["message"], "Internal server error");
    }
    db.finish().await;
}
