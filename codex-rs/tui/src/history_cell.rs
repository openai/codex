use crate::exec_command::strip_bash_lc_and_escape;
use crate::markdown::append_markdown;
use crate::text_block::TextBlock;
use crate::text_formatting::format_and_truncate_tool_result;
use base64::Engine;
use codex_ansi_escape::ansi_escape_line;
use codex_common::elapsed::format_duration;
use codex_common::summarize_sandbox_policy;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::model_supports_reasoning_summaries;
use codex_core::plan_tool::PlanItemArg;
use codex_core::plan_tool::StepStatus;
use codex_core::plan_tool::UpdatePlanArgs;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::SessionConfiguredEvent;
use image::DynamicImage;
use image::ImageReader;
use mcp_types::EmbeddedResourceResource;
use mcp_types::ResourceLink;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use tracing::error;

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

fn span_to_static(span: &Span) -> Span<'static> {
    Span {
        style: span.style,
        content: std::borrow::Cow::Owned(span.content.clone().into_owned()),
    }
}

fn line_to_static(line: &Line) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line.spans.iter().map(span_to_static).collect(),
    }
}

/// Represents an event to display in the conversation history. Returns its
/// `Vec<Line<'static>>` representation to make it easier to display in a
/// scrollable list.
pub(crate) enum HistoryCell {
    /// Welcome message.
    WelcomeMessage { view: TextBlock },

    /// Message from the user.
    UserPrompt { view: TextBlock },

    /// Message from the agent.
    AgentMessage { view: TextBlock },

    /// Reasoning event from the agent.
    AgentReasoning { view: TextBlock },

    /// An exec tool call that has not finished yet.
    ActiveExecCommand { view: TextBlock },

    /// Completed exec tool call.
    CompletedExecCommand { view: TextBlock },

    /// An MCP tool call that has not finished yet.
    ActiveMcpToolCall { view: TextBlock },

    /// Completed MCP tool call where we show the result serialized as JSON.
    CompletedMcpToolCall { view: TextBlock },

    /// Completed MCP tool call where the result is an image.
    /// Admittedly, [mcp_types::CallToolResult] can have multiple content types,
    /// which could be a mix of text and images, so we need to tighten this up.
    // NOTE: For image output we keep the *original* image around and lazily
    // compute a resized copy that fits the available cell width.  Caching the
    // resized version avoids doing the potentially expensive rescale twice
    // because the scroll-view first calls `height()` for layouting and then
    // `render_window()` for painting.
    CompletedMcpToolCallWithImageOutput { _image: DynamicImage },

    /// Background event.
    BackgroundEvent { view: TextBlock },

    /// Output from the `/diff` command.
    GitDiffOutput { view: TextBlock },

    /// Error event from the backend.
    ErrorEvent { view: TextBlock },

    /// Info describing the newly-initialized session.
    SessionInfo { view: TextBlock },

    /// A pending code patch that is awaiting user approval. Mirrors the
    /// behaviour of `ActiveExecCommand` so the user sees *what* patch the
    /// model wants to apply before being prompted to approve or deny it.
    PendingPatch { view: TextBlock },

    /// A patch failed to apply.
    PatchFailed { view: TextBlock },

    /// A human‑friendly rendering of the model's current plan and step
    /// statuses provided via the `update_plan` tool.
    PlanUpdate { view: TextBlock },
}

const TOOL_CALL_MAX_LINES: usize = 5;

