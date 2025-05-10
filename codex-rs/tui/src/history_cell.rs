use codex_ansi_escape::ansi_escape_line;
use codex_common::elapsed::format_duration;
use codex_core::config::Config;
use codex_core::protocol::FileChange;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crate::exec_command::escape_command;
use crate::markdown::append_markdown;

pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
}

pub(crate) enum PatchEventType {
    ApprovalRequest,
    ApplyBegin { auto_approved: bool },
}

/// Represents an event to display in the conversation history. Returns its
/// `Vec<Line<'static>>` representation to make it easier to display in a
/// scrollable list.
pub(crate) enum HistoryCell {
    /// Welcome message.
    WelcomeMessage { lines: Vec<Line<'static>> },

    /// Message from the user.
    UserPrompt { lines: Vec<Line<'static>> },

    /// Message from the agent.
    AgentMessage { lines: Vec<Line<'static>> },

    /// An exec tool call that has not finished yet.
    ActiveExecCommand {
        call_id: String,
        /// The shell command, escaped and formatted.
        command: String,
        start: Instant,
        lines: Vec<Line<'static>>,
    },

    /// Completed exec tool call.
    CompletedExecCommand { lines: Vec<Line<'static>> },

    /// An MCP tool call that has not finished yet.
    ActiveMcpToolCall {
        call_id: String,
        /// `server.tool` fully-qualified name so we can show a concise label
        fq_tool_name: String,
        /// Formatted invocation that mirrors the `$ cmd ...` style of exec
        /// commands. We keep this around so the completed state can reuse the
        /// exact same text without re-formatting.
        invocation: String,
        start: Instant,
        lines: Vec<Line<'static>>,
    },

    /// Completed MCP tool call.
    CompletedMcpToolCall { lines: Vec<Line<'static>> },

    /// Background event
    BackgroundEvent { lines: Vec<Line<'static>> },

    /// Error event from the backend.
    ErrorEvent { lines: Vec<Line<'static>> },

    /// Info describing the newly‑initialized session.
    SessionInfo { lines: Vec<Line<'static>> },

    /// A pending code patch that is awaiting user approval. Mirrors the
    /// behaviour of `ActiveExecCommand` so the user sees *what* patch the
    /// model wants to apply before being prompted to approve or deny it.
    PendingPatch {
        /// Identifier so that a future `PatchApplyEnd` can update the entry
        /// with the final status (not yet implemented).
        lines: Vec<Line<'static>>,
    },
}

const TOOL_CALL_MAX_LINES: usize = 5;

impl HistoryCell {
    pub(crate) fn new_welcome_message(config: &Config) -> Self {
        let mut lines: Vec<Line<'static>> = vec![
            Line::from(vec![
                "OpenAI ".into(),
                "Codex".bold(),
                " (research preview)".dim(),
            ]),
            Line::from(""),
            Line::from("codex session:".magenta().bold()),
        ];

        let entries = vec![
            ("workdir", config.cwd.display().to_string()),
            ("model", config.model.clone()),
            ("provider", config.model_provider_id.clone()),
            ("approval", format!("{:?}", config.approval_policy)),
            ("sandbox", format!("{:?}", config.sandbox_policy)),
        ];
        for (key, value) in entries {
            lines.push(Line::from(vec![format!("{key}: ").bold(), value.into()]));
        }
        lines.push(Line::from(""));
        HistoryCell::WelcomeMessage { lines }
    }

    pub(crate) fn new_user_prompt(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("user".cyan().bold()));
        lines.extend(message.lines().map(|l| Line::from(l.to_string())));
        lines.push(Line::from(""));

        HistoryCell::UserPrompt { lines }
    }

    pub(crate) fn new_agent_message(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("codex".magenta().bold()));
        append_markdown(&message, &mut lines);
        lines.push(Line::from(""));

        HistoryCell::AgentMessage { lines }
    }

    pub(crate) fn new_active_exec_command(call_id: String, command: Vec<String>) -> Self {
        let command_escaped = escape_command(&command);
        let start = Instant::now();

        let lines: Vec<Line<'static>> = vec![
            Line::from(vec!["command".magenta(), " running...".dim()]),
            Line::from(format!("$ {command_escaped}")),
            Line::from(""),
        ];

        HistoryCell::ActiveExecCommand {
            call_id,
            command: command_escaped,
            start,
            lines,
        }
    }

    pub(crate) fn new_completed_exec_command(command: String, output: CommandOutput) -> Self {
        let CommandOutput {
            exit_code,
            stdout,
            stderr,
            duration,
        } = output;

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Title depends on whether we have output yet.
        let title_line = Line::from(vec![
            "command".magenta(),
            format!(
                " (code: {}, duration: {})",
                exit_code,
                format_duration(duration)
            )
            .dim(),
        ]);
        lines.push(title_line);

        let src = if exit_code == 0 { stdout } else { stderr };

        lines.push(Line::from(format!("$ {command}")));
        let mut lines_iter = src.lines();
        for raw in lines_iter.by_ref().take(TOOL_CALL_MAX_LINES) {
            lines.push(ansi_escape_line(raw).dim());
        }
        let remaining = lines_iter.count();
        if remaining > 0 {
            lines.push(Line::from(format!("... {} additional lines", remaining)).dim());
        }
        lines.push(Line::from(""));

        HistoryCell::CompletedExecCommand { lines }
    }

    pub(crate) fn new_active_mcp_tool_call(
        call_id: String,
        server: String,
        tool: String,
        arguments: Option<serde_json::Value>,
    ) -> Self {
        let fq_tool_name = format!("{server}.{tool}");

        // Format the arguments as compact JSON so they roughly fit on one
        // line. If there are no arguments we keep it empty so the invocation
        // mirrors a function-style call.
        let args_str = arguments
            .as_ref()
            .map(|v| {
                // Use compact form to keep things short but readable.
                serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
            })
            .unwrap_or_default();

        let invocation = if args_str.is_empty() {
            format!("{fq_tool_name}()")
        } else {
            format!("{fq_tool_name}({args_str})")
        };

        let start = Instant::now();
        let title_line = Line::from(vec!["tool".magenta(), " running...".dim()]);
        let lines: Vec<Line<'static>> = vec![
            title_line,
            Line::from(format!("$ {invocation}")),
            Line::from(""),
        ];

        HistoryCell::ActiveMcpToolCall {
            call_id,
            fq_tool_name,
            invocation,
            start,
            lines,
        }
    }

    pub(crate) fn new_completed_mcp_tool_call(
        fq_tool_name: String,
        invocation: String,
        start: Instant,
        success: bool,
        result: Option<serde_json::Value>,
    ) -> Self {
        let duration = format_duration(start.elapsed());
        let status_str = if success { "success" } else { "failed" };
        let title_line = Line::from(vec![
            "tool".magenta(),
            format!(" {fq_tool_name} ({status_str}, duration: {})", duration).dim(),
        ]);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(title_line);
        lines.push(Line::from(format!("$ {invocation}")));

        if let Some(res_val) = result {
            let json_pretty =
                serde_json::to_string_pretty(&res_val).unwrap_or_else(|_| res_val.to_string());
            let mut iter = json_pretty.lines();
            for raw in iter.by_ref().take(TOOL_CALL_MAX_LINES) {
                lines.push(Line::from(raw.to_string()).dim());
            }
            let remaining = iter.count();
            if remaining > 0 {
                lines.push(Line::from(format!("... {} additional lines", remaining)).dim());
            }
        }

        lines.push(Line::from(""));

        HistoryCell::CompletedMcpToolCall { lines }
    }

    pub(crate) fn new_background_event(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("event".dim()));
        lines.extend(message.lines().map(|l| Line::from(l.to_string()).dim()));
        lines.push(Line::from(""));
        HistoryCell::BackgroundEvent { lines }
    }

    pub(crate) fn new_error_event(message: String) -> Self {
        let lines: Vec<Line<'static>> = vec![
            vec!["ERROR: ".red().bold(), message.into()].into(),
            "".into(),
        ];
        HistoryCell::ErrorEvent { lines }
    }

    pub(crate) fn new_session_info(config: &Config, model: String) -> Self {
        if config.model == model {
            HistoryCell::SessionInfo { lines: vec![] }
        } else {
            let lines = vec![
                Line::from("model changed:".magenta().bold()),
                Line::from(format!("requested: {}", config.model)),
                Line::from(format!("used: {}", model)),
                Line::from(""),
            ];
            HistoryCell::SessionInfo { lines }
        }
    }

    /// Create a new `PendingPatch` cell that lists the file‑level summary of
    /// a proposed patch. The summary lines should already be formatted (e.g.
    /// "A path/to/file.rs").
    pub(crate) fn new_patch_event(
        event_type: PatchEventType,
        changes: HashMap<PathBuf, FileChange>,
    ) -> Self {
        let title = match event_type {
            PatchEventType::ApprovalRequest => "proposed patch",
            PatchEventType::ApplyBegin {
                auto_approved: true,
            } => "applying patch",
            PatchEventType::ApplyBegin {
                auto_approved: false,
            } => {
                let lines = vec![Line::from("patch applied".magenta().bold())];
                return Self::PendingPatch { lines };
            }
        };

        let summary_lines = create_diff_summary(changes);

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header similar to the command formatter so patches are visually
        // distinct while still fitting the overall colour scheme.
        lines.push(Line::from(title.magenta().bold()));

        for line in summary_lines {
            if line.starts_with('+') {
                lines.push(line.green().into());
            } else if line.starts_with('-') {
                lines.push(line.red().into());
            } else if let Some(space_idx) = line.find(' ') {
                let kind_owned = line[..space_idx].to_string();
                let rest_owned = line[space_idx + 1..].to_string();

                let style_for = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::BOLD);

                let styled_kind = match kind_owned.as_str() {
                    "A" => RtSpan::styled(kind_owned.clone(), style_for(Color::Green)),
                    "D" => RtSpan::styled(kind_owned.clone(), style_for(Color::Red)),
                    "M" => RtSpan::styled(kind_owned.clone(), style_for(Color::Yellow)),
                    "R" | "C" => RtSpan::styled(kind_owned.clone(), style_for(Color::Cyan)),
                    _ => RtSpan::raw(kind_owned.clone()),
                };

                let styled_line =
                    RtLine::from(vec![styled_kind, RtSpan::raw(" "), RtSpan::raw(rest_owned)]);
                lines.push(styled_line);
            } else {
                lines.push(Line::from(line));
            }
        }

        lines.push(Line::from(""));

        HistoryCell::PendingPatch { lines }
    }

