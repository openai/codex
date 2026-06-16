use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::RateLimitReachedType;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::NullableString;
use crate::schema::StopFailureCommandInput;

#[derive(Debug, Clone)]
pub struct StopFailureRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub error: StopFailureError,
    pub error_details: Option<String>,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StopFailureOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub recovery: Option<StopFailureRecovery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopFailureOutput {
    pub recovery: Option<StopFailureRecovery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopFailureRecovery {
    pub model: StopFailureModelSelector,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopFailureModelSelector {
    Current,
    CatalogDefault,
    Id(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopFailureError {
    RateLimit,
    Overloaded,
    AuthenticationFailed,
    BillingError,
    InvalidRequest,
    ServerError,
    Unknown,
}

impl StopFailureError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RateLimit => "rate_limit",
            Self::Overloaded => "overloaded",
            Self::AuthenticationFailed => "authentication_failed",
            Self::BillingError => "billing_error",
            Self::InvalidRequest => "invalid_request",
            Self::ServerError => "server_error",
            Self::Unknown => "unknown",
        }
    }

    fn from_http_status(status: u16) -> Self {
        match status {
            401 => Self::AuthenticationFailed,
            402 => Self::BillingError,
            429 => Self::RateLimit,
            400..=499 => Self::InvalidRequest,
            500..=599 => Self::ServerError,
            _ => Self::Unknown,
        }
    }

    pub fn classify(error: &CodexErr) -> Option<Self> {
        let error = match error {
            CodexErr::UsageLimitReached(error) => match error.rate_limit_reached_type {
                Some(
                    RateLimitReachedType::WorkspaceOwnerCreditsDepleted
                    | RateLimitReachedType::WorkspaceMemberCreditsDepleted
                    | RateLimitReachedType::WorkspaceOwnerUsageLimitReached
                    | RateLimitReachedType::WorkspaceMemberUsageLimitReached,
                ) => Self::BillingError,
                Some(RateLimitReachedType::RateLimitReached) | None => Self::RateLimit,
            },
            CodexErr::ServerOverloaded => Self::Overloaded,
            CodexErr::RefreshTokenFailed(_) => Self::AuthenticationFailed,
            CodexErr::QuotaExceeded | CodexErr::UsageNotIncluded => Self::BillingError,
            CodexErr::ContextWindowExceeded
            | CodexErr::InvalidRequest(_)
            | CodexErr::InvalidImageRequest() => Self::InvalidRequest,
            CodexErr::InternalServerError | CodexErr::Stream(_, _) | CodexErr::RequestTimeout => {
                Self::ServerError
            }
            CodexErr::UnexpectedStatus(error) => Self::from_http_status(error.status.as_u16()),
            CodexErr::RetryLimit(error) => Self::from_http_status(error.status.as_u16()),
            CodexErr::ConnectionFailed(error) => {
                error.source.status().map_or(Self::ServerError, |status| {
                    Self::from_http_status(status.as_u16())
                })
            }
            CodexErr::ResponseStreamFailed(error) => {
                error.source.status().map_or(Self::ServerError, |status| {
                    Self::from_http_status(status.as_u16())
                })
            }
            CodexErr::CyberPolicy { .. }
            | CodexErr::TurnAborted
            | CodexErr::ThreadNotFound(_)
            | CodexErr::AgentLimitReached { .. }
            | CodexErr::SessionConfiguredNotFirstEvent
            | CodexErr::Timeout
            | CodexErr::Spawn
            | CodexErr::Interrupted
            | CodexErr::InternalAgentDied
            | CodexErr::Sandbox(_)
            | CodexErr::LandlockSandboxExecutableNotProvided
            | CodexErr::UnsupportedOperation(_)
            | CodexErr::Fatal(_)
            | CodexErr::Io(_)
            | CodexErr::Json(_)
            | CodexErr::TokioJoin(_)
            | CodexErr::EnvVar(_) => return None,
            #[cfg(target_os = "linux")]
            CodexErr::LandlockRuleset(_) | CodexErr::LandlockPathFd(_) => return None,
        };
        Some(error)
    }
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &StopFailureRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::StopFailure,
        Some(request.error.as_str()),
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: StopFailureRequest,
) -> StopFailureOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::StopFailure,
        Some(request.error.as_str()),
    );
    if matched.is_empty() {
        return StopFailureOutcome::default();
    }

    let input = StopFailureCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "StopFailure".to_string(),
        model: request.model,
        permission_mode: request.permission_mode,
        error: request.error.as_str().to_string(),
        error_details: NullableString::from_string(request.error_details),
        last_assistant_message: NullableString::from_string(request.last_assistant_message),
    };
    let input_json = match serde_json::to_string(&input) {
        Ok(input_json) => input_json,
        Err(error) => {
            return StopFailureOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize stop failure hook input: {error}"),
                ),
                recovery: None,
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_completed,
    )
    .await;
    let recovery = results
        .iter()
        .find_map(|result| result.data.recovery.clone());
    StopFailureOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        recovery,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<StopFailureOutput> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut recovery = None;

    match run_result.error.as_deref() {
        Some(error) => {
            status = HookRunStatus::Failed;
            entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            });
        }
        None => match run_result.exit_code {
            Some(0) => {
                let stdout = run_result.stdout.trim();
                if !stdout.is_empty() {
                    if let Some(parsed) = output_parser::parse_stop_failure(stdout) {
                        recovery = parsed.recovery;
                        if let Some(reason) = recovery
                            .as_ref()
                            .and_then(|recovery| recovery.reason.clone())
                        {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Feedback,
                                text: reason,
                            });
                        }
                    } else {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: "hook returned invalid StopFailure hook JSON output".to_string(),
                        });
                    }
                }
            }
            Some(code) => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: common::trimmed_non_empty(&run_result.stderr)
                        .unwrap_or_else(|| format!("hook exited with code {code}")),
                });
            }
            None => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: "hook process terminated without an exit code".to_string(),
                });
            }
        },
    }

    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &run_result, status, entries),
        },
        data: StopFailureOutput { recovery },
        completion_order: 0,
    }
}

#[cfg(test)]
#[path = "stop_failure_tests.rs"]
mod tests;
