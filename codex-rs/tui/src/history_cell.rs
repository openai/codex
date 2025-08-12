use crate::colors::LIGHT_BLUE;
use crate::diff_render::create_diff_summary;
use crate::exec_command::relativize_to_home;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::slash_command::SlashCommand;
use crate::text_block::TextBlock;
use crate::text_formatting::format_and_truncate_tool_result;
use base64::Engine;
use codex_ansi_escape::ansi_escape_line;
use codex_common::create_config_summary_entries;
use codex_common::elapsed::format_duration;
use codex_core::config::Config;
use codex_core::parse_command::ParsedCommand;
use codex_core::plan_tool::PlanItemArg;
use codex_core::plan_tool::StepStatus;
use codex_core::plan_tool::UpdatePlanArgs;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::TokenUsage;
use codex_login::get_auth_file;
use codex_login::try_read_auth_json;
use image::DynamicImage;
use image::ImageReader;
use mcp_types::EmbeddedResourceResource;
use mcp_types::ResourceLink;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use tracing::error;

#[derive(Clone)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
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

pub(crate) struct ExecCell {
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
}

/// Represents an event to display in the conversation history. Returns its
/// `Vec<Line<'static>>` representation to make it easier to display in a
/// scrollable list.
pub(crate) enum HistoryCell {
    /// Welcome message.
    WelcomeMessage {
        view: TextBlock,
    },

    /// Message from the user.
    UserPrompt {
        view: TextBlock,
    },

    Exec(ExecCell),

    /// An MCP tool call that has not finished yet.
    ActiveMcpToolCall {
        view: TextBlock,
    },

    /// Completed MCP tool call where we show the result serialized as JSON.
    CompletedMcpToolCall {
        view: TextBlock,
    },

    /// Completed MCP tool call where the result is an image.
    /// Admittedly, [mcp_types::CallToolResult] can have multiple content types,
    /// which could be a mix of text and images, so we need to tighten this up.
    // NOTE: For image output we keep the *original* image around and lazily
    // compute a resized copy that fits the available cell width.  Caching the
    // resized version avoids doing the potentially expensive rescale twice
    // because the scroll-view first calls `height()` for layouting and then
    // `render_window()` for painting.
    CompletedMcpToolCallWithImageOutput {
        _image: DynamicImage,
    },

    /// Background event.
    BackgroundEvent {
        view: TextBlock,
    },

    /// Output from the `/diff` command.
    GitDiffOutput {
        view: TextBlock,
    },

    /// Output from the `/status` command.
    StatusOutput {
        view: TextBlock,
    },

    /// Output from the `/prompts` command.
    PromptsOutput {
        view: TextBlock,
    },

    /// Error event from the backend.
    ErrorEvent {
        view: TextBlock,
    },

    /// Info describing the newly-initialized session.
    SessionInfo {
        view: TextBlock,
    },

    /// A pending code patch that is awaiting user approval. Mirrors the
    /// behaviour of `ExecCell` so the user sees *what* patch the
    /// model wants to apply before being prompted to approve or deny it.
    PendingPatch {
        view: TextBlock,
    },

    /// A human‑friendly rendering of the model's current plan and step
    /// statuses provided via the `update_plan` tool.
    PlanUpdate {
        view: TextBlock,
    },

    /// Result of applying a patch (success or failure) with optional output.
    PatchApplyResult {
        view: TextBlock,
    },
}

const TOOL_CALL_MAX_LINES: usize = 5;

fn title_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return String::new(),
    };
    let rest: String = chars.as_str().to_ascii_lowercase();
    first.to_uppercase().collect::<String>() + &rest
}

fn pretty_provider_name(id: &str) -> String {
    if id.eq_ignore_ascii_case("openai") {
        "OpenAI".to_string()
    } else {
        title_case(id)
    }
}