    pub(crate) fn lines(&self) -> &Vec<Line<'static>> {
        match self {
            HistoryCell::WelcomeMessage { lines, .. }
            | HistoryCell::UserPrompt { lines, .. }
            | HistoryCell::AgentMessage { lines, .. }
            | HistoryCell::BackgroundEvent { lines, .. }
            | HistoryCell::ErrorEvent { lines, .. }
            | HistoryCell::SessionInfo { lines, .. }
            | HistoryCell::ActiveExecCommand { lines, .. }
            | HistoryCell::CompletedExecCommand { lines, .. }
            | HistoryCell::ActiveMcpToolCall { lines, .. }
            | HistoryCell::CompletedMcpToolCall { lines, .. }
            | HistoryCell::PendingPatch { lines, .. } => lines,
        }
    }
}

fn create_diff_summary(changes: HashMap<PathBuf, FileChange>) -> Vec<String> {
    // Build a concise, human‑readable summary list similar to the
    // `git status` short format so the user can reason about the
    // patch without scrolling.
    let mut summaries: Vec<String> = Vec::new();
    for (path, change) in &changes {
        use codex_core::protocol::FileChange::*;
        match change {
            Add { content } => {
                let added = content.lines().count();
                summaries.push(format!("A {} (+{added})", path.display()));
            }
            Delete => {
                summaries.push(format!("D {}", path.display()));
            }
            Update {
                unified_diff,
                move_path,
            } => {
                if let Some(new_path) = move_path {
                    summaries.push(format!("R {} → {}", path.display(), new_path.display(),));
                } else {
                    summaries.push(format!("M {}", path.display(),));
                }
                summaries.extend(unified_diff.lines().map(|s| s.to_string()));
            }
        }
    }

    summaries
}
