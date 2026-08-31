use crate::db;
use crate::domain::*;
use crate::error::AppError;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

#[derive(Debug, Deserialize)]
pub struct NotificationParams {
    /// RFC3339 cursor from the previous response's `now`. Absent on the first
    /// poll: the client bootstraps its cursor and gets no backlog.
    pub since: Option<String>,
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "live_debug" => "live debug",
        "file" => "file access",
        "terminal" => "terminal stream",
        "agents" => "agents view",
        "ssh" => "device access",
        _ => "scope",
    }
}

/// Events relevant to the current user since the cursor, derived from the
/// existing tables (no notification storage): requests and comments on assists
/// they own, decisions on requests they made, responders joining their
/// assists, and credits they received.
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<NotificationParams>,
) -> Result<Json<NotificationsResponse>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let now = db::now_rfc3339();
    let since = match params.since {
        Some(s) if !s.is_empty() => s,
        _ => {
            return Ok(Json(NotificationsResponse { now, notifications: vec![] }));
        }
    };
    let mut notifications: Vec<HubNotification> = Vec::new();

    // Requests (and comments) created on assists the viewer owns.
    let rows = sqlx::query(
        "SELECT s.id, s.kind, s.target, s.reason, s.created_at, s.assist_ref,
                a.title, u.name AS requester_name
         FROM scope_requests s
         JOIN assists a ON a.ref = s.assist_ref
         JOIN users u ON u.id = s.requester_id
         WHERE a.owner_id = ? AND s.requester_id != ? AND s.created_at >= ?",
    )
    .bind(&viewer.id)
    .bind(&viewer.id)
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    for r in rows {
        let kind: String = r.get("kind");
        let requester: String = r.get("requester_name");
        let reason: String = r.get("reason");
        let target: Option<String> = r.get("target");
        let message = if kind == "comment" {
            format!("{requester} commented: {reason}")
        } else {
            let what = match &target {
                Some(t) => format!("{} ({t})", kind_label(&kind)),
                None => kind_label(&kind).to_string(),
            };
            format!("{requester} requests {what}: {reason}")
        };
        notifications.push(HubNotification {
            id: format!("req-{}-created", r.get::<i64, _>("id")),
            kind: if kind == "comment" { "comment".into() } else { "scope_requested".into() },
            assist_ref: r.get("assist_ref"),
            assist_title: r.get("title"),
            actor_name: requester,
            message,
            at: r.get("created_at"),
        });
    }

    // Decisions on requests the viewer made (comments auto-approve; skip them).
    let rows = sqlx::query(
        "SELECT s.id, s.kind, s.target, s.status, s.decided_at, s.assist_ref,
                a.title, u.name AS owner_name
         FROM scope_requests s
         JOIN assists a ON a.ref = s.assist_ref
         JOIN users u ON u.id = a.owner_id
         WHERE s.requester_id = ? AND s.kind != 'comment'
           AND s.status IN ('approved', 'denied')
           AND s.decided_at IS NOT NULL AND s.decided_at >= ?",
    )
    .bind(&viewer.id)
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    for r in rows {
        let kind: String = r.get("kind");
        let status: String = r.get("status");
        let owner: String = r.get("owner_name");
        let target: Option<String> = r.get("target");
        let what = match &target {
            Some(t) => format!("{} ({t})", kind_label(&kind)),
            None => kind_label(&kind).to_string(),
        };
        notifications.push(HubNotification {
            id: format!("req-{}-decided", r.get::<i64, _>("id")),
            kind: "scope_decided".into(),
            assist_ref: r.get("assist_ref"),
            assist_title: r.get("title"),
            actor_name: owner.clone(),
            message: format!("{owner} {status} your {what} request"),
            at: r.get("decided_at"),
        });
    }

    // Responders joining assists the viewer owns.
    let rows = sqlx::query(
        "SELECT r.joined_at, r.assist_ref, a.title, u.name AS responder_name
         FROM responders r
         JOIN assists a ON a.ref = r.assist_ref
         JOIN users u ON u.id = r.user_id
         WHERE a.owner_id = ? AND r.user_id != ? AND r.joined_at >= ?",
    )
    .bind(&viewer.id)
    .bind(&viewer.id)
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    for r in rows {
        let name: String = r.get("responder_name");
        notifications.push(HubNotification {
            id: format!("join-{}-{}", r.get::<String, _>("assist_ref"), name),
            kind: "responder_joined".into(),
            assist_ref: r.get("assist_ref"),
            assist_title: r.get("title"),
            actor_name: name.clone(),
            message: format!("{name} is responding"),
            at: r.get("joined_at"),
        });
    }

    // Credits the viewer received.
    let rows = sqlx::query(
        "SELECT c.id, c.created_at, c.assist_ref, a.title, u.name AS owner_name
         FROM credits c
         JOIN assists a ON a.ref = c.assist_ref
         JOIN users u ON u.id = c.from_owner_id
         WHERE c.to_responder_id = ? AND c.created_at >= ?",
    )
    .bind(&viewer.id)
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    for r in rows {
        let owner: String = r.get("owner_name");
        notifications.push(HubNotification {
            id: format!("credit-{}", r.get::<i64, _>("id")),
            kind: "credited".into(),
            assist_ref: r.get("assist_ref"),
            assist_title: r.get("title"),
            actor_name: owner.clone(),
            message: format!("{owner} credited you"),
            at: r.get("created_at"),
        });
    }

    notifications.sort_by(|a, b| a.at.cmp(&b.at));
    Ok(Json(NotificationsResponse { now, notifications }))
}
