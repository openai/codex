use codex_ansi_escape::ansi_escape_line;
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

    /// Background event
    BackgroundEvent { lines: Vec<Line<'static>> },

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

impl HistoryCell {
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
        lines.extend(message.lines().map(|l| Line::from(l.to_string())));
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
            format!(" (code: {}, duration: {:?})", exit_code, duration).dim(),
        ]);
        lines.push(title_line);

        const MAX_LINES: usize = 5;

        let src = if exit_code == 0 { stdout } else { stderr };

        lines.push(Line::from(format!("$ {command}")));
        let mut lines_iter = src.lines();
        for raw in lines_iter.by_ref().take(MAX_LINES) {
            lines.push(ansi_escape_line(raw).dim());
        }
        let remaining = lines_iter.count();
        if remaining > 0 {
            lines.push(Line::from(format!("... {} additional lines", remaining)).dim());
        }
        lines.push(Line::from(""));

        HistoryCell::CompletedExecCommand { lines }
    }

    pub(crate) fn new_background_event(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("event".dim()));
        lines.extend(message.lines().map(|l| Line::from(l.to_string()).dim()));
        lines.push(Line::from(""));
        HistoryCell::BackgroundEvent { lines }
    }

    pub(crate) fn new_session_info(
        config: &Config,
        model: String,
        cwd: std::path::PathBuf,
    ) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from("codex session:".magenta().bold()));
        lines.push(Line::from(vec!["↳ model: ".bold(), model.into()]));
        lines.push(Line::from(vec![
            "↳ cwd: ".bold(),
            cwd.display().to_string().into(),
        ]));
        lines.push(Line::from(vec![
            "↳ approval: ".bold(),
            format!("{:?}", config.approval_policy).into(),
        ]));
        lines.push(Line::from(vec![
            "↳ sandbox: ".bold(),
            format!("{:?}", config.sandbox_policy).into(),
        ]));
        lines.push(Line::from(""));

        HistoryCell::SessionInfo { lines }
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
            HistoryCell::UserPrompt { lines, .. }
            | HistoryCell::AgentMessage { lines, .. }
            | HistoryCell::BackgroundEvent { lines, .. }
            | HistoryCell::SessionInfo { lines, .. }
            | HistoryCell::ActiveExecCommand { lines, .. }
            | HistoryCell::CompletedExecCommand { lines, .. }
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
