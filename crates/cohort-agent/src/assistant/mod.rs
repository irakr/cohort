//! The Cohort assistant: the model-facing part of the owner agent module.
//!
//! Runs inside the app, on this machine. It assembles context from what the
//! user chose to share (bounded and redacted, see [`context`]), builds a
//! structured prompt (see [`prompts`]), calls the model this machine is
//! configured with (see [`config`] and `cohort-llm`), and parses the reply.
//!
//! Nothing here is ever invented. No configuration, a refusal, a transport
//! failure or an unusable reply all yield the empty draft, with a note
//! saying why - the UI shows the empty state, never made-up content.
//!
//! Everything that crosses the wire is logged: a summary at info, the full
//! prompt and reply at debug (see the app's `COHORT_LOG`).

pub mod config;
pub mod context;
pub mod prompts;

use cohort_llm::{CallOptions, Completion};
pub use cohort_llm::{presets, LlmConfig, Preset, Protocol};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ts_rs::TS;

/// What the prompt needs to know about a shared artifact. For files,
/// `detail` is the path that gets snapshotted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ArtifactRef {
    pub kind: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct InsightsInput {
    #[serde(default)]
    pub title: String,
    /// The owner's own words: the source material for the analysis.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct BriefDraft {
    /// Short markdown bullets on what the owner intends and what the
    /// artifacts show. Empty means N/A in the UI.
    #[serde(default)]
    pub insights: String,
    #[serde(default)]
    pub environment: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct DraftOutcome {
    pub draft: BriefDraft,
    /// Why the draft is empty, when it is for a reason worth showing.
    pub note: Option<String>,
    /// The model the server reports having used.
    pub model: Option<String>,
    /// Summed over every attempt: the true cost of this draft.
    pub input_tokens: u32,
    pub output_tokens: u32,
}

const INSIGHTS_OPTIONS: CallOptions =
    CallOptions { max_output_tokens: 2048, timeout: Duration::from_secs(60) };

/// Draft the insights for a new assist from the owner's words plus the
/// shared files, read here. `None` config means not configured. A reply
/// that ignores the layout gets one corrective retry in the same
/// conversation; a second miss leaves the insights empty.
pub async fn draft_insights(cfg: Option<&LlmConfig>, input: &InsightsInput) -> DraftOutcome {
    let Some(cfg) = cfg else {
        log::info!("insights: no assistant configured on this machine");
        return DraftOutcome { note: Some("No assistant is configured on this machine.".into()), ..Default::default() };
    };
    log::info!(
        "insights: {:?} {} model={} key={} title={:?} artifacts={}",
        cfg.protocol,
        cfg.base_url,
        cfg.model,
        if cfg.api_key.as_deref().is_some_and(|k| !k.is_empty()) { "set" } else { "none" },
        input.title,
        input.artifacts.len()
    );
    let paths: Vec<String> = input
        .artifacts
        .iter()
        .filter(|a| a.kind == "file")
        .map(|a| a.detail.clone())
        .collect();
    let files = context::file_blocks(&paths, &context::INSIGHTS);
    log::info!(
        "insights: context {} file(s), {} chars, {} left out of budget, {} skipped as low-value, {} snapshot note(s)",
        files.blocks.len(),
        files.blocks.iter().map(|b| b.content.chars().count()).sum::<usize>(),
        files.not_included.len(),
        files.skipped.len(),
        files.notes.len()
    );
    log::debug!("insights: files in prompt: {:?}", files.blocks.iter().map(|b| b.path.as_str()).collect::<Vec<_>>());

    let http = cohort_llm::client();
    let mut prompt = prompts::insights(input, &files);
    let mut totals = Totals::default();
    for attempt in 1..=2 {
        let completion = match cohort_llm::complete(&http, cfg, &prompt, &INSIGHTS_OPTIONS).await {
            Ok(c) => c,
            Err(e) => {
                let when = if attempt == 2 { " on the second try" } else { "" };
                log::warn!("insights: the assistant failed{when}: {e}");
                return totals.into_outcome(BriefDraft::default(), Some(format!("The assistant failed{when}: {e}. Insights left empty.")));
            }
        };
        totals.add(&completion);
        match parse_insights_reply(&completion.text) {
            Ok(draft) => {
                log::info!(
                    "insights: drafted on attempt {attempt}: {} bullet(s), {} chip(s)",
                    draft.insights.lines().count(),
                    draft.environment.len()
                );
                return totals.into_outcome(draft, None);
            }
            Err(ReplyError(reason)) if attempt == 1 => {
                log::warn!("insights: reply unusable ({reason}); asking once more in the same conversation");
                log::debug!("insights: unusable reply:\n{}", completion.text);
                prompt = prompts::insights_retry(&prompt, &completion.text);
            }
            Err(ReplyError(reason)) => {
                log::warn!("insights: reply unusable again ({reason}); leaving insights empty");
                log::debug!("insights: unusable reply:\n{}", completion.text);
                return totals.into_outcome(
                    BriefDraft::default(),
                    Some(format!("The assistant's replies did not follow the expected layout ({reason}); insights left empty.")),
                );
            }
        }
    }
    unreachable!("the attempt loop always returns")
}

#[derive(Default)]
struct Totals {
    model: Option<String>,
    input_tokens: u32,
    output_tokens: u32,
}

impl Totals {
    fn add(&mut self, c: &Completion) {
        if !c.model.is_empty() {
            self.model = Some(c.model.clone());
        }
        self.input_tokens += c.input_tokens;
        self.output_tokens += c.output_tokens;
    }

    fn into_outcome(self, draft: BriefDraft, note: Option<String>) -> DraftOutcome {
        DraftOutcome {
            draft,
            note,
            model: self.model,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        }
    }
}

/// Why a reply could not be used.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyError(pub String);

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Insights,
    Environment,
}

