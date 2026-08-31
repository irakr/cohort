//! Brief drafting. With ANTHROPIC_API_KEY set, asks the Claude Messages API to
//! draft the brief from the owner's selected artifacts; on ANY failure (no key,
//! HTTP error, refusal, unparseable output) falls back to a deterministic
//! template. The endpoint never fails for LLM reasons.

use crate::config::Config;
use crate::domain::{AssistArtifact, BriefDraft, Failure};
use serde_json::{json, Value};
use std::time::Duration;

const SYSTEM_PROMPT: &str = "You draft the brief for a Cohort assist: a structured summary of a \
developer's problem, written from artifact descriptors captured on their machine. Respond with \
STRICT JSON only, no prose and no code fences, matching: \
{\"goal\": \"markdown, 1 short paragraph plus at most 3 bullets\", \
\"failures\": [{\"label\": \"failing command or error\", \"note\": \"short qualifier\"}], \
\"environment\": [\"short chip strings\"]}. \
Write the goal as the outcome the developer wants, not a restatement of the failure. \
Never invent file contents or secrets; use only what the descriptors say.";

pub async fn draft_brief(
    config: &Config,
    http: &reqwest::Client,
    title: &str,
    artifacts: &[AssistArtifact],
) -> BriefDraft {
    if let Some(key) = &config.anthropic_api_key {
        match call_claude(config, http, key, title, artifacts).await {
            Ok(draft) => return draft,
            Err(e) => tracing::warn!(error = %e, "brief draft via Claude failed, using fallback"),
        }
    }
    deterministic_draft(title, artifacts)
}

async fn call_claude(
    config: &Config,
    http: &reqwest::Client,
    key: &str,
    title: &str,
    artifacts: &[AssistArtifact],
) -> Result<BriefDraft, String> {
    let user_msg = format!(
        "Title: {}\nArtifacts:\n{}",
        if title.is_empty() { "(untitled)" } else { title },
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

/// Templated draft built only from the descriptors. Keeps the create flow
/// working with no API key and in tests.
pub fn deterministic_draft(title: &str, artifacts: &[AssistArtifact]) -> BriefDraft {
    let subject = if title.is_empty() { "this problem" } else { title };
    let goal = format!(
        "Get \"{subject}\" unblocked.\n- reproduce the failure from the shared artifacts\n- find the first divergence from a working run"
    );
    let mut failures: Vec<Failure> = artifacts
        .iter()
        .filter(|a| a.kind == "terminal")
        .map(|a| Failure {
            label: format!("{}: last command failed", a.label),
            note: "captured from terminal".into(),
        })
        .collect();
    failures.extend(artifacts.iter().filter(|a| a.kind == "ai_agent").map(|a| Failure {
        label: format!("{} repeating the same error", a.label),
        note: a.detail.clone(),
    }));
    if failures.is_empty() {
        failures.push(Failure {
            label: "Failing command not yet captured".into(),
            note: "add a terminal artifact to capture it".into(),
        });
    }
    let mut environment: Vec<String> = artifacts
        .iter()
        .filter(|a| a.kind == "file" || a.kind == "custom")
        .map(|a| a.detail.clone())
        .filter(|d| !d.is_empty())
        .collect();
    environment.dedup();
    if environment.is_empty() {
        environment.push("environment not yet fingerprinted".into());
    }
    BriefDraft { goal, failures, environment }
}
