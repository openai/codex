use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_analytics::GuardianCommandSource as AnalyticsGuardianCommandSource;
use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewEventParams;
use codex_analytics::GuardianReviewFailureKind;
use codex_analytics::GuardianReviewRiskLevel as AnalyticsGuardianRiskLevel;
use codex_analytics::GuardianReviewTerminalStatus;
use codex_analytics::GuardianReviewTrigger;
use codex_analytics::GuardianReviewedAction;
use codex_features::Feature;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_shell_command::parse_command::shlex_join;

use crate::codex::Session;

use super::GUARDIAN_MAX_ACTION_STRING_TOKENS;
use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GuardianApprovalRequest;
use super::GuardianAssessment;
use super::prompt::guardian_truncate_text;
use super::review_session_analytics::GuardianReviewSessionReport;

pub(super) struct GuardianReviewAnalyticsInput<'a> {
    pub(super) review_id: String,
    pub(super) target_item_id: String,
    pub(super) turn_id: String,
    pub(super) trigger: GuardianReviewTrigger,
    pub(super) retry_reason: Option<String>,
    pub(super) delegated_review: bool,
    pub(super) reviewed_action: GuardianReviewedAction,
    pub(super) reviewed_action_truncated: bool,
    pub(super) decision: GuardianReviewDecision,
    pub(super) terminal_status: GuardianReviewTerminalStatus,
    pub(super) failure_kind: Option<GuardianReviewFailureKind>,
    pub(super) assessment: Option<&'a GuardianAssessment>,
    pub(super) report: Option<GuardianReviewSessionReport>,
    pub(super) started_at: u64,
    pub(super) completed_at: Option<u64>,
    pub(super) completion_latency_ms: Option<u64>,
}

pub(super) async fn track_guardian_review(
    session: &Session,
    input: GuardianReviewAnalyticsInput<'_>,
) {
    if !session.enabled(Feature::GeneralAnalytics) {
        return;
    }

    let client_metadata = session.app_server_client_metadata().await;
    let GuardianReviewAnalyticsInput {
        review_id,
        target_item_id,
        turn_id,
        trigger,
        retry_reason,
        delegated_review,
        reviewed_action,
        reviewed_action_truncated,
        decision,
        terminal_status,
        failure_kind,
        assessment,
        report,
        started_at,
        completed_at,
        completion_latency_ms,
    } = input;
    let (risk_score, risk_level, rationale) = assessment.map_or((None, None, None), |assessment| {
        (
            Some(assessment.risk_score),
            Some(analytics_risk_level(assessment.risk_level)),
            Some(assessment.rationale.clone()),
        )
    });
    let (
        guardian_thread_id,
        guardian_session_kind,
        guardian_model,
        guardian_reasoning_effort,
        had_prior_review_context,
        guardian_tool_call_counts,
        guardian_time_to_first_token_ms,
        token_usage,
    ) = match report {
        Some(report) => (
            Some(report.guardian_thread_id),
            Some(report.session_kind),
            report.guardian_model,
            report.guardian_reasoning_effort,
            Some(report.had_prior_review_context),
            report.tool_call_counts,
            report.time_to_first_token_ms,
            report.token_usage,
        ),
        None => (None, None, None, None, None, Default::default(), None, None),
    };
    let guardian_tool_call_count = guardian_tool_call_counts.total();
    session
        .services
        .analytics_events_client
        .track_guardian_review(GuardianReviewEventParams {
            thread_id: session.conversation_id.to_string(),
            turn_id,
            review_id,
            target_item_id,
            product_client_id: client_metadata.client_name,
            trigger,
            retry_reason,
            delegated_review,
            reviewed_action,
            reviewed_action_truncated,
            decision,
            terminal_status,
            failure_kind,
            risk_score,
            risk_level,
            rationale,
            guardian_thread_id,
            guardian_session_kind,
            guardian_model,
            guardian_reasoning_effort,
            had_prior_review_context,
            review_timeout_ms: duration_millis_u64(GUARDIAN_REVIEW_TIMEOUT),
            guardian_tool_call_count,
            guardian_tool_call_counts,
            guardian_time_to_first_token_ms,
            guardian_completion_latency_ms: completion_latency_ms,
            started_at,
            completed_at,
            input_tokens: token_usage.as_ref().map(|usage| usage.input_tokens),
            cached_input_tokens: token_usage.as_ref().map(|usage| usage.cached_input_tokens),
            output_tokens: token_usage.as_ref().map(|usage| usage.output_tokens),
            reasoning_output_tokens: token_usage
                .as_ref()
                .map(|usage| usage.reasoning_output_tokens),
            total_tokens: token_usage.as_ref().map(|usage| usage.total_tokens),
        });
}

