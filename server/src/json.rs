use crate::api_types::{ApiError, ApiErrorCode};
use axum::{
    Json,
    extract::{FromRequest, Request, rejection::JsonRejection},
};
use serde::de::DeserializeOwned;

pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // normalize json errors for the api
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|error| match error {
                JsonRejection::MissingJsonContentType(_) => ApiError::new(
                    ApiErrorCode::UnsupportedMediaType,
                    "Use application/json",
                    None,
                ),
                _ => ApiError::validation(None),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::StatusCode, routing::post};
    use http_body_util::BodyExt;
    use serde::Deserialize;
    use tower::ServiceExt;

    #[derive(Deserialize)]
    struct Input {
        username: String,
    }

    async fn accept(ApiJson(input): ApiJson<Input>) -> String {
        input.username
    }

    async fn send(content_type: Option<&str>, body: &str) -> axum::response::Response {
        let mut request = Request::builder().method("POST").uri("/json");
        if let Some(value) = content_type {
            request = request.header("content-type", value);
        }
        Router::new()
            .route("/json", post(accept))
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn accepts_json() {
        let response = send(
            Some("application/json; charset=utf-8"),
            r#"{"username":"Sam"}"#,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "Sam"
        );
    }

    #[tokio::test]
    async fn rejects_missing_or_wrong_media_type() {
        for content_type in [None, Some("text/plain")] {
            let response = send(content_type, r#"{"username":"Sam"}"#).await;
            assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["error"]["code"], "UNSUPPORTED_MEDIA_TYPE");
            assert!(value["error"]["field"].is_null());
        }
    }

    #[tokio::test]
    async fn rejects_invalid_json_or_shape() {
        for body in [r#"{"username":"#, r#"{"username":12}"#, "{}"] {
            let response = send(Some("application/json"), body).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["error"]["code"], "VALIDATION_FAILED");
            assert!(value["error"].get("field").unwrap().is_null());
        }
    }
}
