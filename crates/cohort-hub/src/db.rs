use crate::domain::*;
use crate::error::AppError;
use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

pub async fn pool(db: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(db)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        // A single connection keeps in-memory test databases coherent and is
        // plenty for the base version.
        .max_connections(1)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await.map_err(|e| {
        sqlx::Error::Protocol(format!("migration failed: {e}"))
    })?;
    Ok(pool)
}

// Millisecond precision: notification cursors compare timestamps with a
// strict `>`, so same-second events must still order after the cursor. Keep
// every generated timestamp in this one format - the comparisons are
// lexicographic and only hold if the format is uniform.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_enum<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, AppError> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| AppError::BadRequest(format!("bad enum value '{s}': {e}")))
}

pub fn enum_str<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Resolve the current user from `X-User-Id`, defaulting to `u-alex` (seeded)
/// so plain curl works. Seeded users only - real auth arrives with P2.
pub async fn current_user(pool: &SqlitePool, headers: &HeaderMap) -> Result<User, AppError> {
    let id = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("u-alex")
        .to_string();
    let row = sqlx::query("SELECT id, name, initials FROM users WHERE id = ?")
        .bind(&id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Forbidden(format!("unknown user '{id}'")))?;
    Ok(User {
        id: row.get("id"),
        name: row.get("name"),
        initials: row.get("initials"),
    })
}

pub async fn next_ref(pool: &SqlitePool) -> Result<String, AppError> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(CAST(SUBSTR(ref, 3) AS INTEGER)), 2400) + 1 FROM assists",
    )
    .fetch_one(pool)
    .await?;
    Ok(format!("S-{n}"))
}

/// Raw assist row, mapped by hand (no compile-time macros -> no DATABASE_URL at build).
pub struct AssistRow {
    pub ref_: String,
    pub title: String,
    pub status: AssistStatus,
    pub category: Option<Category>,
    pub owner_id: String,
    pub owner_name: String,
    pub anonymous: bool,
    pub description: String,
    pub insights: String,
    pub environment: Vec<String>,
    pub live_data: Option<LiveData>,
    pub created_at: String,
    pub closed_at: Option<String>,
}

pub async fn assist_row(pool: &SqlitePool, ref_: &str) -> Result<AssistRow, AppError> {
    let row = sqlx::query(
        "SELECT a.*, u.name AS owner_name FROM assists a
         JOIN users u ON u.id = a.owner_id WHERE a.ref = ?",
    )
    .bind(ref_)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let status: String = row.get("status");
    let category: Option<String> = row.get("category");
    let environment: String = row.get("environment");
    let live_data: Option<String> = row.get("live_data");
    Ok(AssistRow {
        ref_: row.get("ref"),
        title: row.get("title"),
        status: parse_enum(&status)?,
        category: match category {
            Some(c) => Some(parse_enum(&c)?),
            None => None,
        },
        owner_id: row.get("owner_id"),
        owner_name: row.get("owner_name"),
        anonymous: row.get::<i64, _>("anonymous") != 0,
        description: row.get("description"),
        insights: row.get("insights"),
        environment: serde_json::from_str(&environment).unwrap_or_default(),
        live_data: live_data.and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get("created_at"),
        closed_at: row.get("closed_at"),
    })
}

pub fn display_owner(row: &AssistRow, viewer_id: &str) -> String {
    if row.anonymous && row.owner_id != viewer_id {
        "Anonymous".to_string()
    } else {
        row.owner_name.clone()
    }
}

pub async fn tags_for(pool: &SqlitePool, ref_: &str) -> Result<Vec<String>, AppError> {
    let rows = sqlx::query("SELECT tag FROM assist_tags WHERE assist_ref = ? ORDER BY tag")
        .bind(ref_)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("tag")).collect())
}

pub async fn responders_for(pool: &SqlitePool, ref_: &str) -> Result<Vec<User>, AppError> {
    let rows = sqlx::query(
        "SELECT u.id, u.name, u.initials FROM responders r
         JOIN users u ON u.id = r.user_id WHERE r.assist_ref = ? ORDER BY r.joined_at",
    )
    .bind(ref_)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| User {
            id: r.get("id"),
            name: r.get("name"),
            initials: r.get("initials"),
        })
        .collect())
}

