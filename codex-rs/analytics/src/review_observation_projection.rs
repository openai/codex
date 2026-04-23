//! Projection from review observations into the current analytics schema.

use crate::events::GuardianApprovalRequestSource as AnalyticsGuardianApprovalRequestSource;
use crate::events::GuardianCommandSource as AnalyticsGuardianCommandSource;
use crate::events::GuardianReviewDecision as AnalyticsGuardianReviewDecision;
use crate::events::GuardianReviewEventParams;
use crate::events::GuardianReviewFailureReason as AnalyticsGuardianReviewFailureReason;
use crate::events::GuardianReviewOutcome as AnalyticsGuardianReviewOutcome;
use crate::events::GuardianReviewRiskLevel as AnalyticsGuardianReviewRiskLevel;
use crate::events::GuardianReviewSessionKind as AnalyticsGuardianReviewSessionKind;
use crate::events::GuardianReviewTerminalStatus as AnalyticsGuardianReviewTerminalStatus;
use crate::events::GuardianReviewUserAuthorization as AnalyticsGuardianReviewUserAuthorization;
use crate::events::GuardianReviewedAction as AnalyticsGuardianReviewedAction;
use codex_observability::events;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxPermissions;

/// Projects a generic review completion into the legacy guardian analytics event.
///
/// This preserves the existing guardian analytics payload, including reviewed
/// action details and guardian rationale that are explicitly marked for
/// analytics on the observation fields.
///
/// User review responses intentionally return None: the current analytics
/// schema only has a guardian review event, while the shared observation is
/// generic enough to represent both user and guardian review completions.
pub(crate) fn legacy_guardian_review_event(
    observation: events::ReviewCompleted<'_>,
) -> Option<GuardianReviewEventParams> {
    let events::ReviewResponse::Guardian(response) = observation.response else {
        return None;
    };
    let (terminal_status, failure_reason) = match response.terminal_status {
        events::ReviewTerminalStatus::Approved => {
            (AnalyticsGuardianReviewTerminalStatus::Approved, None)
        }
        events::ReviewTerminalStatus::Denied => {
            (AnalyticsGuardianReviewTerminalStatus::Denied, None)
        }
        events::ReviewTerminalStatus::Aborted { failure_reason } => (
            AnalyticsGuardianReviewTerminalStatus::Aborted,
            failure_reason.map(review_failure_reason),
        ),
        events::ReviewTerminalStatus::TimedOut { failure_reason } => (
            AnalyticsGuardianReviewTerminalStatus::TimedOut,
            failure_reason.map(review_failure_reason),
        ),
        events::ReviewTerminalStatus::FailedClosed { failure_reason } => (
            AnalyticsGuardianReviewTerminalStatus::FailedClosed,
            failure_reason.map(review_failure_reason),
        ),
    };
    let token_usage = response.token_usage;
    let guardian_session = response.session;

    Some(GuardianReviewEventParams {
        thread_id: observation.thread_id.to_string(),
        turn_id: observation.turn_id.to_string(),
        review_id: observation.review_id.to_string(),
        target_item_id: observation.target_item_id.to_string(),
        retry_reason: observation.retry_reason.map(str::to_string),
        approval_request_source: match observation.request_source {
            events::ReviewRequestSource::MainTurn => {
                AnalyticsGuardianApprovalRequestSource::MainTurn
            }
            events::ReviewRequestSource::DelegatedSubagent => {
                AnalyticsGuardianApprovalRequestSource::DelegatedSubagent
            }
        },
        reviewed_action: reviewed_action(observation.reviewed_action),
        reviewed_action_truncated: observation.reviewed_action_truncated,
        decision: match response.decision {
            events::ReviewDecision::Approved => AnalyticsGuardianReviewDecision::Approved,
            events::ReviewDecision::Denied => AnalyticsGuardianReviewDecision::Denied,
            events::ReviewDecision::Aborted => AnalyticsGuardianReviewDecision::Aborted,
        },
        terminal_status,
        failure_reason,
        risk_level: response.risk_level.map(|risk_level| match risk_level {
            events::ReviewRiskLevel::Low => AnalyticsGuardianReviewRiskLevel::Low,
            events::ReviewRiskLevel::Medium => AnalyticsGuardianReviewRiskLevel::Medium,
            events::ReviewRiskLevel::High => AnalyticsGuardianReviewRiskLevel::High,
            events::ReviewRiskLevel::Critical => AnalyticsGuardianReviewRiskLevel::Critical,
        }),
        user_authorization: response.user_authorization.map(|user_authorization| {
            match user_authorization {
                events::ReviewUserAuthorization::Unknown => {
                    AnalyticsGuardianReviewUserAuthorization::Unknown
                }
                events::ReviewUserAuthorization::Low => {
                    AnalyticsGuardianReviewUserAuthorization::Low
                }
                events::ReviewUserAuthorization::Medium => {
                    AnalyticsGuardianReviewUserAuthorization::Medium
                }
                events::ReviewUserAuthorization::High => {
                    AnalyticsGuardianReviewUserAuthorization::High
                }
            }
        }),
        outcome: response.outcome.map(|outcome| match outcome {
            events::ReviewOutcome::Allow => AnalyticsGuardianReviewOutcome::Allow,
            events::ReviewOutcome::Deny => AnalyticsGuardianReviewOutcome::Deny,
        }),
        rationale: response.rationale.map(str::to_string),
        guardian_thread_id: guardian_session.map(|session| session.guardian_thread_id.to_string()),
        guardian_session_kind: guardian_session.map(|session| match session.session_kind {
            events::GuardianReviewSessionKind::TrunkNew => {
                AnalyticsGuardianReviewSessionKind::TrunkNew
            }
            events::GuardianReviewSessionKind::TrunkReused => {
                AnalyticsGuardianReviewSessionKind::TrunkReused
            }
            events::GuardianReviewSessionKind::EphemeralForked => {
                AnalyticsGuardianReviewSessionKind::EphemeralForked
            }
        }),
        guardian_model: guardian_session.map(|session| session.model.to_string()),
        guardian_reasoning_effort: guardian_session
            .and_then(|session| session.reasoning_effort.map(str::to_string)),
        had_prior_review_context: guardian_session.map(|session| session.had_prior_review_context),
        review_timeout_ms: response.review_timeout_ms,
        tool_call_count: response.tool_call_count,
        time_to_first_token_ms: response.time_to_first_token_ms,
        completion_latency_ms: response.completion_latency_ms,
        started_at: u64::try_from(observation.started_at).unwrap_or_default(),
        completed_at: Some(u64::try_from(observation.ended_at).unwrap_or_default()),
        input_tokens: token_usage.map(|token_usage| token_usage.input_tokens),
        cached_input_tokens: token_usage.map(|token_usage| token_usage.cached_input_tokens),
        output_tokens: token_usage.map(|token_usage| token_usage.output_tokens),
        reasoning_output_tokens: token_usage.map(|token_usage| token_usage.reasoning_output_tokens),
        total_tokens: token_usage.map(|token_usage| token_usage.total_tokens),
    })
}