impl HistoryCell {
    /// Return a cloned, plain representation of the cell's lines suitable for
    /// one‑shot insertion into the terminal scrollback. Image cells are
    /// represented with a simple placeholder for now.
    pub(crate) fn plain_lines(&self) -> Vec<Line<'static>> {
        match self {
            HistoryCell::WelcomeMessage { view }
            | HistoryCell::UserPrompt { view }
            | HistoryCell::AgentMessage { view }
            | HistoryCell::AgentReasoning { view }
            | HistoryCell::BackgroundEvent { view }
            | HistoryCell::GitDiffOutput { view }
            | HistoryCell::ErrorEvent { view }
            | HistoryCell::SessionInfo { view }
            | HistoryCell::CompletedExecCommand { view }
            | HistoryCell::CompletedMcpToolCall { view }
            | HistoryCell::PendingPatch { view }
            | HistoryCell::PatchFailed { view }
            | HistoryCell::PlanUpdate { view }
            | HistoryCell::ActiveExecCommand { view, .. }
            | HistoryCell::ActiveMcpToolCall { view, .. } => {
                view.lines.iter().map(line_to_static).collect()
            }
            HistoryCell::CompletedMcpToolCallWithImageOutput { .. } => vec![
                Line::from("tool result (image output omitted)"),
                Line::from(""),
            ],
        }
    }

    pub(crate) fn new_session_info(
        config: &Config,
        event: SessionConfiguredEvent,
        is_first_event: bool,
    ) -> Self {
        let SessionConfiguredEvent {
            model,
            session_id,
            history_log_id: _,
            history_entry_count: _,
        } = event;
        if is_first_event {
            const VERSION: &str = env!("CARGO_PKG_VERSION");

            let mut lines: Vec<Line<'static>> = vec![
                Line::from(vec![
                    "OpenAI ".into(),
                    "Codex".bold(),
                    format!(" v{VERSION}").into(),
                    " (research preview)".dim(),
                ]),
                Line::from(""),
                Line::from(vec![
                    "codex session".magenta().bold(),
                    " ".into(),
                    session_id.to_string().dim(),
                ]),
            ];

            let mut entries = vec![
                ("workdir", config.cwd.display().to_string()),
                ("model", config.model.clone()),
                ("provider", config.model_provider_id.clone()),
                ("approval", config.approval_policy.to_string()),
                ("sandbox", summarize_sandbox_policy(&config.sandbox_policy)),
            ];
            if config.model_provider.wire_api == WireApi::Responses
                && model_supports_reasoning_summaries(config)
            {
                entries.push((
                    "reasoning effort",
                    config.model_reasoning_effort.to_string(),
                ));
                entries.push((
                    "reasoning summaries",
                    config.model_reasoning_summary.to_string(),
                ));
            }
            for (key, value) in entries {
                lines.push(Line::from(vec![format!("{key}: ").bold(), value.into()]));
            }
            lines.push(Line::from(""));
            HistoryCell::WelcomeMessage {
                view: TextBlock::new(lines),
            }
        } else if config.model == model {
            HistoryCell::SessionInfo {
                view: TextBlock::new(Vec::new()),
            }
        } else {
            let lines = vec![
                Line::from("model changed:".magenta().bold()),
                Line::from(format!("requested: {}", config.model)),
                Line::from(format!("used: {model}")),
                Line::from(""),
            ];
            HistoryCell::SessionInfo {
                view: TextBlock::new(lines),
            }
        }
    }

    pub(crate) fn new_user_prompt(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("user".cyan().bold()));
        lines.extend(message.lines().map(|l| Line::from(l.to_string())));
        lines.push(Line::from(""));

        HistoryCell::UserPrompt {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_agent_message(config: &Config, message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("codex".magenta().bold()));
        append_markdown(&message, &mut lines, config);
        lines.push(Line::from(""));

        HistoryCell::AgentMessage {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_agent_reasoning(config: &Config, text: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("thinking".magenta().italic()));
        append_markdown(&text, &mut lines, config);
        lines.push(Line::from(""));

        HistoryCell::AgentReasoning {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_active_exec_command(command: Vec<String>) -> Self {
        let command_escaped = strip_bash_lc_and_escape(&command);

        let lines: Vec<Line<'static>> = vec![
            Line::from(vec!["command".magenta(), " running...".dim()]),
            Line::from(format!("$ {command_escaped}")),
            Line::from(""),
        ];

        HistoryCell::ActiveExecCommand {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_completed_exec_command(command: Vec<String>, output: CommandOutput) -> Self {
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

        let cmdline = strip_bash_lc_and_escape(&command);
        lines.push(Line::from(format!("$ {cmdline}")));
        let mut lines_iter = src.lines();
        for raw in lines_iter.by_ref().take(TOOL_CALL_MAX_LINES) {
            lines.push(ansi_escape_line(raw).dim());
        }
        let remaining = lines_iter.count();
        if remaining > 0 {
            lines.push(Line::from(format!("... {remaining} additional lines")).dim());
        }
        lines.push(Line::from(""));

        HistoryCell::CompletedExecCommand {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_active_mcp_tool_call(invocation: McpInvocation) -> Self {
        let title_line = Line::from(vec!["tool".magenta(), " running...".dim()]);
        let lines: Vec<Line> = vec![
            title_line,
            format_mcp_invocation(invocation.clone()),
            Line::from(""),
        ];

        HistoryCell::ActiveMcpToolCall {
            view: TextBlock::new(lines),
        }
    }

    /// If the first content is an image, return a new cell with the image.
    /// TODO(rgwood-dd): Handle images properly even if they're not the first result.
    fn try_new_completed_mcp_tool_call_with_image_output(
        result: &Result<mcp_types::CallToolResult, String>,
    ) -> Option<Self> {
        match result {
            Ok(mcp_types::CallToolResult { content, .. }) => {
                if let Some(mcp_types::ContentBlock::ImageContent(image)) = content.first() {
                    let raw_data =
                        match base64::engine::general_purpose::STANDARD.decode(&image.data) {
                            Ok(data) => data,
                            Err(e) => {
                                error!("Failed to decode image data: {e}");
                                return None;
                            }
                        };
                    let reader = match ImageReader::new(Cursor::new(raw_data)).with_guessed_format()
                    {
                        Ok(reader) => reader,
                        Err(e) => {
                            error!("Failed to guess image format: {e}");
                            return None;
                        }
                    };

                    let image = match reader.decode() {
                        Ok(image) => image,
                        Err(e) => {
                            error!("Image decoding failed: {e}");
                            return None;
                        }
                    };

                    Some(HistoryCell::CompletedMcpToolCallWithImageOutput { _image: image })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(crate) fn new_completed_mcp_tool_call(
        num_cols: u16,
        invocation: McpInvocation,
        duration: Duration,
        success: bool,
        result: Result<mcp_types::CallToolResult, String>,
    ) -> Self {
        if let Some(cell) = Self::try_new_completed_mcp_tool_call_with_image_output(&result) {
            return cell;
        }

        let duration = format_duration(duration);
        let status_str = if success { "success" } else { "failed" };
        let title_line = Line::from(vec![
            "tool".magenta(),
            " ".into(),
            if success {
                status_str.green()
            } else {
                status_str.red()
            },
            format!(", duration: {duration}").gray(),
        ]);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(title_line);
        lines.push(format_mcp_invocation(invocation));

        match result {
            Ok(mcp_types::CallToolResult { content, .. }) => {
                if !content.is_empty() {
                    lines.push(Line::from(""));

                    for tool_call_result in content {
                        let line_text = match tool_call_result {
                            mcp_types::ContentBlock::TextContent(text) => {
                                format_and_truncate_tool_result(
                                    &text.text,
                                    TOOL_CALL_MAX_LINES,
                                    num_cols as usize,
                                )
                            }
                            mcp_types::ContentBlock::ImageContent(_) => {
                                // TODO show images even if they're not the first result, will require a refactor of `CompletedMcpToolCall`
                                "<image content>".to_string()
                            }
                            mcp_types::ContentBlock::AudioContent(_) => {
                                "<audio content>".to_string()
                            }
                            mcp_types::ContentBlock::EmbeddedResource(resource) => {
                                let uri = match resource.resource {
                                    EmbeddedResourceResource::TextResourceContents(text) => {
                                        text.uri
                                    }
                                    EmbeddedResourceResource::BlobResourceContents(blob) => {
                                        blob.uri
                                    }
                                };
                                format!("embedded resource: {uri}")
                            }
                            mcp_types::ContentBlock::ResourceLink(ResourceLink { uri, .. }) => {
                                format!("link: {uri}")
                            }
                        };
                        lines.push(Line::styled(line_text, Style::default().fg(Color::Gray)));
                    }
                }

                lines.push(Line::from(""));
            }
            Err(e) => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "Error: ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(e),
                ]));
            }
        };

        HistoryCell::CompletedMcpToolCall {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_background_event(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("event".dim()));
        lines.extend(message.lines().map(|line| ansi_escape_line(line).dim()));
        lines.push(Line::from(""));
        HistoryCell::BackgroundEvent {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_diff_output(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("/diff".magenta()));

        if message.trim().is_empty() {
            lines.push(Line::from("No changes detected.".italic()));
        } else {
            lines.extend(message.lines().map(ansi_escape_line));
        }

        lines.push(Line::from(""));
        HistoryCell::GitDiffOutput {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_error_event(message: String) -> Self {
        let lines: Vec<Line<'static>> = vec![
            vec!["ERROR: ".red().bold(), message.into()].into(),
            "".into(),
        ];
        HistoryCell::ErrorEvent {
            view: TextBlock::new(lines),
        }
    }

    /// Render a user‑friendly plan update with colourful status icons and a
    /// simple progress indicator so users can follow along.
    pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> Self {
        let UpdatePlanArgs { explanation, plan } = update;

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Title
        lines.push(Line::from("plan".magenta().bold()));

        if !plan.is_empty() {
            // Progress bar – show completed/total with a visual bar
            let total = plan.len();
            let completed = plan
                .iter()
                .filter(|p| matches!(p.status, StepStatus::Completed))
                .count();
            let width: usize = 20;
            let filled = (completed * width + total / 2) / total;
            let empty = width.saturating_sub(filled);
            let mut bar_spans: Vec<Span> = Vec::new();
            if filled > 0 {
                bar_spans.push(Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(Color::Green),
                ));
            }
            if empty > 0 {
                bar_spans.push(Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::Gray),
                ));
            }
            let progress_prefix = Span::raw("progress [");
            let progress_suffix = Span::raw("] ");
            let fraction = Span::raw(format!("{completed}/{total}"));
            let mut progress_line_spans = vec![progress_prefix];
            progress_line_spans.extend(bar_spans);
            progress_line_spans.push(progress_suffix);
            progress_line_spans.push(fraction);
            lines.push(Line::from(progress_line_spans));
        }

        // Optional explanation/note from the model
        if let Some(expl) = explanation.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        }) {
            lines.push(Line::from("note".gray().italic()));
            for l in expl.lines() {
                lines.push(Line::from(l.to_string()).gray());
            }
        }

        // Steps (1‑based numbering) with fun, readable status icons
        if plan.is_empty() {
            lines.push(Line::from("(no steps provided)".gray().italic()));
        } else {
            for (idx, PlanItemArg { step, status }) in plan.into_iter().enumerate() {
                let num = idx + 1;
                let icon_span: Span = match status {
                    StepStatus::Completed => Span::from("✓").fg(Color::Green),
                    StepStatus::InProgress => Span::from("▶").fg(Color::Yellow).bold(),
                    StepStatus::Pending => Span::from("○").fg(Color::Gray),
                };
                lines.push(Line::from(vec![
                    format!("{num:>2}. [").into(),
                    icon_span,
                    "] ".into(),
                    step.into(),
                ]));
            }
        }

        lines.push(Line::from(""));

        HistoryCell::PlanUpdate {
            view: TextBlock::new(lines),
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
                return Self::PendingPatch {
                    view: TextBlock::new(lines),
                };
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

        HistoryCell::PendingPatch {
            view: TextBlock::new(lines),
        }
    }

    pub fn new_patch_failed_event(stdout: String, stderr: String) -> Self {
        let lines: Vec<Line<'static>> = vec![
            Line::from("patch failed".red().bold()),
            Line::from(stdout),
            Line::from(stderr),
            Line::from(""),
        ];
        HistoryCell::PatchFailed {
            view: TextBlock::new(lines),
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

fn format_mcp_invocation<'a>(invocation: McpInvocation) -> Line<'a> {
    let args_str = invocation
        .arguments
        .as_ref()
        .map(|v| {
            // Use compact form to keep things short but readable.
            serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
        })
        .unwrap_or_default();

    let invocation_spans = vec![
        Span::styled(invocation.server.clone(), Style::default().fg(Color::Blue)),
        Span::raw("."),
        Span::styled(invocation.tool.clone(), Style::default().fg(Color::Blue)),
        Span::raw("("),
        Span::styled(args_str, Style::default().fg(Color::Gray)),
        Span::raw(")"),
    ];
    Line::from(invocation_spans)
}
