//! In-memory frame relay for application-window mirroring. The hub keeps
//! ONLY the newest frame per (assist, grant) in memory - never in SQLite -
//! and drops it on revoke, close, delete, or restart ("streamed, not
//! recorded", mirroring plan).

use crate::db;
use crate::domain::*;
use crate::error::AppError;
use crate::{AppState, Frame};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

pub const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

/// The active window grant for this request id, if any.
async fn active_window_grant(
    state: &AppState,
    ref_: &str,
    request_id: i64,
) -> Result<Option<Grant>, AppError> {
    let row = db::assist_row(&state.pool, ref_).await?;
    let requests = db::scope_requests_for(&state.pool, ref_).await?;
    let grants = db::derive_grants(&requests, row.status);
    Ok(grants
        .into_iter()
        .find(|g| g.scope_request_id == request_id && g.kind == ScopeKind::Window))
}

/// Owner uploads the newest frame for a granted window (JPEG bytes).
pub async fn put_frame(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((ref_, request_id)): Path<(String, i64)>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    if row.owner_id != viewer.id {
        return Err(AppError::Forbidden("only the owner streams window frames".into()));
    }
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err(AppError::BadRequest(format!(
            "frame must be 1..{MAX_FRAME_BYTES} bytes"
        )));
    }
    if active_window_grant(&state, &ref_, request_id).await?.is_none() {
        return Err(AppError::BadRequest("no active window grant for this request".into()));
    }
    let mut frames = state.frames.lock().expect("frame store lock");
    frames.insert(
        (ref_, request_id),
        Frame { bytes: body.to_vec(), at: db::now_rfc3339() },
    );
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// Grant holder (or the owner) fetches the newest frame.
pub async fn get_frame(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((ref_, request_id)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    let grant = active_window_grant(&state, &ref_, request_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("no active window grant for this request".into()))?;
    if viewer.id != grant.granted_to_id && viewer.id != row.owner_id {
        return Err(AppError::Forbidden("this window is not granted to you".into()));
    }
    let frame = {
        let frames = state.frames.lock().expect("frame store lock");
        frames.get(&(ref_, request_id)).cloned()
    };
    match frame {
        Some(frame) => Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/jpeg".to_string()),
                (header::CACHE_CONTROL, "no-store".to_string()),
                (header::HeaderName::from_static("x-frame-at"), frame.at),
            ],
            frame.bytes,
        )
            .into_response()),
        None => Err(AppError::NotFound),
    }
}

/// Drop every stored frame for an assist (close/delete).
pub fn clear_assist_frames(state: &AppState, ref_: &str) {
    let mut frames = state.frames.lock().expect("frame store lock");
    frames.retain(|(r, _), _| r != ref_);
}

/// Drop the stored frame for one grant (revoke).
pub fn clear_request_frame(state: &AppState, ref_: &str, request_id: i64) {
    let mut frames = state.frames.lock().expect("frame store lock");
    frames.remove(&(ref_.to_string(), request_id));
}
