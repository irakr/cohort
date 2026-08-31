use crate::domain::{BriefDraft, DraftBriefRequest};
use crate::error::AppError;
use crate::llm;
use crate::AppState;
use axum::extract::State;
use axum::Json;

/// Draft the insights from the owner's title, description, and selected
/// artifacts. Empty draft without an API key - the UI shows N/A; nothing is
/// ever invented.
pub async fn draft(
    State(state): State<AppState>,
    Json(req): Json<DraftBriefRequest>,
) -> Result<Json<BriefDraft>, AppError> {
    let draft =
        llm::draft_brief(&state.config, &state.http, &req.title, &req.description, &req.artifacts)
            .await;
    Ok(Json(draft))
}
