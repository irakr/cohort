use crate::db::{self, now_rfc3339};
use crate::domain::*;
use crate::error::AppError;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// CSV of statuses, e.g. "open,dormant". Absent = all.
    pub status: Option<String>,
    pub tag: Option<String>,
    pub mine: Option<bool>,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<AssistSummary>>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let statuses: Option<Vec<String>> = params.status.as_ref().map(|s| {
        s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
    });

    let rows = sqlx::query(
        "SELECT a.ref FROM assists a ORDER BY a.created_at DESC, a.ref DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::new();
    for r in rows {
        let ref_: String = r.get("ref");
        let row = db::assist_row(&state.pool, &ref_).await?;
        if let Some(ss) = &statuses {
            if !ss.contains(&db::enum_str(&row.status)) {
                continue;
            }
        }
        let tags = db::tags_for(&state.pool, &ref_).await?;
        if let Some(tag) = &params.tag {
            if !tags.contains(tag) {
                continue;
            }
        }
        let responders = db::responders_for(&state.pool, &ref_).await?;
        let is_mine = row.owner_id == viewer.id || responders.iter().any(|u| u.id == viewer.id);
        if params.mine.unwrap_or(false) && !is_mine {
            continue;
        }
        out.push(AssistSummary {
            owner_name: db::display_owner(&row, &viewer.id),
            ref_: row.ref_,
            title: row.title,
            status: row.status,
            category: row.category,
            tags,
            responder_names: responders.into_iter().map(|u| u.name).collect(),
            created_at: row.created_at,
            is_mine,
        });
    }
    Ok(Json(out))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateAssist>,
) -> Result<Json<AssistDetail>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let title = req.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    let ref_ = db::next_ref(&state.pool).await?;
    sqlx::query(
        "INSERT INTO assists (ref, title, status, category, owner_id, anonymous, goal, failures, environment, created_at)
         VALUES (?, ?, 'open', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&ref_)
    .bind(title)
    .bind(req.category.as_ref().map(db::enum_str))
    .bind(&viewer.id)
    .bind(req.anonymous as i64)
    .bind(&req.goal)
    .bind(serde_json::to_string(&req.failures).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&req.environment).unwrap_or_else(|_| "[]".into()))
    .bind(now_rfc3339())
    .execute(&state.pool)
    .await?;

    for tag in req.tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        sqlx::query("INSERT OR IGNORE INTO assist_tags (assist_ref, tag) VALUES (?, ?)")
            .bind(&ref_).bind(tag)
            .execute(&state.pool)
            .await?;
    }
    for a in &req.artifacts {
        sqlx::query(
            "INSERT OR IGNORE INTO assist_artifacts (assist_ref, id, kind, label, detail) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&ref_).bind(&a.id).bind(&a.kind).bind(&a.label).bind(&a.detail)
        .execute(&state.pool)
        .await?;
    }
    Ok(Json(db::assist_detail(&state.pool, &ref_, &viewer).await?))
}

pub async fn detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ref_): Path<String>,
) -> Result<Json<AssistDetail>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    Ok(Json(db::assist_detail(&state.pool, &ref_, &viewer).await?))
}

pub async fn join(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ref_): Path<String>,
) -> Result<Json<AssistDetail>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    if row.owner_id == viewer.id {
        return Err(AppError::BadRequest("the owner cannot join as a responder".into()));
    }
    if row.status == AssistStatus::Done {
        return Err(AppError::BadRequest("this assist is closed".into()));
    }
    sqlx::query("INSERT OR IGNORE INTO responders (assist_ref, user_id, joined_at) VALUES (?, ?, ?)")
        .bind(&ref_).bind(&viewer.id).bind(now_rfc3339())
        .execute(&state.pool)
        .await?;
    Ok(Json(db::assist_detail(&state.pool, &ref_, &viewer).await?))
}

/// Seeded live data (file tree, file contents, terminal feed, agent chat).
/// The client gates each pane on the viewer's grants; when the owner agent
/// module streams for real, enforcement moves server-side with it.
pub async fn live_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ref_): Path<String>,
) -> Result<Json<LiveData>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    let responders = db::responders_for(&state.pool, &ref_).await?;
    let allowed = row.owner_id == viewer.id || responders.iter().any(|u| u.id == viewer.id);
    if !allowed {
        return Err(AppError::Forbidden("join the assist to view its live data".into()));
    }
    Ok(Json(row.live_data.unwrap_or_default()))
}

