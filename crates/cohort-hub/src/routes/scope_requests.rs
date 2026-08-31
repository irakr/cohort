use crate::db::{self, now_rfc3339};
use crate::domain::*;
use crate::error::AppError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use sqlx::Row;

/// A responder asks for more context (or to go live). The stated reason is the
/// value loop: it tells the owner what to look at and where.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ref_): Path<String>,
    Json(req): Json<CreateScopeRequest>,
) -> Result<Json<ScopeRequest>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    if row.status == AssistStatus::Done {
        return Err(AppError::BadRequest("this assist is closed".into()));
    }
    let responders = db::responders_for(&state.pool, &ref_).await?;
    if !responders.iter().any(|u| u.id == viewer.id) {
        return Err(AppError::Forbidden("join the assist before requesting scopes".into()));
    }
    if req.reason.trim().is_empty() {
        return Err(AppError::BadRequest("a reason is required".into()));
    }
    // Comments are conversation, not access: recorded approved immediately.
    let (status, decided_at) = if req.kind == ScopeKind::Comment {
        ("approved", Some(now_rfc3339()))
    } else {
        ("pending", None)
    };
    let result = sqlx::query(
        "INSERT INTO scope_requests (assist_ref, requester_id, kind, target, reason, status, ttl_minutes, created_at, decided_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&ref_)
    .bind(&viewer.id)
    .bind(db::enum_str(&req.kind))
    .bind(&req.target)
    .bind(req.reason.trim())
    .bind(status)
    .bind(req.ttl_minutes)
    .bind(now_rfc3339())
    .bind(&decided_at)
    .execute(&state.pool)
    .await?;
    let id = result.last_insert_rowid();
    Ok(Json(fetch(&state, id).await?))
}

pub async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ScopeRequest>, AppError> {
    decide(state, headers, id, "approved").await
}

pub async fn deny(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ScopeRequest>, AppError> {
    decide(state, headers, id, "denied").await
}

async fn decide(
    state: AppState,
    headers: HeaderMap,
    id: i64,
    status: &str,
) -> Result<Json<ScopeRequest>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let request = fetch(&state, id).await?;
    let row = db::assist_row(&state.pool, &request.assist_ref).await?;
    if row.owner_id != viewer.id {
        return Err(AppError::Forbidden("only the owner decides scope requests".into()));
    }
    if request.status != ScopeStatus::Pending {
        return Err(AppError::BadRequest("this request was already decided".into()));
    }
    sqlx::query("UPDATE scope_requests SET status = ?, decided_at = ? WHERE id = ?")
        .bind(status).bind(now_rfc3339()).bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch(&state, id).await?))
}

async fn fetch(state: &AppState, id: i64) -> Result<ScopeRequest, AppError> {
    let r = sqlx::query(
        "SELECT s.*, u.name AS requester_name FROM scope_requests s
         JOIN users u ON u.id = s.requester_id WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let kind: String = r.get("kind");
    let status: String = r.get("status");
    let kind: ScopeKind = serde_json::from_value(serde_json::Value::String(kind))
        .map_err(|e| AppError::BadRequest(format!("{e}")))?;
    let status: ScopeStatus = serde_json::from_value(serde_json::Value::String(status))
        .map_err(|e| AppError::BadRequest(format!("{e}")))?;
    Ok(ScopeRequest {
        id: r.get("id"),
        assist_ref: r.get("assist_ref"),
        requester_id: r.get("requester_id"),
        requester_name: r.get("requester_name"),
        kind,
        target: r.get("target"),
        reason: r.get("reason"),
        status,
        ttl_minutes: r.get("ttl_minutes"),
        created_at: r.get("created_at"),
        decided_at: r.get("decided_at"),
    })
}
