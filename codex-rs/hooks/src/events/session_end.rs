use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::SessionEndCommandInput;

pub(crate) const SESSION_END_DEFAULT_TIMEOUT_SEC: u64 = 1;
/// Keep below app-server's in-process `SHUTDOWN_TIMEOUT`: SessionEnd runs during
/// teardown and must leave headroom within the existing five-second bound.
pub(crate) const SESSION_END_MAX_TIMEOUT_SEC: u64 = 3;
const SESSION_END_REASON: &str = "other";

#[derive(Debug, Clone)]
pub struct SessionEndRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct SessionEndOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(handlers: &[ConfiguredHandler]) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::SessionEnd,
        Some(SESSION_END_REASON),
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SessionEndRequest,
) -> SessionEndOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::SessionEnd,
        Some(SESSION_END_REASON),
    );
    if matched.is_empty() {
        return SessionEndOutcome::default();
    }

    let input_json = match serde_json::to_string(&SessionEndCommandInput {
        session_id: request.session_id.to_string(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "SessionEnd".to_string(),
        reason: SESSION_END_REASON.to_string(),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return SessionEndOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id.clone()),
                    format!("failed to serialize session end hook input: {error}"),
                ),
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
    SessionEndOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<()> {
    let (status, entries) = match (run_result.error.as_deref(), run_result.exit_code) {
        (Some(error), _) => (
            HookRunStatus::Failed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            }],
        ),
        (None, Some(0)) => (HookRunStatus::Completed, Vec::new()),
        (None, Some(code)) => (
            HookRunStatus::Failed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: common::trimmed_non_empty(&run_result.stderr)
                    .unwrap_or_else(|| format!("hook exited with code {code}")),
            }],
        ),
        (None, None) => (
            HookRunStatus::Failed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook process terminated without an exit code".to_string(),
            }],
        ),
    };

    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &run_result, status, entries),
        },
        data: (),
        completion_order: 0,
    }
}

#[cfg(test)]
#[path = "session_end_tests.rs"]
mod tests;