fn build_record_draft(
    row: &db::AssistRow,
    artifacts: &[AssistArtifact],
    requests: &[ScopeRequest],
) -> RecordFields {
    let symptom = match row.failures.first() {
        Some(f) => format!("{} ({})", f.label, f.note),
        None => row.title.clone(),
    };
    let scopes = requests
        .iter()
        .filter(|r| r.status == ScopeStatus::Approved && r.kind != ScopeKind::Comment)
        .map(|r| match &r.target {
            Some(t) => format!("{}: {}", db::enum_str(&r.kind), t),
            None => db::enum_str(&r.kind),
        })
        .chain(artifacts.iter().map(|a| format!("shared at open: {}", a.label)))
        .collect::<Vec<_>>()
        .join(" - ");
    RecordFields {
        symptom,
        env_fingerprint: row.environment.join(" - "),
        scopes_that_mattered: scopes,
        dead_ends: String::new(),
        fix: String::new(),
    }
}

pub async fn record_draft(
    State(state): State<AppState>,
    Path(ref_): Path<String>,
) -> Result<Json<RecordFields>, AppError> {
    let row = db::assist_row(&state.pool, &ref_).await?;
    let artifacts = db::artifacts_for(&state.pool, &ref_).await?;
    let requests = db::scope_requests_for(&state.pool, &ref_).await?;
    Ok(Json(build_record_draft(&row, &artifacts, &requests)))
}

/// Close the assist: set status, record credits (skippable), and write the
/// resolution record. A record is written on EVERY close, whatever the outcome.
pub async fn close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ref_): Path<String>,
    Json(req): Json<CloseAssist>,
) -> Result<Json<AssistDetail>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;
    let row = db::assist_row(&state.pool, &ref_).await?;
    if row.owner_id != viewer.id {
        return Err(AppError::Forbidden("only the owner can close an assist".into()));
    }
    if row.status == AssistStatus::Done {
        return Err(AppError::BadRequest("this assist is already closed".into()));
    }
    let artifacts = db::artifacts_for(&state.pool, &ref_).await?;
    let requests = db::scope_requests_for(&state.pool, &ref_).await?;
    let responders = db::responders_for(&state.pool, &ref_).await?;
    let draft = build_record_draft(&row, &artifacts, &requests);
    let now = now_rfc3339();

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE assists SET status = 'done', closed_at = ? WHERE ref = ?")
        .bind(&now).bind(&ref_)
        .execute(&mut *tx)
        .await?;
    // Credit only actual responders; crediting is a gift from the owner.
    for id in &req.credited_user_ids {
        if responders.iter().any(|u| &u.id == id) {
            sqlx::query(
                "INSERT OR IGNORE INTO credits (assist_ref, from_owner_id, to_responder_id, created_at)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&ref_).bind(&viewer.id).bind(id).bind(&now)
            .execute(&mut *tx)
            .await?;
        }
    }
    let pick = |given: &str, drafted: &str| -> String {
        if given.trim().is_empty() { drafted.to_string() } else { given.to_string() }
    };
    sqlx::query(
        "INSERT INTO resolution_records (assist_ref, outcome, symptom, env_fingerprint, scopes_that_mattered, dead_ends, fix, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&ref_)
    .bind(db::enum_str(&req.outcome))
    .bind(pick(&req.record.symptom, &draft.symptom))
    .bind(pick(&req.record.env_fingerprint, &draft.env_fingerprint))
    .bind(pick(&req.record.scopes_that_mattered, &draft.scopes_that_mattered))
    .bind(req.record.dead_ends.trim())
    .bind(req.record.fix.trim())
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(db::assist_detail(&state.pool, &ref_, &viewer).await?))
}

pub async fn record(
    State(state): State<AppState>,
    Path(ref_): Path<String>,
) -> Result<Json<ResolutionRecord>, AppError> {
    let row = sqlx::query("SELECT * FROM resolution_records WHERE assist_ref = ?")
        .bind(&ref_)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let outcome: String = row.get("outcome");
    Ok(Json(ResolutionRecord {
        assist_ref: row.get("assist_ref"),
        outcome: serde_json::from_value(serde_json::Value::String(outcome))
            .map_err(|e| AppError::BadRequest(e.to_string()))?,
        symptom: row.get("symptom"),
        env_fingerprint: row.get("env_fingerprint"),
        scopes_that_mattered: row.get("scopes_that_mattered"),
        dead_ends: row.get("dead_ends"),
        fix: row.get("fix"),
        created_at: row.get("created_at"),
    }))
}