pub async fn artifacts_for(pool: &SqlitePool, ref_: &str) -> Result<Vec<AssistArtifact>, AppError> {
    let rows = sqlx::query(
        "SELECT id, kind, label, detail, icon, pid FROM assist_artifacts WHERE assist_ref = ? ORDER BY id",
    )
    .bind(ref_)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AssistArtifact {
            id: r.get("id"),
            kind: r.get("kind"),
            label: r.get("label"),
            detail: r.get("detail"),
            icon: r.get("icon"),
            pid: r.get("pid"),
        })
        .collect())
}

pub async fn scope_requests_for(
    pool: &SqlitePool,
    ref_: &str,
) -> Result<Vec<ScopeRequest>, AppError> {
    let rows = sqlx::query(
        "SELECT s.*, u.name AS requester_name FROM scope_requests s
         JOIN users u ON u.id = s.requester_id
         WHERE s.assist_ref = ? ORDER BY s.created_at, s.id",
    )
    .bind(ref_)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|r| {
            let kind: String = r.get("kind");
            let status: String = r.get("status");
            Ok(ScopeRequest {
                id: r.get("id"),
                assist_ref: r.get("assist_ref"),
                requester_id: r.get("requester_id"),
                requester_name: r.get("requester_name"),
                kind: parse_enum(&kind)?,
                target: r.get("target"),
                reason: r.get("reason"),
                status: parse_enum(&status)?,
                ttl_minutes: r.get("ttl_minutes"),
                created_at: r.get("created_at"),
                decided_at: r.get("decided_at"),
            })
        })
        .collect()
}

/// Derive live grants: approved scope requests that have not expired, on an
/// assist that is not done. Comments are conversation, not access - excluded.
pub fn derive_grants(requests: &[ScopeRequest], assist_status: AssistStatus) -> Vec<Grant> {
    if assist_status == AssistStatus::Done {
        return vec![];
    }
    let now = Utc::now();
    requests
        .iter()
        .filter(|r| r.status == ScopeStatus::Approved && r.kind != ScopeKind::Comment)
        .filter_map(|r| {
            let expires_at = match (r.ttl_minutes, &r.decided_at) {
                (Some(ttl), Some(decided)) => {
                    let decided = DateTime::parse_from_rfc3339(decided).ok()?;
                    let exp = decided.with_timezone(&Utc) + Duration::minutes(ttl);
                    if exp <= now {
                        return None; // expired
                    }
                    Some(exp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                }
                _ => None, // no TTL = until close
            };
            Some(Grant {
                scope_request_id: r.id,
                kind: r.kind,
                target: r.target.clone(),
                granted_to_id: r.requester_id.clone(),
                granted_to_name: r.requester_name.clone(),
                expires_at,
            })
        })
        .collect()
}

pub async fn assist_detail(
    pool: &SqlitePool,
    ref_: &str,
    viewer: &User,
) -> Result<AssistDetail, AppError> {
    let row = assist_row(pool, ref_).await?;
    let tags = tags_for(pool, ref_).await?;
    let responders = responders_for(pool, ref_).await?;
    let artifacts = artifacts_for(pool, ref_).await?;
    let scope_requests = scope_requests_for(pool, ref_).await?;
    let grants = derive_grants(&scope_requests, row.status);
    let viewer_is_owner = row.owner_id == viewer.id;
    let viewer_is_responder = responders.iter().any(|r| r.id == viewer.id);
    Ok(AssistDetail {
        owner_name: display_owner(&row, &viewer.id),
        ref_: row.ref_,
        title: row.title,
        status: row.status,
        category: row.category,
        tags,
        owner_id: row.owner_id,
        anonymous: row.anonymous,
        description: row.description,
        insights: row.insights,
        environment: row.environment,
        artifacts,
        responders,
        scope_requests,
        grants,
        viewer_is_owner,
        viewer_is_responder,
        created_at: row.created_at,
        closed_at: row.closed_at,
    })
}