pub(super) fn guardian_reviewed_action(
    request: &GuardianApprovalRequest,
) -> (GuardianReviewTrigger, GuardianReviewedAction, bool) {
    match request {
        GuardianApprovalRequest::Shell {
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            ..
        } => (
            GuardianReviewTrigger::Shell,
            GuardianReviewedAction::Shell {
                command: command.clone(),
                command_display: shlex_join(command),
                cwd: cwd.display().to_string(),
                sandbox_permissions: *sandbox_permissions,
                additional_permissions: additional_permissions.clone(),
                justification: justification.clone(),
            },
            false,
        ),
        GuardianApprovalRequest::ExecCommand {
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
            ..
        } => (
            GuardianReviewTrigger::UnifiedExec,
            GuardianReviewedAction::UnifiedExec {
                command: command.clone(),
                command_display: shlex_join(command),
                cwd: cwd.display().to_string(),
                sandbox_permissions: *sandbox_permissions,
                additional_permissions: additional_permissions.clone(),
                justification: justification.clone(),
                tty: *tty,
            },
            false,
        ),
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            source,
            program,
            argv,
            cwd,
            additional_permissions,
            ..
        } => (
            GuardianReviewTrigger::Execve,
            GuardianReviewedAction::Execve {
                source: analytics_command_source(*source),
                program: program.clone(),
                argv: argv.clone(),
                cwd: cwd.display().to_string(),
                additional_permissions: additional_permissions.clone(),
            },
            false,
        ),
        GuardianApprovalRequest::ApplyPatch {
            cwd, files, patch, ..
        } => {
            let (patch, truncated) = truncate_analytics_string(patch);
            (
                GuardianReviewTrigger::ApplyPatch,
                GuardianReviewedAction::ApplyPatch {
                    cwd: cwd.display().to_string(),
                    files: files
                        .iter()
                        .map(|path| path.to_path_buf().display().to_string())
                        .collect(),
                    patch: Some(patch),
                },
                truncated,
            )
        }
        GuardianApprovalRequest::NetworkAccess {
            target,
            host,
            protocol,
            port,
            ..
        } => (
            GuardianReviewTrigger::NetworkAccess,
            GuardianReviewedAction::NetworkAccess {
                target: target.clone(),
                host: host.clone(),
                protocol: *protocol,
                port: *port,
            },
            false,
        ),
        GuardianApprovalRequest::McpToolCall {
            server,
            tool_name,
            arguments,
            connector_id,
            connector_name,
            tool_title,
            ..
        } => {
            let (arguments, truncated) = arguments
                .clone()
                .map(truncate_analytics_json_value)
                .map_or((None, false), |(value, truncated)| (Some(value), truncated));
            (
                GuardianReviewTrigger::McpToolCall,
                GuardianReviewedAction::McpToolCall {
                    server: server.clone(),
                    tool_name: tool_name.clone(),
                    arguments,
                    connector_id: connector_id.clone(),
                    connector_name: connector_name.clone(),
                    tool_title: tool_title.clone(),
                },
                truncated,
            )
        }
    }
}

pub(super) fn now_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn analytics_risk_level(risk_level: GuardianRiskLevel) -> AnalyticsGuardianRiskLevel {
    match risk_level {
        GuardianRiskLevel::Low => AnalyticsGuardianRiskLevel::Low,
        GuardianRiskLevel::Medium => AnalyticsGuardianRiskLevel::Medium,
        GuardianRiskLevel::High => AnalyticsGuardianRiskLevel::High,
    }
}

fn analytics_command_source(source: GuardianCommandSource) -> AnalyticsGuardianCommandSource {
    match source {
        GuardianCommandSource::Shell => AnalyticsGuardianCommandSource::Shell,
        GuardianCommandSource::UnifiedExec => AnalyticsGuardianCommandSource::UnifiedExec,
    }
}

fn truncate_analytics_json_value(value: serde_json::Value) -> (serde_json::Value, bool) {
    match value {
        serde_json::Value::String(text) => {
            let (text, truncated) = truncate_analytics_string(&text);
            (serde_json::Value::String(text), truncated)
        }
        serde_json::Value::Array(values) => {
            let mut truncated = false;
            let values = values
                .into_iter()
                .map(|value| {
                    let (value, value_truncated) = truncate_analytics_json_value(value);
                    truncated |= value_truncated;
                    value
                })
                .collect();
            (serde_json::Value::Array(values), truncated)
        }
        serde_json::Value::Object(values) => {
            let mut truncated = false;
            let values = values
                .into_iter()
                .map(|(key, value)| {
                    let (value, value_truncated) = truncate_analytics_json_value(value);
                    truncated |= value_truncated;
                    (key, value)
                })
                .collect();
            (serde_json::Value::Object(values), truncated)
        }
        value => (value, false),
    }
}

fn truncate_analytics_string(text: &str) -> (String, bool) {
    let truncated = guardian_truncate_text(text, GUARDIAN_MAX_ACTION_STRING_TOKENS);
    let was_truncated = truncated != text;
    (truncated, was_truncated)
}
