//! Provider-neutral LLM transport.
//!
//! Two wire protocols cover every provider Cohort targets: OpenAI-compatible
//! chat completions (OpenAI, Deepseek, Ollama, vLLM, LM Studio, llama.cpp's
//! server, company gateways, Gemini's compatibility endpoint) and Anthropic
//! Messages. Providers are presets over those two, not code. This crate
//! knows nothing about assists: it takes a [`Prompt`], returns a
//! [`Completion`], and reports failures as [`LlmError`] - never as text.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use ts_rs::TS;

mod anthropic;
mod openai_compat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum Protocol {
    OpenaiCompatible,
    Anthropic,
}

/// What a machine needs to reach its model. Stored per user by the app;
/// the per-call knobs live in [`CallOptions`] instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct LlmConfig {
    pub protocol: Protocol,
    /// `https://api.openai.com/v1`, `http://localhost:11434/v1`,
    /// `http://10.0.0.7:8000/v1`, `https://api.anthropic.com`, ...
    pub base_url: String,
    /// None for local servers that take no key.
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CallOptions {
    pub max_output_tokens: u32,
    pub timeout: Duration,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self { max_output_tokens: 2048, timeout: Duration::from_secs(60) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// A prompt the way the model sees it, independent of provider: the system
/// turn carries role, rules and output contract; the messages carry context
/// and the ask. The adapter decides where `system` goes on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub system: String,
    /// Alternating user/assistant turns; the last one is always the user.
    pub messages: Vec<Message>,
}

impl Prompt {
    /// The common case: one system turn, one user turn.
    pub fn single(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            messages: vec![Message { role: Role::User, content: user.into() }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// The model the server reports it used.
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("no model configured")]
    NotConfigured,
    #[error("prompt must end with a user turn")]
    InvalidPrompt,
    #[error("could not reach the model: {0}")]
    Http(String),
    #[error("the model server answered {0}: {1}")]
    Status(u16, String),
    #[error("the model refused the request")]
    Refused,
    #[error("unexpected reply from the model server: {0}")]
    Malformed(String),
}

/// A shared HTTP client for callers that do not already have one.
pub fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// One non-streaming completion. Validates the prompt and the config, then
/// dispatches on the protocol.
pub async fn complete(
    http: &reqwest::Client,
    cfg: &LlmConfig,
    prompt: &Prompt,
    opts: &CallOptions,
) -> Result<Completion, LlmError> {
    if cfg.base_url.trim().is_empty() || cfg.model.trim().is_empty() {
        return Err(LlmError::NotConfigured);
    }
    match prompt.messages.last() {
        Some(m) if m.role == Role::User => {}
        _ => return Err(LlmError::InvalidPrompt),
    }
    log::debug!(
        "complete: {:?} {} model={} key={} max_output_tokens={} timeout={}s system={} chars, {} message(s)",
        cfg.protocol,
        cfg.base_url,
        cfg.model,
        if cfg.api_key.as_deref().is_some_and(|k| !k.is_empty()) { "set" } else { "none" },
        opts.max_output_tokens,
        opts.timeout.as_secs(),
        prompt.system.chars().count(),
        prompt.messages.len(),
    );
    match cfg.protocol {
        Protocol::OpenaiCompatible => openai_compat::complete(http, cfg, prompt, opts).await,
        Protocol::Anthropic => anthropic::complete(http, cfg, prompt, opts).await,
    }
}

/// Send a built request and hand the JSON body to the protocol's parser.
/// The whole exchange is logged at debug - the request body carries the
/// prompt and never the key, which travels in a header - and one summary
/// line at info.
async fn send(
    url: &str,
    body: &serde_json::Value,
    request: reqwest::RequestBuilder,
    parse: fn(serde_json::Value) -> Result<Completion, LlmError>,
) -> Result<Completion, LlmError> {
    log::debug!("request -> {url}\n{}", serde_json::to_string_pretty(body).unwrap_or_default());
    let started = Instant::now();
    let response = request.send().await.map_err(|e| {
        log::warn!("request to {url} failed after {}ms: {e}", started.elapsed().as_millis());
        LlmError::Http(e.to_string())
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|e| LlmError::Http(e.to_string()))?;
    log::debug!("response <- {} in {}ms\n{text}", status.as_u16(), started.elapsed().as_millis());
    if !status.is_success() {
        let message = server_message(&text);
        log::warn!("{url} answered {}: {message}", status.as_u16());
        return Err(LlmError::Status(status.as_u16(), message));
    }
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| LlmError::Malformed(e.to_string()))?;
    let completion = parse(value)?;
    log::info!(
        "{url} model={} tokens in={} out={} {}ms",
        completion.model,
        completion.input_tokens,
        completion.output_tokens,
        started.elapsed().as_millis()
    );
    Ok(completion)
}

/// Both protocols put a human-readable reason at `error.message`; fall back
/// to a clipped body.
fn server_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(m) = v["error"]["message"].as_str() {
            return m.to_string();
        }
    }
    body.chars().take(200).collect()
}

