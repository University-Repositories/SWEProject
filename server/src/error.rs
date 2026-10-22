use crate::api_types::{ApiError, ApiErrorCode, ApiErrorDetail};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

impl ApiError {
    pub fn new(code: ApiErrorCode, message: &str, field: Option<&str>) -> Self {
        Self {
            error: ApiErrorDetail {
                code,
                message: message.into(),
                field: field.map(str::to_owned),
            },
        }
    }

    pub fn validation(field: Option<&str>) -> Self {
        Self::new(ApiErrorCode::ValidationFailed, "Invalid request", field)
    }

    pub fn status(&self) -> StatusCode {
        // map api codes to response status
        match self.error.code {
            ApiErrorCode::ValidationFailed => StatusCode::BAD_REQUEST,
            ApiErrorCode::Unauthenticated | ApiErrorCode::InvalidCredentials => {
                StatusCode::UNAUTHORIZED
            }
            ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
            ApiErrorCode::UsernameTaken => StatusCode::CONFLICT,
            ApiErrorCode::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn internal() -> Self {
        // keep internal details out of api responses
        Self::new(ApiErrorCode::Internal, "Internal server error", None)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status(), Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_error_codes() {
        for (code, expected) in [
            (ApiErrorCode::ValidationFailed, 400),
            (ApiErrorCode::Unauthenticated, 401),
            (ApiErrorCode::InvalidCredentials, 401),
            (ApiErrorCode::NotFound, 404),
            (ApiErrorCode::UsernameTaken, 409),
            (ApiErrorCode::UnsupportedMediaType, 415),
            (ApiErrorCode::Internal, 500),
        ] {
            assert_eq!(
                ApiError::new(code, "Test error", None).status().as_u16(),
                expected
            );
        }
    }

    #[test]
    fn keeps_error_fields_present() {
        let value = serde_json::to_value(ApiError::internal()).unwrap();
        assert_eq!(value["error"]["code"], "INTERNAL");
        assert_eq!(value["error"]["message"], "Internal server error");
        assert!(value["error"].get("field").unwrap().is_null());
        for field in ["username", "password", "q"] {
            let value = serde_json::to_value(ApiError::validation(Some(field))).unwrap();
            assert_eq!(value["error"]["field"], field);
        }
    }
}
