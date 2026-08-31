//! Insights drafting. With ANTHROPIC_API_KEY set, asks the Claude Messages
//! API to analyze the owner's title, description, and artifact descriptors.
//! Without a key - or on ANY failure - the draft is EMPTY: nothing is ever
//! invented, and the UI shows Insights as N/A. The endpoint never fails for
//! LLM reasons.

use crate::config::Config;
use crate::domain::{AssistArtifact, BriefDraft};
use serde_json::{json, Value};
use std::time::Duration;

const SYSTEM_PROMPT: &str = "You draft the insights for a Cohort assist: a short analysis of what \
a stuck developer intends to do, written from their title, their own problem description, and \
descriptors of artifacts captured on their machine. Respond with STRICT JSON only, no prose and \
no code fences, matching: \
{\"insights\": \"markdown, 2-5 short bullet points\", \"environment\": [\"short chip strings\"]}. \
Each bullet must state one accurate part of the analysis: the intent, what the artifacts show, \
and the most plausible direction. Be concise. State ONLY what the inputs actually say - never \
invent failures, file contents, versions, or secrets. If the inputs are too thin to analyze, \
return {\"insights\": \"\", \"environment\": []}.";

pub async fn draft_brief(
    config: &Config,
    http: &reqwest::Client,
    title: &str,
    description: &str,
    artifacts: &[AssistArtifact],
) -> BriefDraft {
    if let Some(key) = &config.anthropic_api_key {
        match call_claude(config, http, key, title, description, artifacts).await {
            Ok(draft) => return draft,
            Err(e) => tracing::warn!(error = %e, "insights draft via Claude failed; returning empty draft"),
        }
    }
    // Honest fallback: no AI, no analysis. The UI shows N/A.
    BriefDraft { insights: String::new(), environment: Vec::new() }
}

async fn call_claude(
    config: &Config,
    http: &reqwest::Client,
    key: &str,
    title: &str,
    description: &str,
    artifacts: &[AssistArtifact],
) -> Result<BriefDraft, String> {
    let user_msg = format!(
        "Title: {}\nOwner's description: {}\nArtifacts:\n{}",
        if title.is_empty() { "(untitled)" } else { title },
        if description.is_empty() { "(none)" } else { description },
        artifacts
            .iter()
            .map(|a| format!("- [{}] {} ({})", a.kind, a.label, a.detail))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let body = json!({
        "model": config.anthropic_model,
        "max_tokens": 1024,
        "system": SYSTEM_PROMPT,
        "messages": [{ "role": "user", "content": user_msg }],
    });
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("api status {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    if v["stop_reason"] == "refusal" {
        return Err("model refused".into());
    }
    let text = v["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .ok_or("no text block in response")?;
    serde_json::from_str::<BriefDraft>(text.trim()).map_err(|e| format!("bad draft json: {e}"))
}