fn reviewed_action(action: events::ReviewedAction<'_>) -> AnalyticsGuardianReviewedAction {
    match action {
        events::ReviewedAction::Shell {
            command,
            command_display,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
        } => AnalyticsGuardianReviewedAction::Shell {
            command: command.to_vec(),
            command_display: command_display.to_string(),
            cwd: cwd.to_string(),
            sandbox_permissions: sandbox_permissions_for_review(sandbox_permissions),
            additional_permissions: additional_permissions.and_then(permission_profile_for_review),
            justification: justification.map(str::to_string),
        },
        events::ReviewedAction::UnifiedExec {
            command,
            command_display,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        } => AnalyticsGuardianReviewedAction::UnifiedExec {
            command: command.to_vec(),
            command_display: command_display.to_string(),
            cwd: cwd.to_string(),
            sandbox_permissions: sandbox_permissions_for_review(sandbox_permissions),
            additional_permissions: additional_permissions.and_then(permission_profile_for_review),
            justification: justification.map(str::to_string),
            tty,
        },
        events::ReviewedAction::ProcessExec {
            source,
            program,
            argv,
            cwd,
            additional_permissions,
        } => AnalyticsGuardianReviewedAction::Execve {
            source: match source {
                events::ReviewCommandSource::Shell => AnalyticsGuardianCommandSource::Shell,
                events::ReviewCommandSource::UnifiedExec => {
                    AnalyticsGuardianCommandSource::UnifiedExec
                }
            },
            program: program.to_string(),
            argv: argv.to_vec(),
            cwd: cwd.to_string(),
            additional_permissions: additional_permissions.and_then(permission_profile_for_review),
        },
        events::ReviewedAction::ApplyPatch { cwd, files } => {
            AnalyticsGuardianReviewedAction::ApplyPatch {
                cwd: cwd.to_string(),
                files: files.to_vec(),
            }
        }
        events::ReviewedAction::NetworkAccess {
            target,
            host,
            protocol,
            port,
        } => AnalyticsGuardianReviewedAction::NetworkAccess {
            target: target.to_string(),
            host: host.to_string(),
            protocol: match protocol {
                events::ReviewNetworkApprovalProtocol::Http => NetworkApprovalProtocol::Http,
                events::ReviewNetworkApprovalProtocol::Https => NetworkApprovalProtocol::Https,
                events::ReviewNetworkApprovalProtocol::Socks5Tcp => {
                    NetworkApprovalProtocol::Socks5Tcp
                }
                events::ReviewNetworkApprovalProtocol::Socks5Udp => {
                    NetworkApprovalProtocol::Socks5Udp
                }
            },
            port,
        },
        events::ReviewedAction::McpToolCall {
            server,
            tool_name,
            connector_id,
            connector_name,
            tool_title,
        } => AnalyticsGuardianReviewedAction::McpToolCall {
            server: server.to_string(),
            tool_name: tool_name.to_string(),
            connector_id: connector_id.map(str::to_string),
            connector_name: connector_name.map(str::to_string),
            tool_title: tool_title.map(str::to_string),
        },
    }
}

