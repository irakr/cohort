pub mod assists;
pub mod draft_brief;
pub mod my_record;
pub mod notifications;
pub mod scope_requests;
pub mod users;

use crate::AppState;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/users", get(users::list).post(users::create))
        .route("/api/notifications", get(notifications::list))
        .route("/api/assists", get(assists::list).post(assists::create))
        .route("/api/assists/draft-brief", post(draft_brief::draft))
        .route("/api/assists/{ref}", get(assists::detail))
        .route("/api/assists/{ref}/responders", post(assists::join))
        .route("/api/assists/{ref}/artifacts", get(assists::live_data))
        .route("/api/assists/{ref}/scope-requests", post(scope_requests::create))
        .route("/api/assists/{ref}/record-draft", get(assists::record_draft))
        .route("/api/assists/{ref}/close", post(assists::close))
        .route("/api/assists/{ref}/record", get(assists::record))
        .route("/api/scope-requests/{id}/approve", post(scope_requests::approve))
        .route("/api/scope-requests/{id}/deny", post(scope_requests::deny))
        .route("/api/my-record", get(my_record::my_record))
}
