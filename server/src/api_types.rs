use chrono::{DateTime, Utc};
use game_engine::MoveRejection;
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
    GameEnded,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum MoveRejectionCode {
    SongAlreadyPlayed,
    CreditNotOnPreviousSong,
    CreditNotOnSubmittedSong,
    CreditExhausted,
}

impl From<MoveRejection> for MoveRejectionCode {
    fn from(rejection: MoveRejection) -> Self {
        match rejection {
            MoveRejection::SongAlreadyPlayed => Self::SongAlreadyPlayed,
            MoveRejection::CreditNotOnPreviousSong => Self::CreditNotOnPreviousSong,
            MoveRejection::CreditNotOnSubmittedSong => Self::CreditNotOnSubmittedSong,
            MoveRejection::CreditExhausted => Self::CreditExhausted,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameState {
    pub id: Uuid,
    pub status: GameStatus,
    pub opening: RecordingSummary,
    pub links: Vec<ChainLink>,
    pub credit_uses: Vec<CreditUse>,
    pub accepted_links: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum GameStatus {
    Active,
    Ended,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainLink {
    pub position: u32,
    pub recording: RecordingSummary,
    pub credit: Credit,
    pub uses_after: u8,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreditUse {
    pub credit: Credit,
    pub used: u8,
    pub remaining: u8,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StartGameRequest {
    #[ts(optional)]
    pub opening_recording_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SubmitMoveRequest {
    pub recording_id: Uuid,
    pub credit_id: Uuid,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoveResult {
    pub accepted: bool,
    pub rejection: Option<MoveRejectionDetail>,
    pub game: GameState,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoveRejectionDetail {
    pub code: MoveRejectionCode,
    pub message: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateReportRequest {
    #[ts(optional)]
    pub move_id: Option<Uuid>,
    #[ts(optional)]
    pub credit_id: Option<Uuid>,
    pub explanation: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Report {
    pub id: Uuid,
    pub status: ReportStatus,
    pub move_id: Option<Uuid>,
    pub credit_id: Option<Uuid>,
    pub explanation: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReportStatus {
    Open,
    Resolved,
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

    #[test]
    fn game_requests_use_camel_case_and_allow_optional_ids() {
        let start: StartGameRequest = serde_json::from_str("{}").unwrap();
        assert!(start.opening_recording_id.is_none());

        let id = Uuid::nil();
        let start: StartGameRequest =
            serde_json::from_value(serde_json::json!({ "openingRecordingId": id })).unwrap();
        assert_eq!(start.opening_recording_id, Some(id));

        let report: CreateReportRequest = serde_json::from_value(serde_json::json!({
            "moveId": id,
            "explanation": "wrong credit"
        }))
        .unwrap();
        assert_eq!(report.move_id, Some(id));
        assert!(report.credit_id.is_none());
    }

    #[test]
    fn move_result_serializes_rejection_and_game_state() {
        let result = MoveResult {
            accepted: false,
            rejection: Some(MoveRejectionDetail {
                code: MoveRejectionCode::CreditExhausted,
                message: "that credit has no uses left".into(),
            }),
            game: GameState {
                id: Uuid::nil(),
                status: GameStatus::Active,
                opening: RecordingSummary {
                    id: Uuid::nil(),
                    title: "Opening song".into(),
                    artist_name: "Artist".into(),
                    year: None,
                    release_id: None,
                },
                links: vec![],
                credit_uses: vec![],
                accepted_links: 0,
                started_at: "2026-11-15T12:00:00Z".parse().unwrap(),
                ended_at: None,
            },
        };
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["accepted"], false);
        assert_eq!(value["rejection"]["code"], "CREDIT_EXHAUSTED");
        assert_eq!(value["game"]["status"], "active");
        assert_eq!(value["game"]["acceptedLinks"], 0);
        assert_eq!(value["game"]["startedAt"], "2026-11-15T12:00:00Z");
        assert!(value["game"].get("endedAt").unwrap().is_null());
    }
}
