//! Anthropic Messages: `POST {base_url}/v1/messages`. The system prompt is
//! the top-level `system` field, not a message. No thinking, effort or
//! sampling fields are sent: current models run adaptive thinking by
//! default, and a bare request is valid on every model. A `refusal` stop
//! reason is a failure, never text.

use crate::{join, send, CallOptions, Completion, LlmConfig, LlmError, Prompt, Role};
use serde_json::{json, Value};

const API_VERSION: &str = "2023-06-01";

pub(crate) fn url(base_url: &str) -> String {
    join(base_url, "/v1/messages")
}

pub(crate) fn request_body(cfg: &LlmConfig, prompt: &Prompt, opts: &CallOptions) -> Value {
    let messages: Vec<Value> = prompt
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            json!({ "role": role, "content": m.content })
        })
        .collect();
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": opts.max_output_tokens,
        "messages": messages,
    });
    if !prompt.system.is_empty() {
        body["system"] = Value::String(prompt.system.clone());
    }
    body
}

pub(crate) fn parse(v: Value) -> Result<Completion, LlmError> {
    if v["stop_reason"] == "refusal" {
        return Err(LlmError::Refused);
    }
    let text = v["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .ok_or_else(|| LlmError::Malformed("no text block in content".into()))?
        .to_string();
    Ok(Completion {
        text,
        input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
        model: v["model"].as_str().unwrap_or_default().to_string(),
    })
}

pub(crate) async fn complete(
    http: &reqwest::Client,
    cfg: &LlmConfig,
    prompt: &Prompt,
    opts: &CallOptions,
) -> Result<Completion, LlmError> {
    let url = url(&cfg.base_url);
    let body = request_body(cfg, prompt, opts);
    let request = http
        .post(&url)
        .timeout(opts.timeout)
        .header("x-api-key", cfg.api_key.clone().unwrap_or_default())
        .header("anthropic-version", API_VERSION)
        .json(&body);
    send(&url, &body, request, parse).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Message, Protocol};

    fn cfg() -> LlmConfig {
        LlmConfig {
            protocol: Protocol::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            api_key: Some("sk-ant".into()),
            model: "claude-opus-5".into(),
        }
    }

    #[test]
    fn system_is_top_level_and_not_a_message() {
        let prompt = Prompt {
            system: "rules".into(),
            messages: vec![
                Message { role: Role::User, content: "q1".into() },
                Message { role: Role::Assistant, content: "a1".into() },
                Message { role: Role::User, content: "q2".into() },
            ],
        };
        let body = request_body(&cfg(), &prompt, &CallOptions { max_output_tokens: 77, ..Default::default() });
        assert_eq!(body["system"], "rules");
        assert_eq!(body["model"], "claude-opus-5");
        assert_eq!(body["max_tokens"], 77);
        let roles: Vec<&str> = body["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["user", "assistant", "user"]);
        let keys: Vec<&String> = body.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 4, "only model, max_tokens, messages, system: {keys:?}");
        assert!(body.get("thinking").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn empty_system_is_omitted() {
        let body = request_body(&cfg(), &Prompt::single("", "hi"), &CallOptions::default());
        assert!(body.get("system").is_none());
    }

    #[test]
    fn url_is_v1_messages_under_the_base() {
        assert_eq!(url("https://api.anthropic.com"), "https://api.anthropic.com/v1/messages");
        assert_eq!(url("https://gateway.internal/anthropic/"), "https://gateway.internal/anthropic/v1/messages");
    }

    #[test]
    fn parses_the_first_text_block_and_usage() {
        let v: Value = serde_json::from_str(
            r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-opus-5","content":[{"type":"thinking","thinking":""},{"type":"text","text":"{\"insights\":\"- x\",\"environment\":[]}"}],"stop_reason":"end_turn","usage":{"input_tokens":300,"output_tokens":20}}"#,
        )
        .unwrap();
        let c = parse(v).unwrap();
        assert!(c.text.starts_with("{\"insights\""));
        assert_eq!(c.input_tokens, 300);
        assert_eq!(c.output_tokens, 20);
        assert_eq!(c.model, "claude-opus-5");
    }

    #[test]
    fn refusal_is_an_error_not_text() {
        let v: Value = serde_json::from_str(
            r#"{"model":"claude-opus-5","content":[{"type":"text","text":"I cannot help with that."}],"stop_reason":"refusal","usage":{"input_tokens":1,"output_tokens":1}}"#,
        )
        .unwrap();
        assert!(matches!(parse(v), Err(LlmError::Refused)));
    }

    #[test]
    fn no_text_block_is_malformed() {
        let v: Value = serde_json::from_str(r#"{"content":[],"stop_reason":"end_turn"}"#).unwrap();
        assert!(matches!(parse(v), Err(LlmError::Malformed(_))));
    }
}
