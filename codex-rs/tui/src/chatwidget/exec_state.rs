//! Unified exec bookkeeping state and helpers for `ChatWidget`.

use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;

use crate::exec_command::split_command_string;

pub(super) struct RunningCommand {
    pub(super) command: Vec<String>,
    pub(super) parsed_cmd: Vec<ParsedCommand>,
    pub(super) source: ExecCommandSource,
}

pub(super) struct UnifiedExecProcessSummary {
    pub(super) key: String,
    pub(super) call_id: String,
    pub(super) command_display: String,
    pub(super) recent_chunks: Vec<String>,
}

pub(super) struct UnifiedExecWaitState {
    command_display: String,
}

impl UnifiedExecWaitState {
    pub(super) fn new(command_display: String) -> Self {
        Self { command_display }
    }

    pub(super) fn is_duplicate(&self, command_display: &str) -> bool {
        self.command_display == command_display
    }
}

#[derive(Clone, Debug)]
pub(super) struct UnifiedExecWaitStreak {
    pub(super) process_id: String,
    pub(super) command_display: Option<String>,
}

impl UnifiedExecWaitStreak {
    pub(super) fn new(process_id: String, command_display: Option<String>) -> Self {
        Self {
            process_id,
            command_display: command_display.filter(|display| !display.is_empty()),
        }
    }

    pub(super) fn update_command_display(&mut self, command_display: Option<String>) {
        if self.command_display.is_some() {
            return;
        }
        self.command_display = command_display.filter(|display| !display.is_empty());
    }
}

pub(super) fn is_unified_exec_source(source: ExecCommandSource) -> bool {
    matches!(
        source,
        ExecCommandSource::UnifiedExecStartup | ExecCommandSource::UnifiedExecInteraction
    )
}

pub(super) fn is_standard_tool_call(parsed_cmd: &[ParsedCommand]) -> bool {
    !parsed_cmd.is_empty()
        && parsed_cmd
            .iter()
            .all(|parsed| !matches!(parsed, ParsedCommand::Unknown { .. }))
}

pub(super) fn command_execution_command_and_parsed(
    command: &str,
    command_actions: &[codex_app_server_protocol::CommandAction],
) -> (Vec<String>, Vec<ParsedCommand>) {
    (
        split_command_string(command),
        command_actions
            .iter()
            .cloned()
            .map(codex_app_server_protocol::CommandAction::into_core)
            .collect(),
    )
}
