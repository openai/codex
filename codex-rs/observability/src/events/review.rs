//! Review observation event definitions.

use crate::Observation;
use serde::Serialize;

/// Final decision returned by a review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approved,
    Denied,
    Aborted,
}

/// Terminal state of a review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTerminalStatus {
    Approved,
    Denied,
    Aborted {
        failure_reason: Option<ReviewFailureReason>,
    },
    TimedOut {
        failure_reason: Option<ReviewFailureReason>,
    },
    FailedClosed {
        failure_reason: Option<ReviewFailureReason>,
    },
}

/// Stable failure category for review terminals.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFailureReason {
    Timeout,
    Cancelled,
    PromptBuildError,
    SessionError,
    ParseError,
}

/// Source of the request sent for review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRequestSource {
    MainTurn,
    DelegatedSubagent,
}

/// Per-command sandbox override requested by the reviewed action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSandboxPermissions {
    UseDefault,
    RequireEscalated,
    WithAdditionalPermissions,
}

/// Additional filesystem permissions requested for a reviewed action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewFileSystemPermissions<'a> {
    pub read: Option<&'a [String]>,
    pub write: Option<&'a [String]>,
}

/// Additional network permissions requested for a reviewed action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewNetworkPermissions {
    pub enabled: Option<bool>,
}

/// Additional permissions requested for a reviewed action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewPermissionProfile<'a> {
    pub network: Option<ReviewNetworkPermissions>,
    pub file_system: Option<ReviewFileSystemPermissions<'a>>,
}

/// Source tool that produced an exec-style reviewed action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCommandSource {
    Shell,
    UnifiedExec,
}

/// Network protocol involved in a reviewed network access request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewNetworkApprovalProtocol {
    Http,
    Https,
    Socks5Tcp,
    Socks5Udp,
}

/// Action that was evaluated by a review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewedAction<'a> {
    Shell {
        command: &'a [String],
        command_display: &'a str,
        cwd: &'a str,
        sandbox_permissions: ReviewSandboxPermissions,
        additional_permissions: Option<ReviewPermissionProfile<'a>>,
        justification: Option<&'a str>,
    },
    UnifiedExec {
        command: &'a [String],
        command_display: &'a str,
        cwd: &'a str,
        sandbox_permissions: ReviewSandboxPermissions,
        additional_permissions: Option<ReviewPermissionProfile<'a>>,
        justification: Option<&'a str>,
        tty: bool,
    },
    ProcessExec {
        source: ReviewCommandSource,
        program: &'a str,
        argv: &'a [String],
        cwd: &'a str,
        additional_permissions: Option<ReviewPermissionProfile<'a>>,
    },
    ApplyPatch {
        cwd: &'a str,
        files: &'a [String],
    },
    NetworkAccess {
        target: &'a str,
        host: &'a str,
        protocol: ReviewNetworkApprovalProtocol,
        port: u16,
    },
    McpToolCall {
        server: &'a str,
        tool_name: &'a str,
        connector_id: Option<&'a str>,
        connector_name: Option<&'a str>,
        tool_title: Option<&'a str>,
    },
}

/// Risk level assigned by an automated reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// User authorization level observed by an automated reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

/// Policy outcome recommended by an automated reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewOutcome {
    Allow,
    Deny,
}

/// How guardian review obtained a model session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewSessionKind {
    TrunkNew,
    TrunkReused,
    EphemeralForked,
}

/// Guardian model session used to perform a review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GuardianReviewSession<'a> {
    pub guardian_thread_id: &'a str,
    pub session_kind: GuardianReviewSessionKind,
    pub model: &'a str,
    /// Absent when the selected model/provider has no explicit effort setting.
    pub reasoning_effort: Option<&'a str>,
    pub had_prior_review_context: bool,
}

/// Response produced by a user reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct UserReviewResponse {
    pub decision: ReviewDecision,
}

/// Response produced by guardian review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GuardianReviewResponse<'a> {
    pub decision: ReviewDecision,
    /// Terminal runtime state; non-success states carry their stable failure category.
    pub terminal_status: ReviewTerminalStatus,
    /// Absent when review stops before the model reports a risk classification.
    pub risk_level: Option<ReviewRiskLevel>,
    /// Absent when review stops before user authorization is assessed.
    pub user_authorization: Option<ReviewUserAuthorization>,
    /// Absent when review stops before a policy outcome is produced.
    pub outcome: Option<ReviewOutcome>,
    /// Model-authored rationale text returned by guardian review.
    pub rationale: Option<&'a str>,
    /// Absent when review fails before a guardian model session is created or reused.
    pub session: Option<GuardianReviewSession<'a>>,
    pub review_timeout_ms: u64,
    pub tool_call_count: u64,
    /// Absent when the guardian session did not stream model tokens.
    pub time_to_first_token_ms: Option<u64>,
    /// Absent when review ended before a model completion was received.
    pub completion_latency_ms: Option<u64>,
    /// Absent when the guardian model did not report token accounting.
    pub token_usage: Option<super::TurnTokenUsage>,
}

/// Reviewer response that completed a review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "reviewer", rename_all = "snake_case")]
pub enum ReviewResponse<'a> {
    User(UserReviewResponse),
    Guardian(GuardianReviewResponse<'a>),
}

/// Observation emitted when review of an action reaches a terminal state.
#[derive(Observation)]
#[observation(name = "review.completed", crate = "crate", uses = ["analytics"])]
pub struct ReviewCompleted<'a> {
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub review_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub target_item_id: &'a str,

    /// Absent on the first review attempt; present when this review retries an earlier attempt.
    #[obs(level = "basic", class = "operational")]
    pub retry_reason: Option<&'a str>,

    #[obs(level = "basic", class = "operational")]
    pub request_source: ReviewRequestSource,

    /// The reviewed action may contain command text, paths, or tool names.
    #[obs(level = "basic", class = "content")]
    pub reviewed_action: ReviewedAction<'a>,

    #[obs(level = "basic", class = "operational")]
    pub reviewed_action_truncated: bool,

    /// Contains the reviewer-specific terminal result.
    #[obs(level = "basic", class = "content")]
    pub response: ReviewResponse<'a>,

    #[obs(level = "basic", class = "operational")]
    pub started_at: i64,

    #[obs(level = "basic", class = "operational")]
    pub ended_at: i64,
}