fn sandbox_permissions_for_review(
    sandbox_permissions: events::ReviewSandboxPermissions,
) -> SandboxPermissions {
    match sandbox_permissions {
        events::ReviewSandboxPermissions::UseDefault => SandboxPermissions::UseDefault,
        events::ReviewSandboxPermissions::RequireEscalated => SandboxPermissions::RequireEscalated,
        events::ReviewSandboxPermissions::WithAdditionalPermissions => {
            SandboxPermissions::WithAdditionalPermissions
        }
    }
}

fn permission_profile_for_review(
    profile: events::ReviewPermissionProfile<'_>,
) -> Option<PermissionProfile> {
    // Keep observability independent of codex-protocol types. The protocol
    // serde shape is the compatibility boundary for this nested payload.
    let value = serde_json::to_value(profile).ok()?;
    serde_json::from_value(value).ok()
}

fn review_failure_reason(
    failure_reason: events::ReviewFailureReason,
) -> AnalyticsGuardianReviewFailureReason {
    match failure_reason {
        events::ReviewFailureReason::Timeout => AnalyticsGuardianReviewFailureReason::Timeout,
        events::ReviewFailureReason::Cancelled => AnalyticsGuardianReviewFailureReason::Cancelled,
        events::ReviewFailureReason::PromptBuildError => {
            AnalyticsGuardianReviewFailureReason::PromptBuildError
        }
        events::ReviewFailureReason::SessionError => {
            AnalyticsGuardianReviewFailureReason::SessionError
        }
        events::ReviewFailureReason::ParseError => AnalyticsGuardianReviewFailureReason::ParseError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn projected_reviewed_action(action: events::ReviewedAction<'_>) -> serde_json::Value {
        let event =
            legacy_guardian_review_event(review_completed_with_action(action)).expect("project");
        serde_json::to_value(event.reviewed_action).expect("serialize reviewed action")
    }

    fn review_completed_with_action(
        reviewed_action: events::ReviewedAction<'_>,
    ) -> events::ReviewCompleted<'_> {
        events::ReviewCompleted {
            thread_id: "thread-1",
            turn_id: "turn-1",
            review_id: "review-1",
            target_item_id: "item-1",
            retry_reason: None,
            request_source: events::ReviewRequestSource::MainTurn,
            reviewed_action,
            reviewed_action_truncated: false,
            response: events::ReviewResponse::Guardian(events::GuardianReviewResponse {
                decision: events::ReviewDecision::Approved,
                terminal_status: events::ReviewTerminalStatus::Approved,
                risk_level: None,
                user_authorization: None,
                outcome: None,
                rationale: None,
                session: None,
                review_timeout_ms: 30_000,
                tool_call_count: 0,
                time_to_first_token_ms: None,
                completion_latency_ms: None,
                token_usage: None,
            }),
            started_at: 1,
            ended_at: 2,
        }
    }

    #[test]
    fn legacy_guardian_projection_ignores_user_review_responses() {
        let observation = events::ReviewCompleted {
            thread_id: "thread-1",
            turn_id: "turn-1",
            review_id: "review-1",
            target_item_id: "item-1",
            retry_reason: None,
            request_source: events::ReviewRequestSource::MainTurn,
            reviewed_action: events::ReviewedAction::ApplyPatch {
                cwd: "/repo",
                files: &[],
            },
            reviewed_action_truncated: false,
            response: events::ReviewResponse::User(events::UserReviewResponse {
                decision: events::ReviewDecision::Approved,
            }),
            started_at: 1,
            ended_at: 2,
        };

        assert!(legacy_guardian_review_event(observation).is_none());
    }

    #[test]
    fn projects_shell_reviewed_action_with_permission_profile() {
        let command = vec!["git".to_string(), "status".to_string()];
        let read_paths = vec!["/repo".to_string()];
        let write_paths = vec!["/repo/tmp".to_string()];

        let action = events::ReviewedAction::Shell {
            command: &command,
            command_display: "git status",
            cwd: "/repo",
            sandbox_permissions: events::ReviewSandboxPermissions::WithAdditionalPermissions,
            additional_permissions: Some(events::ReviewPermissionProfile {
                network: Some(events::ReviewNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(events::ReviewFileSystemPermissions {
                    read: Some(&read_paths),
                    write: Some(&write_paths),
                }),
            }),
            justification: Some("inspect repository state"),
        };

        assert_eq!(
            projected_reviewed_action(action),
            json!({
                "type": "shell",
                "command": ["git", "status"],
                "command_display": "git status",
                "cwd": "/repo",
                "sandbox_permissions": "with_additional_permissions",
                "additional_permissions": {
                    "network": {
                        "enabled": true
                    },
                    "file_system": {
                        "read": ["/repo"],
                        "write": ["/repo/tmp"]
                    }
                },
                "justification": "inspect repository state"
            })
        );
    }

    #[test]
    fn projects_remaining_reviewed_action_variants() {
        let unified_command = vec!["cargo".to_string(), "test".to_string()];
        assert_eq!(
            projected_reviewed_action(events::ReviewedAction::UnifiedExec {
                command: &unified_command,
                command_display: "cargo test",
                cwd: "/repo",
                sandbox_permissions: events::ReviewSandboxPermissions::RequireEscalated,
                additional_permissions: None,
                justification: None,
                tty: true,
            }),
            json!({
                "type": "unified_exec",
                "command": ["cargo", "test"],
                "command_display": "cargo test",
                "cwd": "/repo",
                "sandbox_permissions": "require_escalated",
                "additional_permissions": null,
                "justification": null,
                "tty": true
            })
        );

        let argv = vec!["git".to_string(), "diff".to_string()];
        assert_eq!(
            projected_reviewed_action(events::ReviewedAction::ProcessExec {
                source: events::ReviewCommandSource::UnifiedExec,
                program: "git",
                argv: &argv,
                cwd: "/repo",
                additional_permissions: None,
            }),
            json!({
                "type": "execve",
                "source": "unified_exec",
                "program": "git",
                "argv": ["git", "diff"],
                "cwd": "/repo",
                "additional_permissions": null
            })
        );

        let files = vec!["src/lib.rs".to_string(), "Cargo.toml".to_string()];
        assert_eq!(
            projected_reviewed_action(events::ReviewedAction::ApplyPatch {
                cwd: "/repo",
                files: &files,
            }),
            json!({
                "type": "apply_patch",
                "cwd": "/repo",
                "files": ["src/lib.rs", "Cargo.toml"]
            })
        );

        assert_eq!(
            projected_reviewed_action(events::ReviewedAction::NetworkAccess {
                target: "https://example.com",
                host: "example.com",
                protocol: events::ReviewNetworkApprovalProtocol::Https,
                port: 443,
            }),
            json!({
                "type": "network_access",
                "target": "https://example.com",
                "host": "example.com",
                "protocol": "https",
                "port": 443
            })
        );

        assert_eq!(
            projected_reviewed_action(events::ReviewedAction::McpToolCall {
                server: "drive",
                tool_name: "search",
                connector_id: Some("drive-connector"),
                connector_name: Some("Drive"),
                tool_title: Some("Search Drive"),
            }),
            json!({
                "type": "mcp_tool_call",
                "server": "drive",
                "tool_name": "search",
                "connector_id": "drive-connector",
                "connector_name": "Drive",
                "tool_title": "Search Drive"
            })
        );
    }
}