impl HistoryCell {
    /// Return a cloned, plain representation of the cell's lines suitable for
    /// one‑shot insertion into the terminal scrollback. Image cells are
    /// represented with a simple placeholder for now.
    /// These lines are also rendered directly by ratatui wrapped in a Paragraph.
    pub(crate) fn plain_lines(&self) -> Vec<Line<'static>> {
        match self {
            HistoryCell::WelcomeMessage { view }
            | HistoryCell::UserPrompt { view }
            | HistoryCell::BackgroundEvent { view }
            | HistoryCell::GitDiffOutput { view }
            | HistoryCell::StatusOutput { view }
            | HistoryCell::PromptsOutput { view }
            | HistoryCell::ErrorEvent { view }
            | HistoryCell::SessionInfo { view }
            | HistoryCell::CompletedMcpToolCall { view }
            | HistoryCell::PendingPatch { view }
            | HistoryCell::PlanUpdate { view }
            | HistoryCell::PatchApplyResult { view }
            | HistoryCell::ActiveMcpToolCall { view, .. } => {
                view.lines.iter().map(line_to_static).collect()
            }
            HistoryCell::Exec(ExecCell {
                command,
                parsed,
                output,
            }) => HistoryCell::exec_command_lines(command, parsed, output.as_ref()),
            HistoryCell::CompletedMcpToolCallWithImageOutput { .. } => vec![
                Line::from("tool result (image output omitted)"),
                Line::from(""),
            ],
        }
    }

    pub(crate) fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(self.plain_lines()))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    pub(crate) fn new_session_info(
        config: &Config,
        event: SessionConfiguredEvent,
        is_first_event: bool,
    ) -> Self {
        let SessionConfiguredEvent {
            model,
            session_id: _,
            history_log_id: _,
            history_entry_count: _,
        } = event;
        if is_first_event {
            let cwd_str = match relativize_to_home(&config.cwd) {
                Some(rel) if !rel.as_os_str().is_empty() => format!("~/{}", rel.display()),
                Some(_) => "~".to_string(),
                None => config.cwd.display().to_string(),
            };

            let lines: Vec<Line<'static>> = vec![
                Line::from(vec![
                    Span::raw(">_ ").dim(),
                    Span::styled(
                        "You are using OpenAI Codex in",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" {cwd_str}")).dim(),
                ]),
                Line::from("".dim()),
                Line::from(" To get started, describe a task or try one of these commands:".dim()),
                Line::from("".dim()),
                Line::from(format!(" /init - {}", SlashCommand::Init.description()).dim()),
                Line::from(format!(" /status - {}", SlashCommand::Status.description()).dim()),
                Line::from(format!(" /diff - {}", SlashCommand::Diff.description()).dim()),
                Line::from(format!(" /prompts - {}", SlashCommand::Prompts.description()).dim()),
                Line::from("".dim()),
            ];
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

    pub(crate) fn new_active_exec_command(
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
    ) -> Self {
        HistoryCell::new_exec_cell(command, parsed, None)
    }

    pub(crate) fn new_completed_exec_command(
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        output: CommandOutput,
    ) -> Self {
        HistoryCell::new_exec_cell(command, parsed, Some(output))
    }

    fn new_exec_cell(
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        output: Option<CommandOutput>,
    ) -> Self {
        HistoryCell::Exec(ExecCell {
            command,
            parsed,
            output,
        })
    }

    fn exec_command_lines(
        command: &[String],
        parsed: &[ParsedCommand],
        output: Option<&CommandOutput>,
    ) -> Vec<Line<'static>> {
        match parsed.is_empty() {
            true => HistoryCell::new_exec_command_generic(command, output),
            false => HistoryCell::new_parsed_command(parsed, output),
        }
    }

    fn new_parsed_command(
        parsed_commands: &[ParsedCommand],
        output: Option<&CommandOutput>,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = vec![Line::from("⚙︎ Working")];

        for (i, parsed) in parsed_commands.iter().enumerate() {
            let text = match parsed {
                ParsedCommand::Read { name, .. } => format!("📖 {name}"),
                ParsedCommand::ListFiles { cmd, path } => match path {
                    Some(p) => format!("📂 {p}"),
                    None => format!("📂 {}", shlex_join_safe(cmd)),
                },
                ParsedCommand::Search { query, path, cmd } => match (query, path) {
                    (Some(q), Some(p)) => format!("🔎 {q} in {p}"),
                    (Some(q), None) => format!("🔎 {q}"),
                    (None, Some(p)) => format!("🔎 {p}"),
                    (None, None) => format!("🔎 {}", shlex_join_safe(cmd)),
                },
                ParsedCommand::Format { .. } => "✨ Formatting".to_string(),
                ParsedCommand::Test { cmd } => format!("🧪 {}", shlex_join_safe(cmd)),
                ParsedCommand::Lint { cmd, .. } => format!("🧹 {}", shlex_join_safe(cmd)),
                ParsedCommand::Unknown { cmd } => format!("⌨️ {}", shlex_join_safe(cmd)),
            };

            let first_prefix = if i == 0 { "  L " } else { "    " };
            for (j, line_text) in text.lines().enumerate() {
                let prefix = if j == 0 { first_prefix } else { "    " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(line_text.to_string(), Style::default().fg(LIGHT_BLUE)),
                ]));
            }
        }

        lines.extend(output_lines(output, true, false));
        lines.push(Line::from(""));

        lines
    }

    fn new_exec_command_generic(
        command: &[String],
        output: Option<&CommandOutput>,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let command_escaped = strip_bash_lc_and_escape(command);
        let mut cmd_lines = command_escaped.lines();
        if let Some(first) = cmd_lines.next() {
            lines.push(Line::from(vec![
                "⚡ Running ".to_string().magenta(),
                first.to_string().into(),
            ]));
        } else {
            lines.push(Line::from("⚡ Running".to_string().magenta()));
        }
        for cont in cmd_lines {
            lines.push(Line::from(cont.to_string()));
        }

        lines.extend(output_lines(output, false, true));

        lines
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
        num_cols: usize,
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
                                    num_cols,
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
    // allow dead code for now. maybe we'll use it again.
    #[allow(dead_code)]
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

    pub(crate) fn new_status_output(config: &Config, usage: &TokenUsage) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("/status".magenta()));

        let config_entries = create_config_summary_entries(config);
        let lookup = |k: &str| -> String {
            config_entries
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };

        // 📂 Workspace
        lines.push(Line::from(vec!["📂 ".into(), "Workspace".bold()]));
        // Path (home-relative, e.g., ~/code/project)
        let cwd_str = match relativize_to_home(&config.cwd) {
            Some(rel) if !rel.as_os_str().is_empty() => format!("~/{}", rel.display()),
            Some(_) => "~".to_string(),
            None => config.cwd.display().to_string(),
        };
        lines.push(Line::from(vec!["  • Path: ".into(), cwd_str.into()]));
        // Approval mode (as-is)
        lines.push(Line::from(vec![
            "  • Approval Mode: ".into(),
            lookup("approval").into(),
        ]));
        // Sandbox (simplified name only)
        let sandbox_name = match &config.sandbox_policy {
            SandboxPolicy::DangerFullAccess => "danger-full-access",
            SandboxPolicy::ReadOnly => "read-only",
            SandboxPolicy::WorkspaceWrite { .. } => "workspace-write",
        };
        lines.push(Line::from(vec![
            "  • Sandbox: ".into(),
            sandbox_name.into(),
        ]));

        lines.push(Line::from(""));

        // 👤 Account (only if ChatGPT tokens exist), shown under the first block
        let auth_file = get_auth_file(&config.codex_home);
        if let Ok(auth) = try_read_auth_json(&auth_file) {
            if let Some(tokens) = auth.tokens.clone() {
                lines.push(Line::from(vec!["👤 ".into(), "Account".bold()]));
                lines.push(Line::from("  • Signed in with ChatGPT"));

                let info = tokens.id_token;
                if let Some(email) = &info.email {
                    lines.push(Line::from(vec!["  • Login: ".into(), email.clone().into()]));
                }

                match auth.openai_api_key.as_deref() {
                    Some(key) if !key.is_empty() => {
                        lines.push(Line::from(
                            "  • Using API key. Run codex login to use ChatGPT plan",
                        ));
                    }
                    _ => {
                        let plan_text = info
                            .get_chatgpt_plan_type()
                            .map(|s| title_case(&s))
                            .unwrap_or_else(|| "Unknown".to_string());
                        lines.push(Line::from(vec!["  • Plan: ".into(), plan_text.into()]));
                    }
                }

                lines.push(Line::from(""));
            }
        }

        // 🧠 Model
        lines.push(Line::from(vec!["🧠 ".into(), "Model".bold()]));
        lines.push(Line::from(vec![
            "  • Name: ".into(),
            config.model.clone().into(),
        ]));
        let provider_disp = pretty_provider_name(&config.model_provider_id);
        lines.push(Line::from(vec![
            "  • Provider: ".into(),
            provider_disp.into(),
        ]));
        // Only show Reasoning fields if present in config summary
        let reff = lookup("reasoning effort");
        if !reff.is_empty() {
            lines.push(Line::from(vec![
                "  • Reasoning Effort: ".into(),
                title_case(&reff).into(),
            ]));
        }
        let rsum = lookup("reasoning summaries");
        if !rsum.is_empty() {
            lines.push(Line::from(vec![
                "  • Reasoning Summaries: ".into(),
                title_case(&rsum).into(),
            ]));
        }

        lines.push(Line::from(""));

        // 📊 Token Usage
        lines.push(Line::from(vec!["📊 ".into(), "Token Usage".bold()]));
        // Input: <input> [+ <cached> cached]
        let mut input_line_spans: Vec<Span<'static>> = vec![
            "  • Input: ".into(),
            usage.non_cached_input().to_string().into(),
        ];
        if let Some(cached) = usage.cached_input_tokens {
            if cached > 0 {
                input_line_spans.push(format!(" (+ {cached} cached)").into());
            }
        }
        lines.push(Line::from(input_line_spans));
        // Output: <output>
        lines.push(Line::from(vec![
            "  • Output: ".into(),
            usage.output_tokens.to_string().into(),
        ]));
        // Total: <total>
        lines.push(Line::from(vec![
            "  • Total: ".into(),
            usage.blended_total().to_string().into(),
        ]));

        lines.push(Line::from(""));
        HistoryCell::StatusOutput {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_prompts_output() -> Self {
        let lines: Vec<Line<'static>> = vec![
            Line::from("/prompts".magenta()),
            Line::from(""),
            Line::from(" 1. Explain this codebase"),
            Line::from(" 2. Summarize recent commits"),
            Line::from(" 3. Implement {feature}"),
            Line::from(" 4. Find and fix a bug in @filename"),
            Line::from(" 5. Write tests for @filename"),
            Line::from(" 6. Improve documentation in @filename"),
            Line::from(""),
        ];
        HistoryCell::PromptsOutput {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_error_event(message: String) -> Self {
        let lines: Vec<Line<'static>> =
            vec![vec!["🖐 ".red().bold(), message.into()].into(), "".into()];
        HistoryCell::ErrorEvent {
            view: TextBlock::new(lines),
        }
    }

    /// Render a user‑friendly plan update styled like a checkbox todo list.
    pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> Self {
        let UpdatePlanArgs { explanation, plan } = update;

        let mut lines: Vec<Line<'static>> = Vec::new();
        // Header with progress summary
        let total = plan.len();
        let completed = plan
            .iter()
            .filter(|p| matches!(p.status, StepStatus::Completed))
            .count();

        let width: usize = 10;
        let filled = if total > 0 {
            (completed * width + total / 2) / total
        } else {
            0
        };
        let empty = width.saturating_sub(filled);

        let mut header: Vec<Span> = Vec::new();
        header.push(Span::raw("📋"));
        header.push(Span::styled(
            " Updated",
            Style::default().add_modifier(Modifier::BOLD).magenta(),
        ));
        header.push(Span::raw(" to do list ["));
        if filled > 0 {
            header.push(Span::styled(
                "█".repeat(filled),
                Style::default().fg(Color::Green),
            ));
        }
        if empty > 0 {
            header.push(Span::styled(
                "░".repeat(empty),
                Style::default().fg(Color::Gray),
            ));
        }
        header.push(Span::raw("] "));
        header.push(Span::raw(format!("{completed}/{total}")));
        lines.push(Line::from(header));

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

        // Steps styled as checkbox items
        if plan.is_empty() {
            lines.push(Line::from("(no steps provided)".gray().italic()));
        } else {
            for (idx, PlanItemArg { step, status }) in plan.into_iter().enumerate() {
                let (box_span, text_span) = match status {
                    StepStatus::Completed => (
                        Span::styled("✔", Style::default().fg(Color::Green)),
                        Span::styled(
                            step,
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::CROSSED_OUT | Modifier::DIM),
                        ),
                    ),
                    StepStatus::InProgress => (
                        Span::raw("□"),
                        Span::styled(
                            step,
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ),
                    StepStatus::Pending => (
                        Span::raw("□"),
                        Span::styled(
                            step,
                            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                        ),
                    ),
                };
                let prefix = if idx == 0 {
                    Span::raw("  ⎿ ")
                } else {
                    Span::raw("    ")
                };
                lines.push(Line::from(vec![
                    prefix,
                    box_span,
                    Span::raw(" "),
                    text_span,
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
        let title = match &event_type {
            PatchEventType::ApprovalRequest => "proposed patch",
            PatchEventType::ApplyBegin {
                auto_approved: true,
            } => "✏️ Applying patch",
            PatchEventType::ApplyBegin {
                auto_approved: false,
            } => {
                let lines: Vec<Line<'static>> = vec![
                    Line::from("✏️ Applying patch".magenta().bold()),
                    Line::from(""),
                ];
                return Self::PendingPatch {
                    view: TextBlock::new(lines),
                };
            }
        };

        let mut lines: Vec<Line<'static>> = create_diff_summary(title, &changes, event_type);

        lines.push(Line::from(""));

        HistoryCell::PendingPatch {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_patch_apply_failure(stderr: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Failure title
        lines.push(Line::from("✘ Failed to apply patch".magenta().bold()));

        if !stderr.trim().is_empty() {
            lines.extend(output_lines(
                Some(&CommandOutput {
                    exit_code: 1,
                    stdout: String::new(),
                    stderr,
                }),
                true,
                true,
            ));
        }

        lines.push(Line::from(""));

        HistoryCell::PatchApplyResult {
            view: TextBlock::new(lines),
        }
    }
}

impl WidgetRef for &HistoryCell {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Text::from(self.plain_lines()))
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

fn output_lines(
    output: Option<&CommandOutput>,
    only_err: bool,
    include_angle_pipe: bool,
) -> Vec<Line<'static>> {
    let CommandOutput {
        exit_code,
        stdout,
        stderr,
    } = match output {
        Some(output) if only_err && output.exit_code == 0 => return vec![],
        Some(output) => output,
        None => return vec![],
    };

    let src = if *exit_code == 0 { stdout } else { stderr };
    let lines: Vec<&str> = src.lines().collect();
    let total = lines.len();
    let limit = TOOL_CALL_MAX_LINES;

    let mut out = Vec::new();

    let head_end = total.min(limit);
    for (i, raw) in lines[..head_end].iter().enumerate() {
        let mut line = ansi_escape_line(raw);
        let prefix = if i == 0 && include_angle_pipe {
            "  ⎿ "
        } else {
            "    "
        };
        line.spans.insert(0, prefix.into());
        line.spans.iter_mut().for_each(|span| {
            span.style = span.style.add_modifier(Modifier::DIM);
        });
        out.push(line);
    }

    // If we will ellipsize less than the limit, just show it.
    let show_ellipsis = total > 2 * limit;
    if show_ellipsis {
        let omitted = total - 2 * limit;
        out.push(Line::from(format!("… +{omitted} lines")));
    }

    let tail_start = if show_ellipsis {
        total - limit
    } else {
        head_end
    };
    for raw in lines[tail_start..].iter() {
        let mut line = ansi_escape_line(raw);
        line.spans.insert(0, "    ".into());
        line.spans.iter_mut().for_each(|span| {
            span.style = span.style.add_modifier(Modifier::DIM);
        });
        out.push(line);
    }

    out
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

fn shlex_join_safe(command: &[String]) -> String {
    match shlex::try_join(command.iter().map(|s| s.as_str())) {
        Ok(cmd) => cmd,
        Err(_) => command.join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_command_with_newlines_starts_each_line_at_origin() {
        let parsed = vec![ParsedCommand::Unknown {
            cmd: vec!["printf".into(), "foo\nbar".into()],
        }];
        let lines = HistoryCell::exec_command_lines(&[], &parsed, None);
        assert!(lines.len() >= 3);
        assert_eq!(lines[1].spans[0].content, "  L ");
        assert_eq!(lines[2].spans[0].content, "    ");
    }
}
