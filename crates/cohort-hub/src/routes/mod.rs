pub mod assists;
pub mod frames;
pub mod my_record;
pub mod notifications;
pub mod scope_requests;
pub mod users;

use crate::AppState;
use axum::routing::{get, post, put};
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
        .route("/api/assists/{ref}", get(assists::detail).delete(assists::destroy))
        .route("/api/assists/{ref}/responders", post(assists::join))
        .route(
            "/api/assists/{ref}/artifacts",
            get(assists::live_data).post(assists::set_live_data),
        )
        .route("/api/assists/{ref}/catalog", post(assists::set_catalog))
        .route("/api/assists/{ref}/scope-requests", post(scope_requests::create))
        .route("/api/assists/{ref}/record-draft", get(assists::record_draft))
        .route("/api/assists/{ref}/close", post(assists::close))
        .route("/api/assists/{ref}/record", get(assists::record))
        .route("/api/scope-requests/{id}/approve", post(scope_requests::approve))
        .route("/api/scope-requests/{id}/deny", post(scope_requests::deny))
        .route("/api/scope-requests/{id}/revoke", post(scope_requests::revoke))
        .route(
            "/api/assists/{ref}/frames/{request_id}",
            put(frames::put_frame).get(frames::get_frame),
        )
        .route("/api/my-record", get(my_record::my_record))
}
