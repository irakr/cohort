use crate::db;
use crate::domain::*;
use crate::error::AppError;
use crate::AppState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use sqlx::Row;

/// The private contribution record. Outbound help accumulates (credits earned,
/// responses, records authored). Inbound help appears ONLY as responder names
/// on individual assists - never as any aggregate. Do not add such a field.
pub async fn my_record(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MyRecord>, AppError> {
    let viewer = db::current_user(&state.pool, &headers).await?;

    let credits_earned: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM credits WHERE to_responder_id = ?")
            .bind(&viewer.id)
            .fetch_one(&state.pool)
            .await?;
    let responses_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM responders WHERE user_id = ?")
            .bind(&viewer.id)
            .fetch_one(&state.pool)
            .await?;
    let records_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM resolution_records r
         JOIN assists a ON a.ref = r.assist_ref WHERE a.owner_id = ?",
    )
    .bind(&viewer.id)
    .fetch_one(&state.pool)
    .await?;

    let refs = sqlx::query(
        "SELECT DISTINCT a.ref, a.created_at FROM assists a
         LEFT JOIN responders r ON r.assist_ref = a.ref
         WHERE a.owner_id = ? OR r.user_id = ?
         ORDER BY a.created_at DESC",
    )
    .bind(&viewer.id)
    .bind(&viewer.id)
    .fetch_all(&state.pool)
    .await?;

    let mut my_assists = Vec::new();
    for r in refs {
        let ref_: String = r.get("ref");
        let row = db::assist_row(&state.pool, &ref_).await?;
        let responders = db::responders_for(&state.pool, &ref_).await?;
        let outcome: Option<String> =
            sqlx::query_scalar("SELECT outcome FROM resolution_records WHERE assist_ref = ?")
                .bind(&ref_)
                .fetch_optional(&state.pool)
                .await?;
        let outcome = match outcome {
            Some(o) => serde_json::from_value(serde_json::Value::String(o)).ok(),
            None => None,
        };
        let role = if row.owner_id == viewer.id { "owner" } else { "responder" };
        my_assists.push(MyAssistRow {
            ref_: row.ref_,
            title: row.title,
            status: row.status,
            role: role.into(),
            responder_names: responders.into_iter().map(|u| u.name).collect(),
            outcome,
            created_at: row.created_at,
        });
    }

    let credit_rows = sqlx::query(
        "SELECT c.assist_ref, c.created_at, a.title, u.name AS from_owner_name,
                (SELECT outcome FROM resolution_records rr WHERE rr.assist_ref = c.assist_ref) AS outcome
         FROM credits c
         JOIN assists a ON a.ref = c.assist_ref
         JOIN users u ON u.id = c.from_owner_id
         WHERE c.to_responder_id = ?
         ORDER BY c.created_at DESC",
    )
    .bind(&viewer.id)
    .fetch_all(&state.pool)
    .await?;
    let credits_rows = credit_rows
        .into_iter()
        .map(|r| {
            let outcome: Option<String> = r.get("outcome");
            CreditRow {
                assist_ref: r.get("assist_ref"),
                title: r.get("title"),
                outcome: outcome
                    .and_then(|o| serde_json::from_value(serde_json::Value::String(o)).ok()),
                from_owner_name: r.get("from_owner_name"),
                created_at: r.get("created_at"),
            }
        })
        .collect();

    Ok(Json(MyRecord {
        user: viewer,
        credits_earned,
        responses_count,
        records_count,
        my_assists,
        credits_rows,
        ai_usage: ai_usage_fixture(),
    }))
}

/// Static fixture until the detector daemon (P1) supplies real numbers.
fn ai_usage_fixture() -> Vec<AiUsageRange> {
    let agents = |scale: f64| {
        vec![
            AiAgentUsage {
                name: "Claude Code".into(),
                model: "Opus 4.5".into(),
                share_pct: 73,
                tokens: format!("{:.1}M", 6.1 * scale),
                spend: format!("Rs {:.0}", 4555.0 * scale),
            },
            AiAgentUsage {
                name: "Cursor".into(),
                model: "GPT-5.2".into(),
                share_pct: 21,
                tokens: format!("{:.1}M", 1.8 * scale),
                spend: format!("Rs {:.0}", 1310.0 * scale),
            },
            AiAgentUsage {
                name: "Aider".into(),
                model: "Qwen3-Max".into(),
                share_pct: 6,
                tokens: format!("{:.1}M", 0.5 * scale),
                spend: format!("Rs {:.0}", 375.0 * scale),
            },
        ]
    };
    vec![
        AiUsageRange {
            range: "7d".into(),
            tokens: "2.1M".into(),
            spend: "Rs 1,560".into(),
            longest_stall: "17 turns".into(),
            agents: agents(0.25),
        },
        AiUsageRange {
            range: "30d".into(),
            tokens: "8.4M".into(),
            spend: "Rs 6,240".into(),
            longest_stall: "38 turns".into(),
            agents: agents(1.0),
        },
        AiUsageRange {
            range: "90d".into(),
            tokens: "23.9M".into(),
            spend: "Rs 17,800".into(),
            longest_stall: "38 turns".into(),
            agents: agents(2.85),
        },
    ]
}
