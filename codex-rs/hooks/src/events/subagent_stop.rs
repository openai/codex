use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use super::stop;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::SubagentStopCommandInput;

#[derive(Debug, Clone)]
pub struct SubagentStopRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub agent_id: String,
    pub agent_type: String,
    pub agent_transcript_path: Option<PathBuf>,
    pub last_assistant_message: Option<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &SubagentStopRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::SubagentStop,
        Some(request.agent_type.as_str()),
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SubagentStopRequest,
) -> stop::StopOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::SubagentStop,
        Some(request.agent_type.as_str()),
    );
    if matched.is_empty() {
        return stop::StopOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            should_block: false,
            block_reason: None,
            continuation_fragments: Vec::new(),
        };
    }

    let input_json = match serde_json::to_string(&SubagentStopCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "SubagentStop".to_string(),
        model: request.model,
        permission_mode: request.permission_mode,
        stop_hook_active: request.stop_hook_active,
        agent_id: request.agent_id,
        agent_type: request.agent_type,
        agent_transcript_path: NullableString::from_path(request.agent_transcript_path),
        last_assistant_message: NullableString::from_string(request.last_assistant_message),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return stop::serialization_failure_outcome(common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                format!("failed to serialize subagent stop hook input: {error}"),
            ));
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        stop::parse_subagent_stop_completed,
    )
    .await;

    let aggregate = stop::aggregate_results(results.iter().map(|result| &result.data));

    stop::StopOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        should_stop: aggregate.should_stop,
        stop_reason: aggregate.stop_reason,
        should_block: aggregate.should_block,
        block_reason: aggregate.block_reason,
        continuation_fragments: aggregate.continuation_fragments,
    }
}