fn join(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

/// A named starting point for the settings form. Picking one fills the
/// fields; the user can change any of them. Default models are set only
/// where they are known for certain - the form asks for the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub default_model: String,
    pub needs_key: bool,
}

pub fn presets() -> Vec<Preset> {
    let preset = |id: &str, name: &str, protocol, base_url: &str, model: &str, needs_key| Preset {
        id: id.into(),
        name: name.into(),
        protocol,
        base_url: base_url.into(),
        default_model: model.into(),
        needs_key,
    };
    vec![
        preset("anthropic", "Anthropic", Protocol::Anthropic, "https://api.anthropic.com", "claude-opus-5", true),
        preset("openai", "OpenAI", Protocol::OpenaiCompatible, "https://api.openai.com/v1", "", true),
        preset("deepseek", "Deepseek", Protocol::OpenaiCompatible, "https://api.deepseek.com/v1", "", true),
        preset(
            "gemini",
            "Gemini",
            Protocol::OpenaiCompatible,
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "",
            true,
        ),
        preset("ollama", "Ollama (local)", Protocol::OpenaiCompatible, "http://localhost:11434/v1", "", false),
        preset("custom", "Custom (OpenAI-compatible)", Protocol::OpenaiCompatible, "", "", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_config_is_not_configured() {
        let cfg = LlmConfig {
            protocol: Protocol::OpenaiCompatible,
            base_url: "".into(),
            api_key: None,
            model: "m".into(),
        };
        let err = complete(&client(), &cfg, &Prompt::single("s", "u"), &CallOptions::default())
            .await
            .unwrap_err();
        assert!(matches!(err, LlmError::NotConfigured));
    }

    #[tokio::test]
    async fn prompt_must_end_with_user_turn() {
        let cfg = LlmConfig {
            protocol: Protocol::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            api_key: Some("k".into()),
            model: "claude-opus-5".into(),
        };
        let prompt = Prompt {
            system: "s".into(),
            messages: vec![Message { role: Role::Assistant, content: "hello".into() }],
        };
        let err = complete(&client(), &cfg, &prompt, &CallOptions::default()).await.unwrap_err();
        assert!(matches!(err, LlmError::InvalidPrompt));
    }

    #[test]
    fn server_message_prefers_the_error_field() {
        assert_eq!(server_message(r#"{"error":{"message":"bad key","type":"auth"}}"#), "bad key");
        assert_eq!(server_message("plain text"), "plain text");
    }

    #[test]
    fn join_tolerates_trailing_slash() {
        assert_eq!(join("https://x/v1/", "/chat/completions"), "https://x/v1/chat/completions");
        assert_eq!(join("https://x/v1", "/chat/completions"), "https://x/v1/chat/completions");
    }

    #[test]
    fn presets_are_distinct_and_only_anthropic_has_a_known_default_model() {
        let all = presets();
        let mut ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for p in &all {
            if p.id == "anthropic" {
                assert_eq!(p.default_model, "claude-opus-5");
            } else {
                assert!(p.default_model.is_empty(), "{} must not guess a model id", p.id);
            }
        }
    }
}
