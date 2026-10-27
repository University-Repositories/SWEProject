use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use data_management::CatalogRepo;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    api_types::{
        ApiError, ApiErrorCode, ApiErrorDetail, Credit, CreditKind, CreditRole, RecordingDetail,
        RecordingSummary, SearchResultsResponse,
    },
    app::AppState,
};

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/recordings/{id}", get(get_recording))
        .with_state(state)
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResultsResponse>, (StatusCode, Json<ApiError>)> {
    let query_str = params.q.unwrap_or_default().trim().to_string();
    if query_str.is_empty() {
        return Ok(Json(SearchResultsResponse {
            results: Vec::new(),
        }));
    }

    let limit = params.limit.unwrap_or(20).clamp(1, 100) as i64;
    let repo = CatalogRepo::new(state.pool);

    match repo.search(&query_str, limit).await {
        Ok(rows) => {
            let results = rows
                .into_iter()
                .map(|r| RecordingSummary {
                    id: r.id,
                    title: r.title,
                    artist_name: r.artist_name,
                    year: r.year,
                    release_id: r.release_id,
                })
                .collect();

            Ok(Json(SearchResultsResponse { results }))
        }
        Err(err) => {
            eprintln!("catalog search error: {err:?}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: ApiErrorDetail {
                        code: ApiErrorCode::Internal,
                        message: "Failed to query catalog search".into(),
                        field: None,
                    },
                }),
            ))
        }
    }
}

async fn get_recording(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RecordingDetail>, (StatusCode, Json<ApiError>)> {
    let uuid = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: ApiErrorDetail {
                    code: ApiErrorCode::ValidationFailed,
                    message: "Invalid recording id format".into(),
                    field: Some("id".into()),
                },
            }),
        )
    })?;

    let repo = CatalogRepo::new(state.pool);
    match repo.get_recording(uuid).await {
        Ok(Some(row)) => {
            let credits = row
                .credits
                .into_iter()
                .map(|c| Credit {
                    credit_id: c.credit_id,
                    name: c.name,
                    kind: match c.kind.to_lowercase().as_str() {
                        "person" | "artist" => CreditKind::Person,
                        "label" => CreditKind::Label,
                        "album" | "release" => CreditKind::Album,
                        _ => CreditKind::Person,
                    },
                    role: match c.role.to_lowercase().as_str() {
                        "artist" | "primary_artist" => CreditRole::Artist,
                        "feature" | "featured_artist" => CreditRole::Feature,
                        "producer" => CreditRole::Producer,
                        "writer" | "songwriter" => CreditRole::Writer,
                        "label" => CreditRole::Label,
                        "album" => CreditRole::Album,
                        _ => CreditRole::Artist,
                    },
                })
                .collect();

            Ok(Json(RecordingDetail {
                summary: RecordingSummary {
                    id: row.id,
                    title: row.title,
                    artist_name: row.artist_name,
                    year: row.year,
                    release_id: row.release_id,
                },
                credits,
            }))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: ApiErrorDetail {
                    code: ApiErrorCode::NotFound,
                    message: "Recording not found".into(),
                    field: Some("id".into()),
                },
            }),
        )),
        Err(err) => {
            eprintln!("catalog get_recording error: {err:?}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: ApiErrorDetail {
                        code: ApiErrorCode::Internal,
                        message: "Failed to fetch recording detail".into(),
                        field: None,
                    },
                }),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/beat4beat".into()),
            "SESSION_TTL_SECONDS" => Some("604800".into()),
            "COOKIE_SECURE" => Some("false".into()),
            _ => None,
        })
        .unwrap();
        let pool = PgPoolOptions::new()
            .connect_lazy(&config.database_url)
            .unwrap();
        crate::app::router(AppState { pool, config })
    }

    #[tokio::test]
    async fn empty_search_returns_empty_results() {
        let app = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/catalog/search?q=")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: SearchResultsResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(json.results.is_empty());
    }

    #[tokio::test]
    async fn invalid_uuid_recording_returns_bad_request() {
        let app = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/catalog/recordings/not-a-valid-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], "VALIDATION_FAILED");
        assert_eq!(value["error"]["field"], "id");
    }
}
