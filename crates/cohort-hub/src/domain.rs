//! Wire types. These serde structs ARE the API contract; `make types` exports
//! them to TypeScript via ts-rs. Timestamps travel as RFC3339 strings.
//!
//! Asymmetry rule (project plan section 8): [`MyRecord`] has no field that could
//! carry an aggregate of help received - inbound help appears only as names
//! on individual assists. Keep it that way.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct User {
    pub id: String,
    pub name: String,
    pub initials: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum AssistStatus {
    Open,
    Dormant,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum Category {
    Broken,
    Environment,
    Approach,
    Review,
    Knowledge,
    AgentLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum Outcome {
    Resolved,
    WorkedAround,
    Abandoned,
    SelfResolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum ScopeKind {
    Comment,
    LiveDebug,
    File,
    Terminal,
    Agents,
    Ssh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", ts(export))]
pub enum ScopeStatus {
    Pending,
    Approved,
    Denied,
}

/// An artifact the owner selected or added when opening the assist.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct AssistArtifact {
    pub id: String,
    /// "terminal" | "file" | "ai_agent" | "custom"
    pub kind: String,
    pub label: String,
    pub detail: String,
    /// App icon as a data:image/png;base64 URI, when the scan resolved one.
    #[serde(default)]
    pub icon: Option<String>,
    /// Process id at share time, for sessions and agents.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub pid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct AssistSummary {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub title: String,
    pub status: AssistStatus,
    pub category: Option<Category>,
    pub tags: Vec<String>,
    /// "Anonymous" when the assist is flagged anonymous and the viewer is not the owner.
    pub owner_name: String,
    /// Responder NAMES, never a count (project plan section 11.6).
    pub responder_names: Vec<String>,
    pub created_at: String,
    pub is_mine: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ScopeRequest {
    #[ts(type = "number")]
    pub id: i64,
    pub assist_ref: String,
    pub requester_id: String,
    pub requester_name: String,
    pub kind: ScopeKind,
    pub target: Option<String>,
    pub reason: String,
    pub status: ScopeStatus,
    /// Request payload, e.g. the responder's SSH public key on ssh requests.
    #[serde(default)]
    pub payload: Option<String>,
    #[ts(type = "number | null")]
    pub ttl_minutes: Option<i64>,
    pub created_at: String,
    pub decided_at: Option<String>,
}

/// A live grant, derived from an approved, unexpired scope request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct Grant {
    #[ts(type = "number")]
    pub scope_request_id: i64,
    pub kind: ScopeKind,
    pub target: Option<String>,
    pub granted_to_id: String,
    pub granted_to_name: String,
    /// None = until the assist closes.
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct AssistDetail {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub title: String,
    pub status: AssistStatus,
    pub category: Option<Category>,
    pub tags: Vec<String>,
    pub owner_id: String,
    pub owner_name: String,
    pub anonymous: bool,
    /// Owner-written problem statement (markdown).
    pub description: String,
    /// AI-drafted analysis of the shared artifacts; empty until the Cohort AI
    /// integration is enabled (the UI shows N/A).
    pub insights: String,
    pub environment: Vec<String>,
    pub artifacts: Vec<AssistArtifact>,
    pub responders: Vec<User>,
    pub scope_requests: Vec<ScopeRequest>,
    pub grants: Vec<Grant>,
    /// What the owner's engine currently sees (see CatalogUpload); empty
    /// until the owner's app publishes while viewing the assist.
    pub catalog: Vec<AssistArtifact>,
    pub catalog_at: Option<String>,
    pub viewer_is_owner: bool,
    pub viewer_is_responder: bool,
    pub created_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CreateAssist {
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<Category>,
    #[serde(default)]
    pub anonymous: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub insights: String,
    #[serde(default)]
    pub environment: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<AssistArtifact>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CreateScopeRequest {
    pub kind: ScopeKind,
    #[serde(default)]
    pub target: Option<String>,
    pub reason: String,
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub ttl_minutes: Option<i64>,
}

/// Body of an approve decision. For ssh requests the owner supplies the
/// connection target (user@host) here.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct DecideScopeRequest {
    #[serde(default)]
    pub target: Option<String>,
}

/// Owner-published snapshot of what their engine currently sees (running
/// terminals and agents, suggested paths), for the responder's request wizard.
#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CatalogUpload {
    pub items: Vec<AssistArtifact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct RecordFields {
    #[serde(default)]
    pub symptom: String,
    #[serde(default)]
    pub env_fingerprint: String,
    #[serde(default)]
    pub scopes_that_mattered: String,
    #[serde(default)]
    pub dead_ends: String,
    #[serde(default)]
    pub fix: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CloseAssist {
    pub outcome: Outcome,
    #[serde(default)]
    pub credited_user_ids: Vec<String>,
    #[serde(default)]
    pub record: RecordFields,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ResolutionRecord {
    pub assist_ref: String,
    pub outcome: Outcome,
    pub symptom: String,
    pub env_fingerprint: String,
    pub scopes_that_mattered: String,
    pub dead_ends: String,
    pub fix: String,
    pub created_at: String,
}

/// One row in "My assists" on the record screen. Owned and responded assists
/// both appear; `responder_names` on an owned row is inbound help shown as
/// names per assist - the only form inbound help ever takes.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct MyAssistRow {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub title: String,
    pub status: AssistStatus,
    /// "owner" | "responder"
    pub role: String,
    pub responder_names: Vec<String>,
    pub outcome: Option<Outcome>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CreditRow {
    pub assist_ref: String,
    pub title: String,
    pub outcome: Option<Outcome>,
    pub from_owner_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct AiAgentUsage {
    pub name: String,
    pub model: String,
    #[ts(type = "number")]
    pub share_pct: i64,
    pub tokens: String,
    pub spend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct AiUsageRange {
    /// "7d" | "30d" | "90d"
    pub range: String,
    pub tokens: String,
    pub spend: String,
    pub longest_stall: String,
    pub agents: Vec<AiAgentUsage>,
}

/// Private contribution record. Outbound help accumulates; inbound help never
/// aggregates - there is deliberately no field for it here.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct MyRecord {
    pub user: User,
    #[ts(type = "number")]
    pub credits_earned: i64,
    #[ts(type = "number")]
    pub responses_count: i64,
    #[ts(type = "number")]
    pub records_count: i64,
    pub my_assists: Vec<MyAssistRow>,
    pub credits_rows: Vec<CreditRow>,
    pub ai_usage: Vec<AiUsageRange>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct CreateUser {
    pub name: String,
}

/// One event for the current user, derived from the hub's tables on read.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct HubNotification {
    /// Synthetic, stable per event (e.g. "req-12-decided").
    pub id: String,
    /// "scope_requested" | "scope_decided" | "comment" | "responder_joined" | "credited"
    pub kind: String,
    pub assist_ref: String,
    pub assist_title: String,
    pub actor_name: String,
    pub message: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct NotificationsResponse {
    /// Pass back as `since` on the next poll.
    pub now: String,
    pub notifications: Vec<HubNotification>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct DraftBriefRequest {
    #[serde(default)]
    pub title: String,
    /// Owner-written problem statement, source material for the analysis.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub artifacts: Vec<AssistArtifact>,
}

/// Draft produced by the Cohort AI integration. Without an API key both
/// fields stay empty - nothing is invented; the UI shows Insights as N/A.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct BriefDraft {
    /// Short markdown bullets on what the owner intends and what the
    /// artifacts show.
    pub insights: String,
    pub environment: Vec<String>,
}

// ---- Live data (seeded per assist; later streamed by the owner agent) ----

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct FileNode {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub children: Vec<FileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct ChatMsg {
    pub who: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct LiveData {
    #[serde(default)]
    pub file_tree: Vec<FileNode>,
    #[serde(default)]
    pub files: HashMap<String, String>,
    #[serde(default)]
    pub terminal_tabs: Vec<String>,
    #[serde(default)]
    pub terminal_feed: Vec<String>,
    #[serde(default)]
    pub agent_chat: Vec<ChatMsg>,
}
