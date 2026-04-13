use std::time::Duration;

use codex_otel::GUARDIAN_REVIEW_COUNT_METRIC as OTEL_GUARDIAN_REVIEW_COUNT_METRIC;
use codex_otel::GUARDIAN_REVIEW_E2E_DURATION_METRIC as OTEL_GUARDIAN_REVIEW_E2E_DURATION_METRIC;
use codex_otel::GUARDIAN_REVIEW_PHASE_DURATION_METRIC as OTEL_GUARDIAN_REVIEW_PHASE_DURATION_METRIC;
use codex_otel::GUARDIAN_REVIEW_PROMPT_APPROX_TOKENS_METRIC as OTEL_GUARDIAN_REVIEW_PROMPT_APPROX_TOKENS_METRIC;
use codex_otel::GUARDIAN_REVIEW_PROMPT_TRANSCRIPT_ENTRIES_METRIC as OTEL_GUARDIAN_REVIEW_PROMPT_TRANSCRIPT_ENTRIES_METRIC;
use codex_otel::SessionTelemetry;

use super::GuardianApprovalRequest;
use super::prompt::GuardianPromptStats;

pub(crate) const GUARDIAN_REVIEW_COUNT_METRIC: &str = OTEL_GUARDIAN_REVIEW_COUNT_METRIC;
pub(crate) const GUARDIAN_REVIEW_DURATION_METRIC: &str = OTEL_GUARDIAN_REVIEW_E2E_DURATION_METRIC;
pub(crate) const GUARDIAN_REVIEW_PHASE_DURATION_METRIC: &str =
    OTEL_GUARDIAN_REVIEW_PHASE_DURATION_METRIC;
pub(crate) const GUARDIAN_REVIEW_PROMPT_APPROX_TOKENS_METRIC: &str =
    OTEL_GUARDIAN_REVIEW_PROMPT_APPROX_TOKENS_METRIC;
pub(crate) const GUARDIAN_REVIEW_PROMPT_TRANSCRIPT_ENTRIES_METRIC: &str =
    OTEL_GUARDIAN_REVIEW_PROMPT_TRANSCRIPT_ENTRIES_METRIC;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardianApprovalRequestSource {
    MainTurn,
    DelegatedSubagent,
}

impl GuardianApprovalRequestSource {
    pub(crate) fn as_tag(self) -> &'static str {
        match self {
            Self::MainTurn => "main_turn",
            Self::DelegatedSubagent => "delegated_subagent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardianReviewSessionMode {
    TrunkReused,
    TrunkSpawned,
    EphemeralBusyTrunk,
    EphemeralReuseKeyMismatch,
}

impl GuardianReviewSessionMode {
    pub(crate) fn as_tag(self) -> &'static str {
        match self {
            Self::TrunkReused => "trunk_reused",
            Self::TrunkSpawned => "trunk_spawned",
            Self::EphemeralBusyTrunk => "ephemeral_busy_trunk",
            Self::EphemeralReuseKeyMismatch => "ephemeral_reuse_key_mismatch",
        }
    }
}

pub(crate) fn guardian_action_kind(request: &GuardianApprovalRequest) -> &'static str {
    match request {
        GuardianApprovalRequest::Shell { .. } => "shell",
        GuardianApprovalRequest::ExecCommand { .. } => "exec_command",
        #[cfg(unix)]
        GuardianApprovalRequest::Execve { .. } => "execve",
        GuardianApprovalRequest::ApplyPatch { .. } => "apply_patch",
        GuardianApprovalRequest::NetworkAccess { .. } => "network_access",
        GuardianApprovalRequest::McpToolCall { .. } => "mcp_tool_call",
    }
}

pub(crate) fn guardian_bool_tag(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(crate) fn record_guardian_review_metrics(
    session_telemetry: &SessionTelemetry,
    action_kind: &str,
    request_source: GuardianApprovalRequestSource,
    result: &str,
    retry: &str,
    target_item: &str,
    duration: Duration,
) {
    let tags = [
        ("action_kind", action_kind),
        ("request_source", request_source.as_tag()),
        ("result", result),
        ("retry", retry),
        ("target_item", target_item),
    ];
    session_telemetry.counter(GUARDIAN_REVIEW_COUNT_METRIC, /*inc*/ 1, &tags);
    session_telemetry.record_duration(GUARDIAN_REVIEW_DURATION_METRIC, duration, &tags);
}

pub(crate) fn record_guardian_prompt_stats(
    session_telemetry: &SessionTelemetry,
    action_kind: &str,
    session_mode: GuardianReviewSessionMode,
    stats: GuardianPromptStats,
) {
    let session_mode_tag = session_mode.as_tag();
    let prompt_mode_tag = stats.prompt_mode.as_tag();
    let base_tags = [
        ("action_kind", action_kind),
        ("session_mode", session_mode_tag),
        ("prompt_mode", prompt_mode_tag),
    ];

    session_telemetry.histogram(
        GUARDIAN_REVIEW_PROMPT_APPROX_TOKENS_METRIC,
        saturating_i64_from_usize(stats.approx_prompt_tokens),
        &base_tags,
    );

    for (entry_scope, value) in [
        ("total", stats.total_transcript_entries),
        ("considered", stats.considered_transcript_entries),
        ("retained", stats.retained_transcript_entries),
    ] {
        session_telemetry.histogram(
            GUARDIAN_REVIEW_PROMPT_TRANSCRIPT_ENTRIES_METRIC,
            saturating_i64_from_usize(value),
            &[
                ("action_kind", action_kind),
                ("session_mode", session_mode_tag),
                ("prompt_mode", prompt_mode_tag),
                ("entry_scope", entry_scope),
            ],
        );
    }
}

fn saturating_i64_from_usize(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
