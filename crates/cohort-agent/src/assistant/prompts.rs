//! One function per task, each returning a structured [`Prompt`]. The
//! system turn carries the role, the rules and the reply layout; the user
//! turn carries the context in delimited blocks and ends with the ask.
//! No template engine: the shape of a prompt is code, and tests read it.
//!
//! Reply protocol. Replies are plain text in named sections, never JSON:
//! models routinely break JSON when the payload is prose (raw newlines
//! inside strings, stray fences), and the content here IS prose - bullets
//! and short chips. Two headers on their own lines, bullets beneath:
//!
//! ```text
//! INSIGHTS
//! - ...
//! ENVIRONMENT
//! - ...
//! ```
//!
//! The parser in the parent module tolerates decoration around the headers
//! and bullet styles; a reply with no INSIGHTS header at all gets exactly
//! one corrective retry (see [`insights_retry`]).

use super::context::FileContext;
use super::InsightsInput;
use cohort_llm::{Message, Prompt, Role};

pub const INSIGHTS_SYSTEM: &str = "You draft the insights for a Cohort assist: a short analysis of what \
a stuck developer intends to do, written from their title, their own problem description, and \
excerpts of the artifacts they chose to share from their machine.\n\
Reply in exactly this plain-text layout and nothing else - no preamble, no closing remark, no \
code fences, no JSON:\n\
INSIGHTS\n\
- one short bullet\n\
- another short bullet\n\
ENVIRONMENT\n\
- one short chip\n\
- another short chip\n\
Under INSIGHTS write 2 to 5 bullets: the intent, what the artifacts show, and the most plausible \
direction. Under ENVIRONMENT write 3 to 6 bullets naming the languages, frameworks, tools and \
versions the inputs actually show, each under 40 characters, like a version chip - not the \
machine, the people, or the situation. State ONLY what the inputs say - never invent \
failures, file contents, versions, or secrets; <redacted> marks a masked secret, leave it be. \
If a section has nothing truthful to say, write a single bullet: - none";

pub const INSIGHTS_RETRY: &str = "That reply did not follow the required layout, so none of it could be \
used. Reply again with exactly two sections and nothing else:\nINSIGHTS\n- ...\nENVIRONMENT\n- ...";

pub fn insights(input: &InsightsInput, files: &FileContext) -> Prompt {
    let mut user = String::new();
    block(&mut user, "title", if input.title.trim().is_empty() { "(untitled)" } else { input.title.trim() });
    block(
        &mut user,
        "description",
        if input.description.trim().is_empty() { "(none)" } else { input.description.trim() },
    );

    user.push_str("<artifacts>\n");
    if input.artifacts.is_empty() {
        user.push_str("(none shared)\n");
    }
    for a in &input.artifacts {
        user.push_str(&format!("- [{}] {} ({})\n", a.kind, a.label, a.detail));
    }
    user.push_str("</artifacts>\n");

    for f in &files.blocks {
        user.push_str(&format!("<file path=\"{}\" chars=\"{}\">\n", f.path, f.total_chars));
        user.push_str(&f.content);
        if !f.content.ends_with('\n') {
            user.push('\n');
        }
        if f.truncated {
            user.push_str(&format!(
                "[... truncated, {} more chars]\n",
                f.total_chars.saturating_sub(f.content.chars().count())
            ));
        }
        user.push_str("</file>\n");
    }
    if !files.not_included.is_empty() {
        block(&mut user, "not-included", &summary(&files.not_included, "did not fit the context budget"));
    }
    if !files.skipped.is_empty() {
        block(
            &mut user,
            "skipped",
            &summary(&files.skipped, "skipped as low-value context (lockfiles, minified bundles, maps)"),
        );
    }
    if !files.notes.is_empty() {
        block(&mut user, "snapshot-notes", &files.notes.join("\n"));
    }

    user.push_str("Write the INSIGHTS and ENVIRONMENT sections.");
    Prompt::single(INSIGHTS_SYSTEM, user)
}

/// The same conversation continued: the model's unusable reply as its own
/// turn, then the correction. Keeping the original context in place means
/// the model fixes the layout rather than re-deriving the analysis.
pub fn insights_retry(first: &Prompt, reply: &str) -> Prompt {
    let mut messages = first.messages.clone();
    messages.push(Message {
        role: Role::Assistant,
        // Some servers reject an empty assistant turn.
        content: if reply.trim().is_empty() { "(empty reply)".into() } else { reply.to_string() },
    });
    messages.push(Message { role: Role::User, content: INSIGHTS_RETRY.into() });
    Prompt { system: first.system.clone(), messages }
}

fn block(out: &mut String, tag: &str, body: &str) {
    out.push_str(&format!("<{tag}>\n{body}\n</{tag}>\n"));
}

