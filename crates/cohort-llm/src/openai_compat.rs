//! OpenAI-compatible chat completions: `POST {base_url}/chat/completions`.
//! The system prompt travels as the first message with role `system`.
//! Nothing beyond model, messages and max_tokens is sent, so one request
//! shape works across every server that speaks this protocol.

use crate::{join, send, CallOptions, Completion, LlmConfig, LlmError, Prompt, Role};
use serde_json::{json, Value};

pub(crate) fn url(base_url: &str) -> String {
    join(base_url, "/chat/completions")
}

pub(crate) fn request_body(cfg: &LlmConfig, prompt: &Prompt, opts: &CallOptions) -> Value {
    let mut messages = Vec::with_capacity(prompt.messages.len() + 1);
    if !prompt.system.is_empty() {
        messages.push(json!({ "role": "system", "content": prompt.system }));
    }
    for m in &prompt.messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        messages.push(json!({ "role": role, "content": m.content }));
    }
    json!({
        "model": cfg.model,
        "messages": messages,
        "max_tokens": opts.max_output_tokens,
    })
}

pub(crate) fn parse(v: Value) -> Result<Completion, LlmError> {
    let text = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| LlmError::Malformed("no choices[0].message.content".into()))?
        .to_string();
    Ok(Completion {
        text,
        input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
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
    let mut request = http.post(&url).timeout(opts.timeout).json(&body);
    if let Some(key) = cfg.api_key.as_deref().filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }
    send(&url, &body, request, parse).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Message, Protocol};

    fn cfg() -> LlmConfig {
        LlmConfig {
            protocol: Protocol::OpenaiCompatible,
            base_url: "http://localhost:11434/v1".into(),
            api_key: None,
            model: "llama3".into(),
        }
    }

    #[test]
    fn system_goes_first_as_a_message_and_nothing_extra_is_sent() {
        let prompt = Prompt {
            system: "rules".into(),
            messages: vec![
                Message { role: Role::User, content: "q1".into() },
                Message { role: Role::Assistant, content: "a1".into() },
                Message { role: Role::User, content: "q2".into() },
            ],
        };
        let body = request_body(&cfg(), &prompt, &CallOptions { max_output_tokens: 99, ..Default::default() });
        assert_eq!(body["model"], "llama3");
        assert_eq!(body["max_tokens"], 99);
        let roles: Vec<&str> = body["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
        assert_eq!(body["messages"][0]["content"], "rules");
        let keys: Vec<&String> = body.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 3, "only model, messages, max_tokens: {keys:?}");
    }

    #[test]
    fn empty_system_is_omitted() {
        let body = request_body(&cfg(), &Prompt::single("", "hi"), &CallOptions::default());
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn url_is_chat_completions_under_the_base() {
        assert_eq!(url("https://api.openai.com/v1"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(url("https://api.deepseek.com/v1/"), "https://api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn parses_a_standard_reply() {
        let v: Value = serde_json::from_str(
            r#"{"id":"x","model":"llama3:8b","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":1,"total_tokens":13}}"#,
        )
        .unwrap();
        let c = parse(v).unwrap();
        assert_eq!(c.text, "OK");
        assert_eq!(c.input_tokens, 12);
        assert_eq!(c.output_tokens, 1);
        assert_eq!(c.model, "llama3:8b");
    }

    #[test]
    fn missing_content_is_malformed_not_empty() {
        let v: Value = serde_json::from_str(r#"{"choices":[]}"#).unwrap();
        assert!(matches!(parse(v), Err(LlmError::Malformed(_))));
    }
}
