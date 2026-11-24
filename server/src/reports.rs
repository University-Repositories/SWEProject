use crate::{
    api_types::{ApiError, ApiErrorCode, CreateReportRequest, Report, ReportStatus, User},
    app::AppState,
    auth::{require_moderator, require_session},
    games::request_id,
    json::ApiJson,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
};
use data_management::{ReportRepo, ReportRow};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(create))
        .route("/{id}", get(read))
        .route_layer(middleware::from_fn_with_state(state, require_session))
}

pub fn moderation_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(open_reports))
        .route("/{id}/resolve", post(resolve))
        .route_layer(middleware::from_fn(require_moderator))
        .route_layer(middleware::from_fn_with_state(state, require_session))
}

fn not_found() -> ApiError {
    ApiError::new(ApiErrorCode::NotFound, "Report not found", None)
}

impl From<ReportRow> for Report {
    fn from(row: ReportRow) -> Self {
        Self {
            id: row.id,
            status: if row.status == "resolved" {
                ReportStatus::Resolved
            } else {
                ReportStatus::Open
            },
            move_id: row.move_id,
            credit_id: row.credit_id,
            explanation: row.explanation,
            created_at: row.created_at,
        }
    }
}

async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    ApiJson(input): ApiJson<CreateReportRequest>,
) -> Result<(StatusCode, Json<Report>), ApiError> {
    let row = save_report(&state.pool, user.id, input).await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn save_report(
    pool: &sqlx::PgPool,
    reporter: Uuid,
    input: CreateReportRequest,
) -> Result<ReportRow, ApiError> {
    let explanation = input.explanation.trim();
    if explanation.is_empty() || explanation.chars().count() > 500 {
        return Err(ApiError::validation(Some("explanation")));
    }
    if input.move_id.is_none() && input.credit_id.is_none() {
        return Err(ApiError::validation(Some("moveId")));
    }

    let mut tx = pool.begin().await.map_err(|_| ApiError::internal())?;
    // i want move reports from players who were in that game
    if let Some(id) = input.move_id {
        let found = sqlx::query_scalar::<_, Uuid>(
            "SELECT m.id FROM moves m JOIN game_players p ON p.game_id = m.game_id
             WHERE m.id = $1 AND p.user_id = $2 FOR KEY SHARE OF m, p",
        )
        .bind(id)
        .bind(reporter)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::internal())?;
        if found.is_none() {
            return Err(ApiError::new(
                ApiErrorCode::NotFound,
                "Move not found",
                None,
            ));
        }
    }
    if let Some(id) = input.credit_id {
        let found = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM credit_entities WHERE id = $1 FOR KEY SHARE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::internal())?;
        if found.is_none() {
            return Err(ApiError::validation(Some("creditId")));
        }
    }
    let row = ReportRepo::create_with_executor(
        &mut *tx,
        reporter,
        input.move_id,
        input.credit_id,
        explanation,
    )
    .await
    .map_err(|_| ApiError::internal())?;
    tx.commit().await.map_err(|_| ApiError::internal())?;
    Ok(row)
}

async fn read(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Report>, ApiError> {
    let report = ReportRepo::new(state.pool)
        .get(request_id(&id)?)
        .await
        .map_err(|_| ApiError::internal())?
        .filter(|row| row.reporter_id == user.id)
        .ok_or_else(not_found)?;
    Ok(Json(report.into()))
}

#[derive(Deserialize)]
struct ReportFilter {
    status: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewItem {
    #[serde(flatten)]
    report: Report,
    reporter_username: String,
    game_id: Option<Uuid>,
    recording_id: Option<Uuid>,
    song_title: Option<String>,
    credit_name: Option<String>,
}

async fn open_reports(
    State(state): State<AppState>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<ReviewItem>>, ApiError> {
    if filter
        .status
        .as_deref()
        .is_some_and(|status| status != "open")
    {
        return Err(ApiError::validation(Some("status")));
    }
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<Uuid>,
            Option<Uuid>,
            String,
            chrono::DateTime<chrono::Utc>,
            String,
            Option<Uuid>,
            Option<Uuid>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT r.id, r.move_id, COALESCE(r.credit_id, m.credit_id), r.explanation,
                r.created_at, u.username, m.game_id, m.recording_id, s.title, c.name
         FROM reports r JOIN users u ON u.id = r.reporter_id
         LEFT JOIN moves m ON m.id = r.move_id
         LEFT JOIN recordings s ON s.id = m.recording_id
         LEFT JOIN credit_entities c ON c.id = COALESCE(r.credit_id, m.credit_id)
         WHERE r.status = 'open' ORDER BY r.created_at, r.id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| ApiError::internal())?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    move_id,
                    credit_id,
                    explanation,
                    created_at,
                    reporter_username,
                    game_id,
                    recording_id,
                    song_title,
                    credit_name,
                )| ReviewItem {
                    report: Report {
                        id,
                        status: ReportStatus::Open,
                        move_id,
                        credit_id,
                        explanation,
                        created_at,
                    },
                    reporter_username,
                    game_id,
                    recording_id,
                    song_title,
                    credit_name,
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Decision {
    Upheld,
    Dismissed,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveReport {
    decision: Decision,
    reason: String,
}

async fn resolve(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<ResolveReport>,
) -> Result<Json<Report>, ApiError> {
    let id = request_id(&id)?;
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(ApiError::validation(Some("reason")));
    }
    let decision = match input.decision {
        Decision::Upheld => "upheld",
        Decision::Dismissed => "dismissed",
    };

    // checking status here keeps two mods from resolving the same report
    let row = sqlx::query_as::<_, ReportRow>(
        "UPDATE reports SET status = 'resolved', decision = $2,
             resolution_reason = $3, reviewer_id = $4, resolved_at = now()
         WHERE id = $1 AND status = 'open'
         RETURNING id, reporter_id, move_id, credit_id, explanation, status,
                   resolution_reason, reviewer_id, created_at, resolved_at, decision",
    )
    .bind(id)
    .bind(decision)
    .bind(reason)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| ApiError::internal())?;
    if let Some(row) = row {
        return Ok(Json(row.into()));
    }
    if ReportRepo::new(state.pool)
        .get(id)
        .await
        .map_err(|_| ApiError::internal())?
        .is_none()
    {
        return Err(not_found());
    }
    Err(ApiError::new(
        ApiErrorCode::ReportResolved,
        "Report already resolved",
        None,
    ))
}