/// Parse the sectioned reply described in [`prompts`]. Tolerant of what
/// models actually do around a layout: markdown decoration on the headers
/// (`## Insights:`, `**ENVIRONMENT**`), any bullet style, a code fence
/// around the whole thing, a sentence before the first header. Strict about
/// the one thing that matters: without an INSIGHTS header there is nothing
/// to trust, and the caller retries.
pub fn parse_insights_reply(text: &str) -> Result<BriefDraft, ReplyError> {
    let mut section: Option<Section> = None;
    let mut saw_insights = false;
    let mut insights: Vec<String> = Vec::new();
    let mut environment: Vec<String> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        if let Some(header) = header_of(line) {
            section = Some(header);
            saw_insights |= header == Section::Insights;
            continue;
        }
        match section {
            // Insights are prose: a line without a bullet marker still counts.
            Some(Section::Insights) => insights.push(bullet_text(line).to_string()),
            // Chips must be bullets, so a closing remark never becomes one.
            Some(Section::Environment) if is_bullet(line) => environment.push(bullet_text(line).to_string()),
            // Preamble before the first header, or a non-bullet line under
            // ENVIRONMENT: dropped.
            _ => {}
        }
    }

    if !saw_insights {
        return Err(ReplyError("no INSIGHTS section in the reply".into()));
    }

    let insights: Vec<String> = insights.into_iter().filter(|b| !b.is_empty() && !is_none(b)).collect();
    let mut chips: Vec<String> = Vec::new();
    for chip in environment {
        let chip: String = chip.chars().take(48).collect::<String>().trim().to_string();
        if chip.is_empty() || is_none(&chip) {
            continue;
        }
        if chips.iter().any(|c| c.eq_ignore_ascii_case(&chip)) {
            continue;
        }
        chips.push(chip);
        if chips.len() == 8 {
            break;
        }
    }
    Ok(BriefDraft {
        insights: insights.iter().map(|b| format!("- {b}")).collect::<Vec<_>>().join("\n"),
        environment: chips,
    })
}

/// A header is one of our two words on a line of its own, allowing the
/// decoration models add: `## Insights`, `**ENVIRONMENT:**`, `Insights:`.
fn header_of(line: &str) -> Option<Section> {
    let core = line
        .trim_start_matches(|c: char| c == '#' || c == '*' || c == '_' || c.is_whitespace())
        .trim_end_matches(|c: char| c == ':' || c == '*' || c == '_' || c.is_whitespace());
    match core.to_ascii_uppercase().as_str() {
        "INSIGHTS" | "INSIGHT" => Some(Section::Insights),
        "ENVIRONMENT" | "ENVIRONMENTS" | "ENV" => Some(Section::Environment),
        _ => None,
    }
}

