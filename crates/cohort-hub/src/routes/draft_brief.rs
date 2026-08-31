use crate::domain::{BriefDraft, DraftBriefRequest};
use crate::error::AppError;
use crate::llm;
use crate::AppState;
use axum::extract::State;
use axum::Json;

/// Draft the brief from the owner's selected artifacts. The artifacts are
/// analyzed to draft the overview; they are never shown to responders as-is.
pub async fn draft(
    State(state): State<AppState>,
    Json(req): Json<DraftBriefRequest>,
) -> Result<Json<BriefDraft>, AppError> {
    let draft = llm::draft_brief(&state.config, &state.http, &req.title, &req.artifacts).await;
    Ok(Json(draft))
}
