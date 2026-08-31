//! Owner agent module.
//!
//! Runs inside the Cohort app on the owner's machine. Suggests artifacts for
//! the context picker and fingerprints the environment. Data produced here
//! stays local until the owner explicitly shares it.
//!
//! The base version is a deterministic stub behind [`AgentModule`]; real
//! detection (terminal discovery, agent-session telemetry, git state) replaces
//! [`StubAgent`] later without touching the UI or the hub.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ArtifactCandidate {
    /// Stable id within the suggestion set, e.g. "t1", "f2".
    pub id: String,
    /// Picker kind: "terminal" | "file" | "ai_agent" | "custom".
    pub kind: String,
    /// 2-3 char icon badge, e.g. "iT", "YML", "CC".
    pub badge: String,
    pub label: String,
    pub detail: String,
    /// Shows the amber caution marker in the picker (e.g. path may hold secrets).
    pub warn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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

/// Deterministic stub used by the base version.
pub struct StubAgent;

impl AgentModule for StubAgent {
    fn suggest_artifacts(&self) -> Vec<ArtifactGroup> {
        fn c(id: &str, kind: &str, badge: &str, label: &str, detail: &str, warn: bool) -> ArtifactCandidate {
            ArtifactCandidate {
                id: id.into(),
                kind: kind.into(),
                badge: badge.into(),
                label: label.into(),
                detail: detail.into(),
                warn,
            }
        }
        vec![
            ArtifactGroup {
                title: "Terminals".into(),
                items: vec![
                    c("t1", "terminal", "iT", "iTerm2 (payments)", "last command: kubectl rollout status", false),
                    c("t2", "terminal", "VS", "VS Code (zsh)", "integrated terminal, 2 tabs", false),
                    c("t3", "terminal", ">_", "Terminal (ssh)", "ssh staging-02 - idle 18m", true),
                ],
            },
            ArtifactGroup {
                title: "Files".into(),
                items: vec![
                    c("f1", "file", "YML", "deployment.yaml", "k8s/payments - ref a3f9c1", false),
                    c("f2", "file", "YML", "kustomization.yaml", "k8s/payments - ref a3f9c1", false),
                    c("f3", "file", "YML", "values.yaml", "charts/payments - ref a3f9c1", true),
                ],
            },
            ArtifactGroup {
                title: "AI agents".into(),
                items: vec![
                    c("a1", "ai_agent", "CC", "Claude Code", "agent session active - 41 turns", false),
                    c("a2", "ai_agent", "Cu", "Cursor", "agent session idle - 12 turns", false),
                ],
            },
        ]
    }

    fn env_fingerprint(&self) -> Vec<String> {
        vec![
            "Kubernetes 1.29".into(),
            "Helm 3.14".into(),
            "registry.internal:5000".into(),
            "Linux amd64".into(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_are_stable_and_non_empty() {
        let a = StubAgent.suggest_artifacts();
        let b = StubAgent.suggest_artifacts();
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|g| !g.items.is_empty()));
        assert_eq!(serde_json::to_string(&a).is_ok(), true);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }

    #[test]
    fn fingerprint_non_empty() {
        assert!(!StubAgent.env_fingerprint().is_empty());
    }
}
