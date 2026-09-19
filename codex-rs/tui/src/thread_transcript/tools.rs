//! Share tool history conversion between live handlers and persisted transcript pages.
//!
//! These values contain presentation data only. Creating historical cells never starts a task,
//! changes composer state, or leaves an animation running; pending items retain their last known
//! status. Callers own chronological grouping.

use std::time::Duration;

use crate::exec_cell::CommandOutput;
use crate::exec_cell::ExecCell;
use crate::exec_cell::new_active_exec_command;
use crate::history_cell::McpInvocation;
use crate::history_cell::McpToolCallCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::new_active_mcp_tool_call;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::parse_command::ParsedCommand;
use ratatui::style::Stylize as _;
use ratatui::text::Line;

/// Preserve status and output when replay cannot reconstruct a rich completed tool cell.
/// Declined commands never ran, and historical terminal interactions do not carry stdin details.
/// Pending items retain their last known state without starting a running clock.
pub(crate) fn historical_tool_fallback(item: &ThreadItem) -> Option<PlainHistoryCell> {
    let lines = match item {
        ThreadItem::CommandExecution {
            command,
            source,
            status,
            aggregated_output,
            exit_code,
            ..
        } if matches!(
            status,
            CommandExecutionStatus::InProgress | CommandExecutionStatus::Declined
        ) || *source == CommandExecutionSource::UnifiedExecInteraction
            || replay_command_args(command).is_none() =>
        {
            let status_label = match status {
                CommandExecutionStatus::InProgress => "In progress at last update",
                CommandExecutionStatus::Completed => "Completed",
                CommandExecutionStatus::Failed => "Failed",
                CommandExecutionStatus::Declined => "Declined",
            };
            let mut status_line = if *source == CommandExecutionSource::UnifiedExecInteraction {
                format!("Terminal interaction · {status_label}")
            } else {
                status_label.to_string()
            };
            if matches!(
                status,
                CommandExecutionStatus::Completed | CommandExecutionStatus::Failed
            ) && let Some(code) = exit_code
            {
                status_line.push_str(&format!(" · exit {code}"));
            }
            let mut lines: Vec<Line<'static>> = vec![
                vec!["$ ".dim(), command.clone().into()].into(),
                status_line.dim().into(),
            ];
            if let Some(output) = aggregated_output {
                lines.extend(
                    output
                        .lines()
                        .map(|line| vec!["  ".dim(), line.trim_end().to_string().dim()].into()),
                );
            }
            lines
        }
        ThreadItem::McpToolCall {
            server,
            tool,
            status,
            result,
            error,
            ..
        } if *status == McpToolCallStatus::InProgress || (result.is_none() && error.is_none()) => {
            let status = match status {
                McpToolCallStatus::InProgress => "In progress at last update",
                McpToolCallStatus::Completed => "Completed · result unavailable",
                McpToolCallStatus::Failed => "Failed · result unavailable",
            };
            vec![format!("mcp tool: {server}/{tool} · {status}").dim().into()]
        }
        ThreadItem::CollabAgentToolCall {
            tool:
                tool @ (CollabAgentTool::SpawnAgent
                | CollabAgentTool::SendInput
                | CollabAgentTool::CloseAgent
                | CollabAgentTool::ResumeAgent
                | CollabAgentTool::Wait),
            status,
            ..
        } if *status != CollabAgentToolCallStatus::Completed => {
            let status = match status {
                CollabAgentToolCallStatus::InProgress => "In progress at last update",
                CollabAgentToolCallStatus::Failed => "Failed",
                CollabAgentToolCallStatus::Interrupted => "Interrupted",
                CollabAgentToolCallStatus::Completed => "Completed",
            };
            vec![format!("agent tool: {tool:?} · {status}").dim().into()]
        }
        _ => return None,
    };
    Some(PlainHistoryCell::new(lines))
}

// The protocol joins argv with shlex. Keep noncanonical legacy commands verbatim.
fn replay_command_args(command: &str) -> Option<Vec<String>> {
    let args = shlex::split(command)?;
    (shlex::try_join(args.iter().map(String::as_str)).as_deref() == Ok(command)).then_some(args)
}

pub(crate) struct CommandHistory {
    pub(crate) id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) source: CommandExecutionSource,
    pub(crate) aggregated_output: String,
    pub(crate) exit_code: i32,
    pub(crate) duration: Duration,
}

impl CommandHistory {
    pub(crate) fn from_item(item: ThreadItem) -> Option<Self> {
        let ThreadItem::CommandExecution {
            id,
            command,
            source,
            status,
            command_actions,
            aggregated_output,
            exit_code,
            duration_ms,
            ..
        } = item
        else {
            return None;
        };
        // Thread snapshots omit stdin, so `None` would falsely classify these as waits.
        if source == CommandExecutionSource::UnifiedExecInteraction {
            return None;
        }
        let exit_code = match status {
            CommandExecutionStatus::InProgress | CommandExecutionStatus::Declined => return None,
            CommandExecutionStatus::Completed => exit_code.unwrap_or_default(),
            CommandExecutionStatus::Failed => {
                exit_code.filter(|code| *code != 0).unwrap_or(/*default*/ 1)
            }
        };
        Some(Self {
            id,
            command: replay_command_args(&command)?,
            parsed: command_actions
                .into_iter()
                .map(codex_app_server_protocol::CommandAction::into_core)
                .collect(),
            source,
            aggregated_output: aggregated_output.unwrap_or_default(),
            exit_code,
            duration: Duration::from_millis(duration_ms.unwrap_or_default().max(/*other*/ 0) as u64),
        })
    }

    pub(crate) fn into_cell(self) -> ExecCell {
        let output = CommandOutput::new(self.exit_code, self.aggregated_output);
        let mut cell = new_active_exec_command(
            self.id.clone(),
            self.command,
            self.parsed,
            self.source,
            /*interaction_input*/ None,
            /*animations_enabled*/ false,
        );
        let completed = cell.complete_call(&self.id, output, self.duration);
        debug_assert!(completed, "new exec cell should contain {}", self.id);
        cell
    }
}

pub(crate) struct McpHistory {
    pub(crate) id: String,
    pub(crate) invocation: McpInvocation,
    pub(crate) duration: Duration,
    pub(crate) result: Result<CallToolResult, String>,
}

impl McpHistory {
    pub(crate) fn from_item(item: ThreadItem) -> Option<Self> {
        let ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            arguments,
            result,
            error,
            duration_ms,
            ..
        } = item
        else {
            return None;
        };
        if status == McpToolCallStatus::InProgress {
            return None;
        }
        let result = match (result, error) {
            (_, Some(error)) => Err(error.message),
            (Some(result), None) => {
                let result = *result;
                Ok(CallToolResult {
                    content: result.content,
                    structured_content: result.structured_content,
                    is_error: Some(status == McpToolCallStatus::Failed),
                    meta: None,
                })
            }
            (None, None) => return None,
        };
        Some(Self {
            id,
            invocation: McpInvocation {
                server,
                tool,
                arguments: Some(arguments),
            },
            duration: Duration::from_millis(duration_ms.unwrap_or_default().max(/*other*/ 0) as u64),
            result,
        })
    }

    pub(crate) fn into_cell(self) -> McpToolCallCell {
        let mut cell =
            new_active_mcp_tool_call(self.id, self.invocation, /*animations_enabled*/ false);
        cell.complete(self.duration, self.result);
        cell
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
