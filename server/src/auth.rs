use crate::{
    api_types::{ApiError, Role, User},
    app::AppState,
    cookies::{clear_session_cookie, read_session_token, session_cookie},
    json::ApiJson,
};
use authentication::service;
use axum::{
    Extension, Json, Router,
    extract::{Request, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, SET_COOKIE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use data_management::accounts::UserRecord;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountInput {
    username: String,
    password: String,
}

impl TryFrom<UserRecord> for User {
    type Error = ApiError;

    fn try_from(record: UserRecord) -> Result<Self, Self::Error> {
        // i want db roles checked before sending public user data
        let role = match record.role.as_str() {
            "player" => Role::Player,
            "moderator" => Role::Moderator,
            _ => return Err(ApiError::internal()),
        };
        Ok(Self {
            id: record.id,
            username: record.username,
            role,
            display_name: record.display_name,
            created_at: record.created_at,
        })
    }
}

async fn register(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<AccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    let user = service::register(&state.pool, &input.username, &input.password).await?;
    // i want acct creation separate from login
    Ok((StatusCode::CREATED, Json(User::try_from(user)?)))
}

async fn login(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<AccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    let session = service::login(
        &state.pool,
        &input.username,
        &input.password,
        state.config.session_ttl,
    )
    .await?;
    // i want the auth token sent only through the cookie
    let cookie = session_cookie(&session.token, &state.config)?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, cookie)]))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let token = read_session_token(&headers)?;
    service::logout(&state.pool, &token).await?;
    // i want the saved auth session removed before clearing the cookie
    let cookie = clear_session_cookie(&state.config)?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, cookie)]))
}

async fn require_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = read_session_token(request.headers())?;
    let user = service::authenticate(&state.pool, &token).await?;
    // i want protected routes to reuse the checked acct data
    request.extensions_mut().insert(User::try_from(user)?);
    Ok(next.run(request).await)
}

async fn me(Extension(user): Extension<User>) -> Json<User> {
    Json(user)
}

async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    // i want auth responses kept out of browser caches
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub fn router(state: AppState) -> Router<AppState> {
    let protected = Router::new()
        .route("/me", get(me))
        .route_layer(middleware::from_fn_with_state(state, require_session));
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .merge(protected)
        .layer(middleware::from_fn(no_store))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app, config::Config};
    use axum::body::Body;
    use http_body_util::BodyExt;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    async fn send(
        path: &str,
        content_type: Option<&str>,
        body: &str,
        cookie: Option<&str>,
    ) -> Response {
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/unused".into()),
            "SESSION_TTL_SECONDS" => Some("60".into()),
            "COOKIE_SECURE" => Some("false".into()),
            _ => None,
        })
        .unwrap();
        let pool = PgPoolOptions::new()
            .connect_lazy(&config.database_url)
            .unwrap();
        let method = if path == "/me" { "GET" } else { "POST" };
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/auth{path}"));
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        app::router(AppState { pool, config })
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap()
    }

    async fn error(response: Response, status: u16, code: &str) -> serde_json::Value {
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(SET_COOKIE));
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], code);
        value
    }

    #[tokio::test]
    async fn rejects_bad_json_before_using_the_db() {
        for path in ["/register", "/login"] {
            for body in [
                "{",
                "{}",
                r#"{"username":"Sam","password":12}"#,
                r#"{"username":"Sam","password":"validpass","role":"moderator"}"#,
            ] {
                error(
                    send(path, Some("application/json"), body, None).await,
                    400,
                    "VALIDATION_FAILED",
                )
                .await;
            }
            error(
                send(path, None, "{}", None).await,
                415,
                "UNSUPPORTED_MEDIA_TYPE",
            )
            .await;
        }
    }

    #[tokio::test]
    async fn maps_validation_errors_on_the_real_routes() {
        for (body, field) in [
            (r#"{"username":"ab","password":"validpass"}"#, "username"),
            (r#"{"username":"Sam","password":"short"}"#, "password"),
        ] {
            let value = error(
                send("/register", Some("application/json"), body, None).await,
                400,
                "VALIDATION_FAILED",
            )
            .await;
            assert_eq!(value["error"]["field"], field);
            error(
                send("/login", Some("application/json"), body, None).await,
                401,
                "INVALID_CREDENTIALS",
            )
            .await;
        }
    }

    #[tokio::test]
    async fn rejects_missing_and_bad_cookies_before_using_the_db() {
        for path in ["/me", "/logout"] {
            for cookie in [None, Some("b4b_session=bad"), Some("theme=dark")] {
                error(send(path, None, "", cookie).await, 401, "UNAUTHENTICATED").await;
            }
        }
    }

    #[test]
    fn converts_only_known_db_roles() {
        for role in ["player", "moderator", "admin"] {
            let user = User::try_from(UserRecord {
                id: uuid::Uuid::nil(),
                username: "Sam".into(),
                role: role.into(),
                display_name: None,
                created_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            });
            if role == "admin" {
                assert_eq!(
                    user.unwrap_err().status(),
                    StatusCode::INTERNAL_SERVER_ERROR
                );
            } else {
                assert_eq!(serde_json::to_value(user.unwrap()).unwrap()["role"], role);
            }
        }
    }
}
