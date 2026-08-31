use crate::domain::{CreateUser, User};
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

fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_uppercase()
}

/// Register a user for this machine's app instance. Identity only, no
/// credentials; real auth arrives with P2.
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateUser>,
) -> Result<Json<User>, AppError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("a name is required".into()));
    }
    let base = slug(name);
    if base.is_empty() {
        return Err(AppError::BadRequest("the name needs letters or digits".into()));
    }
    let mut id = format!("u-{base}");
    let mut n = 2;
    loop {
        let taken: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(&id)
            .fetch_one(&state.pool)
            .await?;
        if taken == 0 {
            break;
        }
        id = format!("u-{base}-{n}");
        n += 1;
    }
    let user = User {
        id,
        name: name.to_string(),
        initials: initials(name),
    };
    sqlx::query("INSERT INTO users (id, name, initials) VALUES (?, ?, ?)")
        .bind(&user.id)
        .bind(&user.name)
        .bind(&user.initials)
        .execute(&state.pool)
        .await?;
    Ok(Json(user))
}
