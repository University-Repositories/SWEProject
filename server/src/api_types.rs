use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Role {
    Player,
    Moderator,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub role: Role,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ApiErrorCode {
    ValidationFailed,
    Unauthenticated,
    InvalidCredentials,
    NotFound,
    UsernameTaken,
    UnsupportedMediaType,
    Internal,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiErrorDetail {
    pub code: ApiErrorCode,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

// --- Catalog Wire Types (PBI 07) ---

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSummary {
    pub id: Uuid,
    pub title: String,
    pub artist_name: String,
    pub year: Option<i32>,
    pub release_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDetail {
    #[serde(flatten)]
    pub summary: RecordingSummary,
    pub credits: Vec<Credit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub credit_id: Uuid,
    pub name: String,
    pub kind: CreditKind,
    pub role: CreditRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CreditKind {
    Person,
    Label,
    Album,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CreditRole {
    Artist,
    Feature,
    Producer,
    Writer,
    Label,
    Album,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsResponse {
    pub results: Vec<RecordingSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_user_contract() {
        let user = User {
            id: Uuid::nil(),
            username: "Sam".into(),
            role: Role::Player,
            display_name: None,
            created_at: "2026-10-22T12:00:00Z".parse().unwrap(),
        };
        let value = serde_json::to_value(user).unwrap();
        assert_eq!(value["role"], "player");
        assert_eq!(value["username"], "Sam");
        assert_eq!(value["id"], Uuid::nil().to_string());
        assert_eq!(value["createdAt"], "2026-10-22T12:00:00Z");
        assert!(value.get("displayName").unwrap().is_null());
        assert_eq!(value.as_object().unwrap().len(), 5);
    }

    #[test]
    fn trigger_catalog_ts_export() {
        let _ = std::mem::size_of::<RecordingSummary>();
        let _ = std::mem::size_of::<RecordingDetail>();
        let _ = std::mem::size_of::<Credit>();
        let _ = std::mem::size_of::<CreditKind>();
        let _ = std::mem::size_of::<CreditRole>();
        let _ = std::mem::size_of::<SearchResultsResponse>();
    }
}
