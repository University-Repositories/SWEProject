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
        let schema = format!("route_test_{}", Uuid::new_v4().simple());
        // i want route tests isolated from other db tables
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
    send_route(app, method, &format!("/api/auth{path}"), body, cookie).await
}

async fn send_route(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
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

async fn login_player(app: &Router, username: &str) -> String {
    let credentials = input(username, "TestPass123!");
    check(
        send(app, "/register", Some(credentials.clone()), None).await,
        201,
    )
    .await;
    let response = send(app, "/login", Some(credentials), None).await;
    let cookie = session_cookie(&response);
    check(response, 204).await;
    cookie
}

async fn api(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    status: u16,
) -> Value {
    let response = send_route(app, method, path, body, cookie).await;
    assert_eq!(response.status().as_u16(), status, "{method} {path}");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

const OPENING: &str = "8d227b3b-8ea9-42b4-8e54-521b7ce3a241";
const NEXT: &str = "5370d061-f09b-4654-8e43-85f269a9b7ff";
const CREDIT: &str = "b6807869-d355-4422-90f7-ebbe02b11be7";

async fn game_test() -> (TestApp, String) {
    let db = TestApp::new().await;
    data_management::seed_dev_fixture(&db.pool).await.unwrap();
    let cookie = login_player(&db.app, "Player").await;
    (db, cookie)
}

async fn start_game(app: &Router, player: Option<&str>) -> Value {
    api(
        app,
        "POST",
        "/api/games",
        Some(json!({"openingRecordingId": OPENING})),
        player,
        201,
    )
    .await
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn plays_and_ends_a_solo_game_through_the_routes() {
    let (db, cookie) = game_test().await;
    let app = &db.app;
    let player = Some(cookie.as_str());
    let other_cookie = login_player(app, "Reviewer").await;
    let reviewer = Some(other_cookie.as_str());
    let games = "/api/games";
    api(app, "POST", games, Some(json!({})), None, 401).await;
    api(app, "POST", games, None, player, 415).await;
    api(app, "GET", "/api/games/nope", None, player, 400).await;
    api(
        app,
        "GET",
        &format!("{games}/{}", Uuid::nil()),
        None,
        player,
        404,
    )
    .await;
    let game = start_game(app, player).await;
    let game_id = game["id"].as_str().unwrap();
    let game_path = format!("/api/games/{game_id}");
    let moves = format!("{game_path}/moves");
    let attempt = json!({"recordingId": NEXT, "creditId": CREDIT});
    let end = format!("{game_path}/end");
    for (recording, credit, code) in [
        (
            NEXT,
            "5534c03b-c2e8-4a66-b25c-9d62d6ea153d",
            "CREDIT_NOT_ON_PREVIOUS_SONG",
        ),
        (
            "1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb",
            CREDIT,
            "CREDIT_NOT_ON_SUBMITTED_SONG",
        ),
    ] {
        let rejected = api(
            app,
            "POST",
            &moves,
            Some(json!({"recordingId": recording, "creditId": credit})),
            player,
            200,
        )
        .await;
        assert_eq!(rejected["rejection"]["code"], code);
        assert_eq!(rejected["game"], game);
    }
    let result = api(app, "POST", &moves, Some(attempt.clone()), player, 200).await;
    assert_eq!(result["accepted"], true);
    let active = &result["game"];
    let rejected = api(app, "POST", &moves, Some(attempt.clone()), player, 200).await;
    assert_eq!(rejected["rejection"]["code"], "SONG_ALREADY_PLAYED");
    assert_eq!(&rejected["game"], active);
    api(app, "GET", &game_path, None, reviewer, 404).await;
    api(app, "POST", &end, None, reviewer, 404).await;

    let ended = api(app, "POST", &end, None, player, 200).await;
    assert_eq!(ended["status"], "ended");
    assert_eq!(ended["acceptedLinks"], 1);
    assert!(ended["endedAt"].is_string());
    for (path, body) in [(&end, None), (&moves, Some(attempt))] {
        let error = api(app, "POST", path, body, player, 409).await;
        assert_eq!(error["error"]["code"], "GAME_ENDED");
    }
    assert_eq!(api(app, "GET", &game_path, None, player, 200).await, ended);

    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn rejects_a_fourth_use_of_the_same_credit() {
    let (db, cookie) = game_test().await;
    let app = &db.app;
    let player = Some(cookie.as_str());
    let game = start_game(app, player).await;
    let moves = format!("/api/games/{}/moves", game["id"].as_str().unwrap());
    for index in 0..4 {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO recordings (id, title, artist_name) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(format!("Song {index}"))
            .bind("Alabama Shakes")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO recording_credits (recording_id, credit_id, role) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(CREDIT.parse::<Uuid>().unwrap())
        .bind("artist")
        .execute(&db.pool)
        .await
        .unwrap();
        let move_body = json!({"recordingId": id, "creditId": CREDIT});
        let result = api(app, "POST", &moves, Some(move_body), player, 200).await;
        assert_eq!(result["accepted"], index < 3);
        if index == 3 {
            assert_eq!(result["rejection"]["code"], "CREDIT_EXHAUSTED");
            assert_eq!(result["game"]["acceptedLinks"], 3);
        }
    }
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires ACCOUNT_TEST_DATABASE_URL"]
async fn reviews_reports_without_changing_the_game() {
    let (db, cookie) = game_test().await;
    let app = &db.app;
    let player = Some(cookie.as_str());
    let other_cookie = login_player(app, "Reviewer").await;
    let reviewer = Some(other_cookie.as_str());
    let game = start_game(app, player).await;
    let game_id = game["id"].as_str().unwrap();
    let game_path = format!("/api/games/{game_id}");
    let moves = format!("{game_path}/moves");
    let attempt = json!({"recordingId": NEXT, "creditId": CREDIT});
    let result = api(app, "POST", &moves, Some(attempt), player, 200).await;
    let active = &result["game"];
    let move_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM moves WHERE game_id = $1")
        .bind(game_id.parse::<Uuid>().unwrap())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let reports = "/api/reports";
    let report_input = json!({"moveId": move_id, "explanation": "check this link"});
    api(
        app,
        "POST",
        reports,
        Some(report_input.clone()),
        reviewer,
        404,
    )
    .await;
    api(
        app,
        "POST",
        reports,
        Some(json!({"explanation": "missing target"})),
        player,
        400,
    )
    .await;
    api(
        app,
        "POST",
        reports,
        Some(json!({"creditId": CREDIT, "explanation": " "})),
        player,
        400,
    )
    .await;
    let report = api(app, "POST", reports, Some(report_input), player, 201).await;
    let credit_report = api(
        app,
        "POST",
        reports,
        Some(json!({"creditId": CREDIT, "explanation": "credit issue"})),
        player,
        201,
    )
    .await;
    assert_eq!(credit_report["status"], "open");
    let report_id = report["id"].as_str().unwrap();
    let report_path = format!("{reports}/{report_id}");
    api(app, "GET", &report_path, None, reviewer, 404).await;

    let moderation = "/api/moderation/reports";
    let open = format!("{moderation}?status=open");
    api(app, "GET", &open, None, None, 401).await;
    api(app, "GET", &open, None, reviewer, 403).await;
    let mut db_url = std::env::var("ACCOUNT_TEST_DATABASE_URL").unwrap();
    db_url.push(if db_url.contains('?') { '&' } else { '?' });
    db_url.push_str(&format!("options[search_path]={},public", db.schema));
    let grant = std::process::Command::new(env!("CARGO_BIN_EXE_grant_moderator"))
        .arg("Reviewer")
        .env("DATABASE_URL", db_url)
        .output()
        .unwrap();
    assert!(
        grant.status.success(),
        "{}",
        String::from_utf8_lossy(&grant.stderr)
    );
    let me = check(send(app, "/me", None, reviewer).await, 200).await;
    assert_eq!(me["role"], "moderator");
    let listing = api(app, "GET", &open, None, reviewer, 200).await;
    assert_eq!(listing[0]["id"], report_id);
    assert_eq!(listing[0]["gameId"], game_id);
    assert_eq!(listing[0]["creditName"], "Alabama Shakes");
    assert_eq!(listing[0]["reporterUsername"], "Player");
    let resolve = format!("{moderation}/{report_id}/resolve");
    api(
        app,
        "POST",
        &resolve,
        Some(json!({"decision": "upheld"})),
        reviewer,
        400,
    )
    .await;
    let resolution = json!({"decision": "upheld", "reason": "checked the credit"});
    api(app, "POST", &resolve, Some(resolution.clone()), player, 403).await;
    api(
        app,
        "POST",
        &resolve,
        Some(resolution.clone()),
        reviewer,
        200,
    )
    .await;
    let repeat = api(app, "POST", &resolve, Some(resolution), reviewer, 409).await;
    assert_eq!(repeat["error"]["code"], "REPORT_RESOLVED");
    let seen = api(app, "GET", &report_path, None, player, 200).await;
    assert_eq!(seen["status"], "resolved");
    let saved = data_management::ReportRepo::new(db.pool.clone())
        .get(report_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.decision.as_deref(), Some("upheld"));
    assert_eq!(
        saved.resolution_reason.as_deref(),
        Some("checked the credit")
    );
    assert_eq!(saved.reviewer_id.unwrap().to_string(), me["id"]);
    assert!(saved.resolved_at.is_some());
    assert_eq!(
        &api(app, "GET", &game_path, None, player, 200).await,
        active
    );

    db.finish().await;
}