fn is_bullet(line: &str) -> bool {
    bullet_text(line).len() < line.len()
}

/// The text after a bullet marker: `- `, `* `, `+ `, a bullet glyph, or a
/// list number like `1.` / `2)`. The line itself when there is none.
fn bullet_text(line: &str) -> &str {
    let l = line.trim();
    for marker in ["- ", "* ", "+ ", "\u{2022} ", "\u{2013} "] {
        if let Some(rest) = l.strip_prefix(marker) {
            return rest.trim();
        }
    }
    let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 2 {
        let rest = &l[digits..];
        if let Some(rest) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')')) {
            if rest.starts_with(' ') {
                return rest.trim();
            }
        }
    }
    l
}

fn is_none(s: &str) -> bool {
    matches!(
        s.trim().trim_matches(|c: char| c == '(' || c == ')' || c == '.').to_ascii_lowercase().as_str(),
        "none" | "n/a" | "nothing" | "no chips" | "not applicable"
    )
}

/// One tiny round trip for the settings form's Test button.
pub async fn test_config(cfg: &LlmConfig) -> Result<String, String> {
    let prompt = cohort_llm::Prompt::single(
        "You are being pinged to verify connectivity.",
        "Reply with the single word OK.",
    );
    let opts = CallOptions { max_output_tokens: 16, timeout: Duration::from_secs(30) };
    let http = cohort_llm::client();
    match cohort_llm::complete(&http, cfg, &prompt, &opts).await {
        Ok(c) => Ok(format!(
            "Reached {} ({} tokens in, {} out).",
            if c.model.is_empty() { cfg.model.clone() } else { c.model },
            c.input_tokens,
            c.output_tokens
        )),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // ---- the reply protocol ----

    #[test]
    fn parses_the_exact_layout() {
        let d = parse_insights_reply(
            "INSIGHTS\n- intent: ship 1.9.4 to staging\n- the deployment pins registry.internal\nENVIRONMENT\n- Kubernetes 1.29\n- Helm 3.14\n",
        )
        .unwrap();
        assert_eq!(d.insights, "- intent: ship 1.9.4 to staging\n- the deployment pins registry.internal");
        assert_eq!(d.environment, vec!["Kubernetes 1.29", "Helm 3.14"]);
    }

    #[test]
    fn tolerates_markdown_decoration_bullet_styles_fences_and_preamble() {
        let text = "Here is the analysis you asked for:\n```\n## Insights:\n* first point\n1. second point\n\n**ENVIRONMENT**\n\u{2022} Rust 1.98\n- rust 1.98\n+ sqlx 0.9\nHope this helps!\n```";
        let d = parse_insights_reply(text).unwrap();
        assert_eq!(d.insights, "- first point\n- second point");
        // Deduped case-insensitively; the closing remark is not a chip.
        assert_eq!(d.environment, vec!["Rust 1.98", "sqlx 0.9"]);
    }

    #[test]
    fn none_means_empty_and_environment_is_optional() {
        let d = parse_insights_reply("INSIGHTS\n- none\nENVIRONMENT\n- none").unwrap();
        assert_eq!(d, BriefDraft::default());
        let d = parse_insights_reply("INSIGHTS\n- only the intent is clear").unwrap();
        assert_eq!(d.insights, "- only the intent is clear");
        assert!(d.environment.is_empty());
    }

    #[test]
    fn prose_under_insights_still_counts_as_a_bullet() {
        let d = parse_insights_reply("INSIGHTS\nThe developer wants to ship the release.\nENVIRONMENT\n- Node 20").unwrap();
        assert_eq!(d.insights, "- The developer wants to ship the release.");
    }

    #[test]
    fn a_reply_without_the_insights_header_is_an_error() {
        assert!(parse_insights_reply("").is_err());
        assert!(parse_insights_reply("I cannot analyze this.").is_err());
        assert!(parse_insights_reply("{\"insights\": \"- old json\", \"environment\": []}").is_err());
        assert!(parse_insights_reply("ENVIRONMENT\n- Rust").is_err(), "environment alone is not enough to trust");
    }

    #[test]
    fn chips_are_clipped_and_capped() {
        let long = "x".repeat(200);
        let many: String = (1..=20).map(|i| format!("- chip {i}\n")).collect();
        let d = parse_insights_reply(&format!("INSIGHTS\n- a\nENVIRONMENT\n- {long}\n{many}")).unwrap();
        assert_eq!(d.environment[0].len(), 48);
        assert_eq!(d.environment.len(), 8);
    }

    // ---- the draft flow, against a fake OpenAI-compatible server ----

    /// Answers each connection with the next canned reply and records the
    /// request bodies it saw. Enough HTTP/1.1 to satisfy reqwest.
    fn fake_openai_server(replies: Vec<&'static str>) -> (String, Arc<Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_by_server = seen.clone();
        std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let mut header_end = 0;
                while header_end == 0 {
                    let n = stream.read(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = pos + 4;
                    }
                }
                let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
                let length: usize = headers
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                    .unwrap_or(0);
                while buf.len() < header_end + length {
                    let n = stream.read(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                seen_by_server.lock().unwrap().push(String::from_utf8_lossy(&buf[header_end..]).to_string());
                let json = serde_json::json!({
                    "model": "fake-1",
                    "choices": [{ "message": { "role": "assistant", "content": reply } }],
                    "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                    json.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        (base_url, seen)
    }

    fn local(base_url: &str) -> LlmConfig {
        LlmConfig { protocol: Protocol::OpenaiCompatible, base_url: base_url.into(), api_key: None, model: "fake".into() }
    }

    #[tokio::test]
    async fn a_layout_miss_gets_one_corrective_retry_in_the_same_conversation() {
        let (base_url, seen) = fake_openai_server(vec![
            "Sure! The developer is trying to ship a release and the rollout hangs.",
            "INSIGHTS\n- intent: ship the release\nENVIRONMENT\n- Kubernetes 1.29",
        ]);
        let out = draft_insights(Some(&local(&base_url)), &InsightsInput { title: "Rollout hangs".into(), ..Default::default() }).await;

        assert_eq!(out.note, None);
        assert_eq!(out.draft.insights, "- intent: ship the release");
        assert_eq!(out.draft.environment, vec!["Kubernetes 1.29"]);
        assert_eq!(out.model.as_deref(), Some("fake-1"));
        assert_eq!((out.input_tokens, out.output_tokens), (20, 10), "both attempts are paid for");

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        let second: serde_json::Value = serde_json::from_str(&seen[1]).unwrap();
        let roles: Vec<&str> = second["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
        assert!(second["messages"][2]["content"].as_str().unwrap().starts_with("Sure! The developer"));
        assert!(second["messages"][3]["content"].as_str().unwrap().contains("did not follow the required layout"));
    }

    #[tokio::test]
    async fn two_layout_misses_leave_the_insights_empty_with_the_reason() {
        let (base_url, seen) = fake_openai_server(vec!["prose", "more prose"]);
        let out = draft_insights(Some(&local(&base_url)), &InsightsInput { title: "t".into(), ..Default::default() }).await;
        assert_eq!(out.draft, BriefDraft::default());
        assert!(out.note.unwrap().contains("did not follow the expected layout"));
        assert_eq!(seen.lock().unwrap().len(), 2, "exactly one retry, never more");
        assert_eq!((out.input_tokens, out.output_tokens), (20, 10));
    }

    #[tokio::test]
    async fn a_good_first_reply_needs_no_retry() {
        let (base_url, seen) = fake_openai_server(vec!["INSIGHTS\n- fine\nENVIRONMENT\n- none"]);
        let out = draft_insights(Some(&local(&base_url)), &InsightsInput { title: "t".into(), ..Default::default() }).await;
        assert_eq!(out.draft.insights, "- fine");
        assert!(out.draft.environment.is_empty());
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn no_config_is_an_empty_draft_with_a_note() {
        let out = draft_insights(None, &InsightsInput::default()).await;
        assert_eq!(out.draft, BriefDraft::default());
        assert!(out.note.unwrap().contains("No assistant is configured"));
        assert!(out.model.is_none());
    }

    #[tokio::test]
    async fn unreachable_model_is_an_empty_draft_with_a_note() {
        // A closed port: the transport fails fast and nothing is invented.
        let out = draft_insights(Some(&local("http://127.0.0.1:9/v1")), &InsightsInput { title: "t".into(), ..Default::default() }).await;
        assert_eq!(out.draft, BriefDraft::default());
        assert!(out.note.unwrap().starts_with("The assistant failed:"));
    }
}
