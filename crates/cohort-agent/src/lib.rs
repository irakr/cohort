//! Owner agent module.
//!
//! Runs inside the Cohort app on the owner's machine. Suggests artifacts for
//! the context picker and fingerprints the environment. Data produced here
//! stays local until the owner explicitly shares it.
//!
//! Detection is real and scoped to what the machine can cheaply reveal (see
//! [`scan`]): interactive terminal sessions with their working directories,
//! running or installed AI agents (with Claude Code session activity from its
//! transcripts), and the directories those point at. The detector daemon (P1)
//! deepens this with telemetry; the picker's manual "Add artifacts" covers
//! anything the scan cannot see.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

pub mod icons;
pub mod scan;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ArtifactCandidate {
    /// Stable id within the suggestion set, e.g. "a-claude".
    pub id: String,
    /// Picker kind: "terminal" | "file" | "ai_agent" | "custom".
    pub kind: String,
    /// 2-3 char icon badge, e.g. "CC", "YML".
    pub badge: String,
    pub label: String,
    pub detail: String,
    /// Shows the caution marker in the picker (e.g. path may hold secrets).
    pub warn: bool,
    /// App icon as a `data:image/png;base64,...` URI; None means the UI
    /// shows a placeholder badge.
    pub icon: Option<String>,
    /// Process id of the detected session/agent; None for paths and
    /// installed-only entries.
    #[ts(type = "number | null")]
    pub pid: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ArtifactGroup {
    /// "Terminals" | "Files" | "AI agents"
    pub title: String,
    pub items: Vec<ArtifactCandidate>,
}

/// The owner agent module. Everything defaults to read-only suggestions;
/// nothing is shared until the owner toggles it on in the picker.
pub trait AgentModule: Send + Sync {
    fn suggest_artifacts(&self) -> Vec<ArtifactGroup>;
    fn env_fingerprint(&self) -> Vec<String>;
}

/// Detect installed AI agent CLIs by their well-known home directories.
/// Only reports presence - it reads nothing from inside them.
pub fn agent_installs(home: &Path) -> Vec<ArtifactCandidate> {
    let known: [(&str, &str, &str, &str); 3] = [
        (".claude", "a-claude", "CC", "Claude Code"),
        (".cursor", "a-cursor", "Cu", "Cursor"),
        (".aider", "a-aider", "Ai", "Aider"),
    ];
    known
        .iter()
        .filter(|(dir, _, _, _)| home.join(dir).is_dir())
        .map(|(_, id, badge, label)| ArtifactCandidate {
            id: (*id).into(),
            kind: "ai_agent".into(),
            badge: (*badge).into(),
            label: (*label).into(),
            detail: "installed - no active session".into(),
            warn: false,
            icon: None,
            pid: None,
        })
        .collect()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The real local agent used by the base version: scans the process table,
/// tty shells, and agent transcripts on every call. See [`scan`].
pub struct LocalAgent;

impl AgentModule for LocalAgent {
    fn suggest_artifacts(&self) -> Vec<ArtifactGroup> {
        match home_dir() {
            Some(home) => scan::scan(&home),
            None => Vec::new(),
        }
    }

    fn env_fingerprint(&self) -> Vec<String> {
        vec![format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_installs_reports_only_present_dirs() {
        let base = std::env::temp_dir().join(format!("cohort-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join(".claude")).unwrap();

        let found = agent_installs(&base);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "Claude Code");
        assert_eq!(found[0].kind, "ai_agent");

        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        let found = agent_installs(&base);
        assert_eq!(found.len(), 2);

        std::fs::remove_dir_all(&base).unwrap();
        assert!(agent_installs(&base).is_empty());
    }

    #[test]
    fn live_scan_does_not_panic_and_serializes() {
        // Runs against the real machine: content varies, shape must hold.
        let groups = LocalAgent.suggest_artifacts();
        assert!(serde_json::to_string(&groups).is_ok());
        for group in &groups {
            assert!(!group.items.is_empty());
            for item in &group.items {
                assert!(["terminal", "file", "ai_agent"].contains(&item.kind.as_str()));
            }
        }
    }

    #[test]
    fn fingerprint_is_real_and_non_empty() {
        let fp = LocalAgent.env_fingerprint();
        assert_eq!(fp.len(), 1);
        assert!(fp[0].contains(std::env::consts::OS));
    }
}
