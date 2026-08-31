use crate::domain::User;
use crate::error::AppError;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use sqlx::Row;

/// Seeded users, for the app's user switcher. Real auth arrives with P2.
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<User>>, AppError> {
    let rows = sqlx::query("SELECT id, name, initials FROM users ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| User {
                id: r.get("id"),
                name: r.get("name"),
                initials: r.get("initials"),
            })
            .collect(),
    ))
}
