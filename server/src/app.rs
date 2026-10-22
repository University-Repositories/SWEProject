use crate::{
    api_types::{ApiError, ApiErrorCode},
    config::Config,
};
use axum::{Json, Router, routing::get};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    // process check only no db query
    Json(Health { status: "ok" })
}

async fn not_found() -> ApiError {
    ApiError::new(ApiErrorCode::NotFound, "Route not found", None)
}

pub fn router(state: AppState) -> Router {
    // leave room for auth and catalog routes
    Router::new()
        .nest("/api", Router::new().route("/health", get(health)))
        .fallback(not_found)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn app() -> Router {
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/beat4beat".into()),
            "SESSION_TTL_SECONDS" => Some("604800".into()),
            "COOKIE_SECURE" => Some("false".into()),
            _ => None,
        })
        .unwrap();
        // lazy pool keeps tests independent of db
        let pool = PgPoolOptions::new()
            .connect_lazy(&config.database_url)
            .unwrap();
        router(AppState { pool, config })
    }

    #[tokio::test]
    async fn serves_health_without_account_features() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            serde_json::json!({"status":"ok"})
        );
    }

    #[tokio::test]
    async fn returns_shared_not_found_error() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], "NOT_FOUND");
        assert!(value["error"].get("field").unwrap().is_null());
    }
}
