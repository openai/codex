//! Turn lifecycle observation event definitions.

use crate::Observation;
use serde::Serialize;

/// Terminal turn status after Codex stops working on a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

/// How a turn was submitted for execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSubmissionType {
    Default,
    Queued,
}

/// Filesystem sandbox mode resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    FullAccess,
    ReadOnly,
    WorkspaceWrite,
    ExternalSandbox,
}

/// Approval policy resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Legacy name for the policy that asks unless an action is trusted.
    Untrusted,
    OnFailure,
    OnRequest,
    Granular,
    Never,
}

/// Destination that reviews approval requests for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalReviewer {
    User,
    GuardianSubagent,
}

/// Collaboration mode resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationMode {
    Default,
    Plan,
}

/// Reasoning effort resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

/// Reasoning summary mode resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

/// Service tier resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    Fast,
    Flex,
}

/// Response personality resolved for a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Personality {
    None,
    Friendly,
    Pragmatic,
}

/// Configuration resolved before a turn starts executing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TurnConfig<'a> {
    pub num_input_images: usize,
    /// Absent when the caller cannot distinguish default from queued submission.
    pub submission_type: Option<TurnSubmissionType>,
    pub ephemeral: bool,
    pub model: &'a str,
    pub model_provider: &'a str,
    pub sandbox_mode: SandboxMode,
    /// Kept separate from sandbox mode because analytics reports network
    /// capability as an independent resolved setting.
    pub sandbox_network_access: bool,
    /// Absent when the selected model/provider has no explicit effort setting.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// None means no summary setting was resolved; Some(None) means summaries
    /// were explicitly disabled.
    pub reasoning_summary: Option<ReasoningSummary>,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: ApprovalPolicy,
    pub approval_reviewer: ApprovalReviewer,
    pub collaboration_mode: CollaborationMode,
    /// Absent when no personality setting was resolved.
    pub personality: Option<Personality>,
    pub is_first_turn: bool,
}

/// Observation emitted when execution of a turn starts.
#[derive(Observation)]
#[observation(name = "turn.started", crate = "crate", uses = ["analytics"])]
pub struct TurnStarted<'a> {
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    pub config: TurnConfig<'a>,

    /// Unix timestamp in seconds when the turn started.
    #[obs(level = "basic", class = "operational")]
    pub started_at: i64,
}

/// Token accounting reported for a completed turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TurnTokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

/// Observation emitted when a turn reaches a terminal state.
#[derive(Observation)]
#[observation(name = "turn.ended", crate = "crate", uses = ["analytics"])]
pub struct TurnEnded<'a> {
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    pub status: TurnStatus,

    /// Absent when a turn ends before provider usage is available.
    #[obs(level = "basic", class = "operational")]
    pub token_usage: Option<TurnTokenUsage>,

    /// Unix timestamp in seconds when the turn ended.
    #[obs(level = "basic", class = "operational")]
    pub ended_at: i64,

    #[obs(level = "basic", class = "operational")]
    pub duration_ms: i64,
}