/// "N file(s) <why>: a, b, c, and 40 more" - the count and a few names,
/// never every path; a long list of absolute paths once cost more tokens
/// than the files it described.
fn summary(paths: &[String], why: &str) -> String {
    const SHOW: usize = 8;
    let shown: Vec<&str> = paths.iter().take(SHOW).map(String::as_str).collect();
    let mut s = format!("{} file(s) {why}: {}", paths.len(), shown.join(", "));
    if paths.len() > SHOW {
        s.push_str(&format!(", and {} more", paths.len() - SHOW));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::context::FileBlock;
    use crate::assistant::ArtifactRef;

    #[test]
    fn insights_prompt_puts_rules_in_system_and_context_in_user() {
        let input = InsightsInput {
            title: "Rollout hangs on an image pull".into(),
            description: "The pod never becomes ready.".into(),
            artifacts: vec![
                ArtifactRef { kind: "file".into(), label: "deployment.yaml".into(), detail: "k8s/deployment.yaml".into() },
                ArtifactRef { kind: "ai_agent".into(), label: "Claude Code".into(), detail: "agent session active".into() },
            ],
        };
        let files = FileContext {
            blocks: vec![FileBlock {
                path: "k8s/deployment.yaml".into(),
                content: "image: api:1.9.4".into(),
                truncated: true,
                total_chars: 900,
            }],
            not_included: vec!["charts/values.yaml".into()],
            skipped: vec!["Cargo.lock".into()],
            notes: vec![],
        };
        let prompt = insights(&input, &files);

        assert!(prompt.system.contains("INSIGHTS\n- one short bullet"));
        assert!(prompt.system.contains("ENVIRONMENT\n- one short chip"));
        assert!(prompt.system.contains("no JSON"));
        assert!(prompt.system.contains("never invent"));
        assert_eq!(prompt.messages.len(), 1);
        let user = &prompt.messages[0].content;
        assert!(user.contains("<title>\nRollout hangs on an image pull\n</title>"));
        assert!(user.contains("<description>\nThe pod never becomes ready.\n</description>"));
        assert!(user.contains("- [file] deployment.yaml (k8s/deployment.yaml)"));
        assert!(user.contains("- [ai_agent] Claude Code (agent session active)"));
        assert!(user.contains("<file path=\"k8s/deployment.yaml\" chars=\"900\">\nimage: api:1.9.4\n"));
        assert!(user.contains("[... truncated, 884 more chars]"));
        assert!(user.contains("<not-included>\n1 file(s) did not fit the context budget: charts/values.yaml\n</not-included>"));
        assert!(user.contains("<skipped>\n1 file(s) skipped as low-value context (lockfiles, minified bundles, maps): Cargo.lock\n</skipped>"));
        assert!(user.ends_with("Write the INSIGHTS and ENVIRONMENT sections."));
    }

    #[test]
    fn long_exclusion_lists_are_summarised_not_dumped() {
        let many: Vec<String> = (1..=53).map(|i| format!("Cohort/some/deep/path/file{i}.rs")).collect();
        let s = summary(&many, "did not fit the context budget");
        assert!(s.starts_with("53 file(s) did not fit the context budget: Cohort/some/deep/path/file1.rs, "));
        assert!(s.ends_with("file8.rs, and 45 more"));
        assert!(!s.contains("file9.rs"));
    }

    #[test]
    fn thin_input_is_labelled_not_padded() {
        let prompt = insights(&InsightsInput::default(), &FileContext::default());
        let user = &prompt.messages[0].content;
        assert!(user.contains("<title>\n(untitled)\n</title>"));
        assert!(user.contains("<description>\n(none)\n</description>"));
        assert!(user.contains("(none shared)"));
        assert!(!user.contains("<file"));
        assert!(!user.contains("<not-included>"));
    }

    #[test]
    fn retry_continues_the_same_conversation() {
        let first = insights(&InsightsInput { title: "t".into(), ..Default::default() }, &FileContext::default());
        let retry = insights_retry(&first, "Sure! Here is my analysis in prose.");
        assert_eq!(retry.system, first.system);
        assert_eq!(retry.messages.len(), 3);
        assert_eq!(retry.messages[0], first.messages[0]);
        assert_eq!(retry.messages[1].role, Role::Assistant);
        assert_eq!(retry.messages[1].content, "Sure! Here is my analysis in prose.");
        assert_eq!(retry.messages[2].role, Role::User);
        assert!(retry.messages[2].content.starts_with("That reply did not follow the required layout"));

        let blank = insights_retry(&first, "   ");
        assert_eq!(blank.messages[1].content, "(empty reply)");
    }
}
