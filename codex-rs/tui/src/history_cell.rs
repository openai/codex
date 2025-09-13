use crate::diff_render::create_diff_summary;
use crate::exec_command::relativize_to_home;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::markdown::append_markdown;
use crate::render::line_utils::line_to_static;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::slash_command::SlashCommand;
use crate::text_formatting::format_and_truncate_tool_result;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use crate::wrapping::word_wrap_lines;
use base64::Engine;
use codex_ansi_escape::ansi_escape_line;
use codex_common::create_config_summary_entries;
use codex_common::elapsed::format_duration;
use codex_core::auth::get_auth_file;
use codex_core::auth::try_read_auth_json;
use codex_core::config::Config;
use codex_core::config_types::ReasoningSummaryFormat;
use codex_core::plan_tool::PlanItemArg;
use codex_core::plan_tool::StepStatus;
use codex_core::plan_tool::UpdatePlanArgs;
use codex_core::project_doc::discover_project_doc_paths;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::num_format::format_with_separators;
use codex_protocol::parse_command::ParsedCommand;
use image::DynamicImage;
use image::ImageReader;
use itertools::Itertools;
use mcp_types::EmbeddedResourceResource;
use mcp_types::ResourceLink;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::any::Any;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tracing::error;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) formatted_output: String,
}

#[derive(Clone, Debug)]
pub(crate) enum PatchEventType {
    ApprovalRequest,
    ApplyBegin { auto_approved: bool },
}

/// Represents an event to display in the conversation history. Returns its
/// `Vec<Line<'static>>` representation to make it easier to display in a
/// scrollable list.
pub(crate) trait HistoryCell: std::fmt::Debug + Send + Sync + Any {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        self.display_lines(u16::MAX)
    }

    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(self.display_lines(width)))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    fn is_stream_continuation(&self) -> bool {
        false
    }
}

impl dyn HistoryCell {
    pub(crate) fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
pub(crate) struct UserHistoryCell {
    pub message: String,
}

impl HistoryCell for UserHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Wrap the content first, then prefix each wrapped line with the marker.
        let wrap_width = width.saturating_sub(1); // account for the ▌ prefix
        let wrapped = textwrap::wrap(
            &self.message,
            textwrap::Options::new(wrap_width as usize)
                .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit), // Match textarea wrap
        );

        for line in wrapped {
            lines.push(vec!["▌".cyan().dim(), line.to_string().dim()].into());
        }
        lines
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push("user".cyan().bold().into());
        lines.extend(self.message.lines().map(|l| l.to_string().into()));
        lines
    }
}

#[derive(Debug)]
pub(crate) struct AgentMessageCell {
    lines: Vec<Line<'static>>,
    is_first_line: bool,
}

impl AgentMessageCell {
    pub(crate) fn new(lines: Vec<Line<'static>>, is_first_line: bool) -> Self {
        Self {
            lines,
            is_first_line,
        }
    }
}

impl HistoryCell for AgentMessageCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        word_wrap_lines(
            &self.lines,
            RtOptions::new(width as usize)
                .initial_indent(if self.is_first_line {
                    "> ".into()
                } else {
                    "  ".into()
                })
                .subsequent_indent("  ".into()),
        )
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        if self.is_first_line {
            out.push("codex".magenta().bold().into());
        }
        out.extend(self.lines.clone());
        out
    }

    fn is_stream_continuation(&self) -> bool {
        !self.is_first_line
    }
}

#[derive(Debug)]
pub(crate) struct PlainHistoryCell {
    lines: Vec<Line<'static>>,
}

impl HistoryCell for PlainHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

#[derive(Debug)]
pub(crate) struct TranscriptOnlyHistoryCell {
    lines: Vec<Line<'static>>,
}

impl HistoryCell for TranscriptOnlyHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        Vec::new()
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
    event_type: PatchEventType,
    changes: HashMap<PathBuf, FileChange>,
    cwd: PathBuf,
}

impl HistoryCell for PatchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        create_diff_summary(
            &self.changes,
            self.event_type.clone(),
            &self.cwd,
            width as usize,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
    start_time: Option<Instant>,
    duration: Option<Duration>,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    calls: Vec<ExecCall>,
}
impl HistoryCell for ExecCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.is_exploring_cell() {
            self.exploring_display_lines(width)
        } else {
            self.command_display_lines(width)
        }
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![];
        for call in &self.calls {
            let cmd_display = strip_bash_lc_and_escape(&call.command);
            for (i, part) in cmd_display.lines().enumerate() {
                if i == 0 {
                    lines.push(vec!["$ ".magenta(), part.to_string().into()].into());
                } else {
                    lines.push(vec!["    ".into(), part.to_string().into()].into());
                }
            }

            if let Some(output) = call.output.as_ref() {
                lines.extend(output.formatted_output.lines().map(ansi_escape_line));
                let duration = call
                    .duration
                    .map(format_duration)
                    .unwrap_or_else(|| "unknown".to_string());
                let mut result: Line = if output.exit_code == 0 {
                    Line::from("✓".green().bold())
                } else {
                    Line::from(vec![
                        "✗".red().bold(),
                        format!(" ({})", output.exit_code).into(),
                    ])
                };
                result.push_span(format!(" • {duration}").dim());
                lines.push(result);
            }
            lines.push("".into());
        }
        lines
    }
}

impl ExecCell {
    fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.output.is_none())
    }

    fn exploring_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        let active_start_time = self
            .calls
            .iter()
            .find(|c| c.output.is_none())
            .and_then(|c| c.start_time);
        out.push(Line::from(vec![
            if self.is_active() {
                // Show an animated spinner while exploring
                spinner(active_start_time)
            } else {
                "•".bold()
            },
            " ".into(),
            if self.is_active() {
                "Exploring".bold()
            } else {
                "Explored".bold()
            },
        ]));
        let mut calls = self.calls.clone();
        let mut out_indented = Vec::new();
        while !calls.is_empty() {
            let mut call = calls.remove(0);
            if call
                .parsed
                .iter()
                .all(|c| matches!(c, ParsedCommand::Read { .. }))
            {
                while let Some(next) = calls.first() {
                    if next
                        .parsed
                        .iter()
                        .all(|c| matches!(c, ParsedCommand::Read { .. }))
                    {
                        call.parsed.extend(next.parsed.clone());
                        calls.remove(0);
                    } else {
                        break;
                    }
                }
            }
            let call_lines: Vec<(&str, Vec<Span<'static>>)> = if call
                .parsed
                .iter()
                .all(|c| matches!(c, ParsedCommand::Read { .. }))
            {
                let names: Vec<String> = call
                    .parsed
                    .iter()
                    .map(|c| match c {
                        ParsedCommand::Read { name, .. } => name.clone(),
                        _ => unreachable!(),
                    })
                    .unique()
                    .collect();
                vec![(
                    "Read",
                    itertools::Itertools::intersperse(
                        names.into_iter().map(|n| n.into()),
                        ", ".dim(),
                    )
                    .collect(),
                )]
            } else {
                let mut lines = Vec::new();
                for p in call.parsed {
                    match p {
                        ParsedCommand::Read { name, .. } => {
                            lines.push(("Read", vec![name.into()]));
                        }
                        ParsedCommand::ListFiles { cmd, path } => {
                            lines.push(("List", vec![path.unwrap_or(cmd).into()]));
                        }
                        ParsedCommand::Search { cmd, query, path } => {
                            lines.push((
                                "Search",
                                match (query, path) {
                                    (Some(q), Some(p)) => {
                                        vec![q.into(), " in ".dim(), p.into()]
                                    }
                                    (Some(q), None) => vec![q.into()],
                                    _ => vec![cmd.into()],
                                },
                            ));
                        }
                        ParsedCommand::Unknown { cmd } => {
                            lines.push(("Run", vec![cmd.into()]));
                        }
                    }
                }
                lines
            };
            for (title, line) in call_lines {
                let line = Line::from(line);
                let initial_indent = Line::from(vec![title.cyan(), " ".into()]);
                let subsequent_indent = " ".repeat(initial_indent.width()).into();
                let wrapped = word_wrap_line(
                    &line,
                    RtOptions::new(width as usize)
                        .initial_indent(initial_indent)
                        .subsequent_indent(subsequent_indent),
                );
                push_owned_lines(&wrapped, &mut out_indented);
            }
        }
        out.extend(prefix_lines(out_indented, "  └ ".dim(), "    ".into()));
        out
    }

    fn command_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        use textwrap::Options as TwOptions;

        let mut lines: Vec<Line<'static>> = Vec::new();
        let [call] = &self.calls.as_slice() else {
            panic!("Expected exactly one call in a command display cell");
        };
        let success = call.output.as_ref().map(|o| o.exit_code == 0);
        let bullet = match success {
            Some(true) => "•".green().bold(),
            Some(false) => "•".red().bold(),
            None => spinner(call.start_time),
        };
        let title = if self.is_active() { "Running" } else { "Ran" };
        let cmd_display = strip_bash_lc_and_escape(&call.command);

        // If the command fits on the same line as the header at the current width,
        // show a single compact line: "• Ran <command>". Use the width of
        // "• Running " (including trailing space) as the reserved prefix width.
        // If the command contains newlines, always use the multi-line variant.
        let reserved = "• Running ".width();

        let mut body_lines: Vec<Line<'static>> = Vec::new();

        let highlighted_lines = crate::render::highlight::highlight_bash_to_lines(&cmd_display);

        if highlighted_lines.len() == 1
            && highlighted_lines[0].width() < (width as usize).saturating_sub(reserved)
        {
            let mut line = Line::from(vec![bullet, " ".into(), title.bold(), " ".into()]);
            line.extend(highlighted_lines[0].clone());
            lines.push(line);
        } else {
            lines.push(vec![bullet, " ".into(), title.bold()].into());

            for hl_line in highlighted_lines.iter() {
                let opts = crate::wrapping::RtOptions::new((width as usize).saturating_sub(4))
                    .initial_indent("".into())
                    .subsequent_indent("    ".into())
                    // Hyphenation likes to break words on hyphens, which is bad for bash scripts --because-of-flags.
                    .word_splitter(textwrap::WordSplitter::NoHyphenation);
                let wrapped_borrowed = crate::wrapping::word_wrap_line(hl_line, opts);
                body_lines.extend(wrapped_borrowed.iter().map(|l| line_to_static(l)));
            }
        }
        if let Some(output) = call.output.as_ref()
            && output.exit_code != 0
        {
            let out = output_lines(Some(output), false, false, false)
                .into_iter()
                .join("\n");
            if !out.trim().is_empty() {
                // Wrap the output.
                for line in out.lines() {
                    let wrapped = textwrap::wrap(line, TwOptions::new(width as usize - 4));
                    body_lines.extend(wrapped.into_iter().map(|l| Line::from(l.to_string().dim())));
                }
            }
        }
        lines.extend(prefix_lines(body_lines, "  └ ".dim(), "    ".into()));
        lines
    }
}

impl WidgetRef for &ExecCell {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let content_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        };
        let lines = self.display_lines(area.width);
        let max_rows = area.height as usize;
        let rendered = if lines.len() > max_rows {
            // Keep the last `max_rows` lines in original order
            lines[lines.len() - max_rows..].to_vec()
        } else {
            lines
        };

        Paragraph::new(Text::from(rendered))
            .wrap(Wrap { trim: false })
            .render(content_area, buf);
    }
}

impl ExecCell {
    /// Convert an active exec cell into a failed, completed exec cell.
    /// Any call without output is marked as failed with a red ✗.
    pub(crate) fn into_failed(mut self) -> ExecCell {
        for call in self.calls.iter_mut() {
            if call.output.is_none() {
                let elapsed = call
                    .start_time
                    .map(|st| st.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(0));
                call.start_time = None;
                call.duration = Some(elapsed);
                call.output = Some(CommandOutput {
                    exit_code: 1,
                    stdout: String::new(),
                    stderr: String::new(),
                    formatted_output: String::new(),
                });
            }
        }
        self
    }

    pub(crate) fn new(call: ExecCall) -> Self {
        ExecCell { calls: vec![call] }
    }

    fn is_exploring_call(call: &ExecCall) -> bool {
        !call.parsed.is_empty()
            && call.parsed.iter().all(|p| {
                matches!(
                    p,
                    ParsedCommand::Read { .. }
                        | ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                )
            })
    }

    fn is_exploring_cell(&self) -> bool {
        self.calls.iter().all(Self::is_exploring_call)
    }

    pub(crate) fn with_added_call(
        &self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
    ) -> Option<Self> {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        };
        if self.is_exploring_cell() && Self::is_exploring_call(&call) {
            Some(Self {
                calls: [self.calls.clone(), vec![call]].concat(),
            })
        } else {
            None
        }
    }

    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) {
        if let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) {
            call.output = Some(output);
            call.duration = Some(duration);
            call.start_time = None;
        }
    }

    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && self.calls.iter().all(|c| c.output.is_some())
    }
}

#[derive(Debug)]
struct CompletedMcpToolCallWithImageOutput {
    _image: DynamicImage,
}
impl HistoryCell for CompletedMcpToolCallWithImageOutput {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["tool result (image output omitted)".into()]
    }
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
/// Return the emoji followed by a hair space (U+200A).
/// Using only the hair space avoids excessive padding after the emoji while
/// still providing a small visual gap across terminals.
fn padded_emoji(emoji: &str) -> String {
    format!("{emoji}\u{200A}")
}

pub(crate) fn new_session_info(
    config: &Config,
    event: SessionConfiguredEvent,
    is_first_event: bool,
) -> PlainHistoryCell {
    let SessionConfiguredEvent {
        model,
        reasoning_effort: _,
        session_id: _,
        history_log_id: _,
        history_entry_count: _,
        initial_messages: _,
        rollout_path: _,
    } = event;
    if is_first_event {
        let cwd_str = match relativize_to_home(&config.cwd) {
            Some(rel) if !rel.as_os_str().is_empty() => {
                let sep = std::path::MAIN_SEPARATOR;
                format!("~{sep}{}", rel.display())
            }
            Some(_) => "~".to_string(),
            None => config.cwd.display().to_string(),
        };
        // Discover AGENTS.md files to decide whether to suggest `/init`.
        let has_agents_md = discover_project_doc_paths(config)
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            ">_ ".dim(),
            "You are using OpenAI Codex in".bold(),
            format!(" {cwd_str}").dim(),
        ]));
        lines.push(Line::from("".dim()));
        lines.push(Line::from(
            " To get started, describe a task or try one of these commands:".dim(),
        ));
        lines.push(Line::from("".dim()));
        if !has_agents_md {
            lines.push(Line::from(vec![
                " /init".bold(),
                format!(" - {}", SlashCommand::Init.description()).dim(),
            ]));
        }
        lines.push(Line::from(vec![
            " /status".bold(),
            format!(" - {}", SlashCommand::Status.description()).dim(),
        ]));
        lines.push(Line::from(vec![
            " /approvals".bold(),
            format!(" - {}", SlashCommand::Approvals.description()).dim(),
        ]));
        lines.push(Line::from(vec![
            " /model".bold(),
            format!(" - {}", SlashCommand::Model.description()).dim(),
        ]));
        PlainHistoryCell { lines }
    } else if config.model == model {
        PlainHistoryCell { lines: Vec::new() }
    } else {
        let lines = vec![
            "model changed:".magenta().bold().into(),
            format!("requested: {}", config.model).into(),
            format!("used: {model}").into(),
        ];
        PlainHistoryCell { lines }
    }
}

pub(crate) fn new_user_prompt(message: String) -> UserHistoryCell {
    UserHistoryCell { message }
}

pub(crate) fn new_user_approval_decision(lines: Vec<Line<'static>>) -> PlainHistoryCell {
    PlainHistoryCell { lines }
}

pub(crate) fn new_active_exec_command(
    call_id: String,
    command: Vec<String>,
    parsed: Vec<ParsedCommand>,
) -> ExecCell {
    ExecCell::new(ExecCall {
        call_id,
        command,
        parsed,
        output: None,
        start_time: Some(Instant::now()),
        duration: None,
    })
}

fn spinner(start_time: Option<Instant>) -> Span<'static> {
    const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let idx = start_time
        .map(|st| ((st.elapsed().as_millis() / 100) as usize) % FRAMES.len())
        .unwrap_or(0);
    let ch = FRAMES[idx];
    ch.to_string().into()
}

pub(crate) fn new_active_mcp_tool_call(invocation: McpInvocation) -> PlainHistoryCell {
    let title_line = Line::from(vec!["tool".magenta(), " running...".dim()]);
    let lines: Vec<Line> = vec![title_line, format_mcp_invocation(invocation)];

    PlainHistoryCell { lines }
}

pub(crate) fn new_web_search_call(query: String) -> PlainHistoryCell {
    let lines: Vec<Line<'static>> = vec![Line::from(vec![padded_emoji("🌐").into(), query.into()])];
    PlainHistoryCell { lines }
}

/// If the first content is an image, return a new cell with the image.
/// TODO(rgwood-dd): Handle images properly even if they're not the first result.
fn try_new_completed_mcp_tool_call_with_image_output(
    result: &Result<mcp_types::CallToolResult, String>,
) -> Option<CompletedMcpToolCallWithImageOutput> {
    match result {
        Ok(mcp_types::CallToolResult { content, .. }) => {
            if let Some(mcp_types::ContentBlock::ImageContent(image)) = content.first() {
                let raw_data = match base64::engine::general_purpose::STANDARD.decode(&image.data) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to decode image data: {e}");
                        return None;
                    }
                };
                let reader = match ImageReader::new(Cursor::new(raw_data)).with_guessed_format() {
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

                Some(CompletedMcpToolCallWithImageOutput { _image: image })
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
) -> Box<dyn HistoryCell> {
    if let Some(cell) = try_new_completed_mcp_tool_call_with_image_output(&result) {
        return Box::new(cell);
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
        format!(", duration: {duration}").dim(),
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
                        mcp_types::ContentBlock::AudioContent(_) => "<audio content>".to_string(),
                        mcp_types::ContentBlock::EmbeddedResource(resource) => {
                            let uri = match resource.resource {
                                EmbeddedResourceResource::TextResourceContents(text) => text.uri,
                                EmbeddedResourceResource::BlobResourceContents(blob) => blob.uri,
                            };
                            format!("embedded resource: {uri}")
                        }
                        mcp_types::ContentBlock::ResourceLink(ResourceLink { uri, .. }) => {
                            format!("link: {uri}")
                        }
                    };
                    lines.push(Line::styled(
                        line_text,
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
            }
        }
        Err(e) => {
            lines.push(vec!["Error: ".red().bold(), e.into()].into());
        }
    };

    Box::new(PlainHistoryCell { lines })
}

pub(crate) fn new_status_output(
    config: &Config,
    usage: &TokenUsage,
    session_id: &Option<ConversationId>,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push("/status".magenta().into());

    let config_entries = create_config_summary_entries(config);
    let lookup = |k: &str| -> String {
        config_entries
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // 📂 Workspace
    lines.push(vec![padded_emoji("📂").into(), "Workspace".bold()].into());
    // Path (home-relative, e.g., ~/code/project)
    let cwd_str = match relativize_to_home(&config.cwd) {
        Some(rel) if !rel.as_os_str().is_empty() => {
            let sep = std::path::MAIN_SEPARATOR;
            format!("~{sep}{}", rel.display())
        }
        Some(_) => "~".to_string(),
        None => config.cwd.display().to_string(),
    };
    lines.push(vec!["  • Path: ".into(), cwd_str.into()].into());
    // Approval mode (as-is)
    lines.push(vec!["  • Approval Mode: ".into(), lookup("approval").into()].into());
    // Sandbox (simplified name only)
    let sandbox_name = match &config.sandbox_policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access",
        SandboxPolicy::ReadOnly => "read-only",
        SandboxPolicy::WorkspaceWrite { .. } => "workspace-write",
    };
    lines.push(vec!["  • Sandbox: ".into(), sandbox_name.into()].into());

    // AGENTS.md files discovered via core's project_doc logic
    let agents_list = {
        match discover_project_doc_paths(config) {
            Ok(paths) => {
                let mut rels: Vec<String> = Vec::new();
                for p in paths {
                    let display = if let Some(parent) = p.parent() {
                        if parent == config.cwd {
                            "AGENTS.md".to_string()
                        } else {
                            let mut cur = config.cwd.as_path();
                            let mut ups = 0usize;
                            let mut reached = false;
                            while let Some(c) = cur.parent() {
                                if cur == parent {
                                    reached = true;
                                    break;
                                }
                                cur = c;
                                ups += 1;
                            }
                            if reached {
                                let up = format!("..{}", std::path::MAIN_SEPARATOR);
                                format!("{}AGENTS.md", up.repeat(ups))
                            } else if let Ok(stripped) = p.strip_prefix(&config.cwd) {
                                stripped.display().to_string()
                            } else {
                                p.display().to_string()
                            }
                        }
                    } else {
                        p.display().to_string()
                    };
                    rels.push(display);
                }
                rels
            }
            Err(_) => Vec::new(),
        }
    };
    if agents_list.is_empty() {
        lines.push("  • AGENTS files: (none)".into());
    } else {
        lines.push(vec!["  • AGENTS files: ".into(), agents_list.join(", ").into()].into());
    }
    lines.push("".into());

    // 👤 Account (only if ChatGPT tokens exist), shown under the first block
    let auth_file = get_auth_file(&config.codex_home);
    if let Ok(auth) = try_read_auth_json(&auth_file)
        && let Some(tokens) = auth.tokens.clone()
    {
        lines.push(vec![padded_emoji("👤").into(), "Account".bold()].into());
        lines.push("  • Signed in with ChatGPT".into());

        let info = tokens.id_token;
        if let Some(email) = &info.email {
            lines.push(vec!["  • Login: ".into(), email.clone().into()].into());
        }

        match auth.openai_api_key.as_deref() {
            Some(key) if !key.is_empty() => {
                lines.push("  • Using API key. Run codex login to use ChatGPT plan".into());
            }
            _ => {
                let plan_text = info
                    .get_chatgpt_plan_type()
                    .map(|s| title_case(&s))
                    .unwrap_or_else(|| "Unknown".to_string());
                lines.push(vec!["  • Plan: ".into(), plan_text.into()].into());
            }
        }

        lines.push("".into());
    }

    // 🧠 Model
    lines.push(vec![padded_emoji("🧠").into(), "Model".bold()].into());
    lines.push(vec!["  • Name: ".into(), config.model.clone().into()].into());
    let provider_disp = pretty_provider_name(&config.model_provider_id);
    lines.push(vec!["  • Provider: ".into(), provider_disp.into()].into());
    // Only show Reasoning fields if present in config summary
    let reff = lookup("reasoning effort");
    if !reff.is_empty() {
        lines.push(vec!["  • Reasoning Effort: ".into(), title_case(&reff).into()].into());
    }
    let rsum = lookup("reasoning summaries");
    if !rsum.is_empty() {
        lines.push(vec!["  • Reasoning Summaries: ".into(), title_case(&rsum).into()].into());
    }

    lines.push("".into());

    // 💻 Client
    let cli_version = crate::version::CODEX_CLI_VERSION;
    lines.push(vec![padded_emoji("💻").into(), "Client".bold()].into());
    lines.push(vec!["  • CLI Version: ".into(), cli_version.into()].into());
    lines.push("".into());

    // 📊 Token Usage
    lines.push(vec!["📊 ".into(), "Token Usage".bold()].into());
    if let Some(session_id) = session_id {
        lines.push(vec!["  • Session ID: ".into(), session_id.to_string().into()].into());
    }
    // Input: <input> [+ <cached> cached]
    let mut input_line_spans: Vec<Span<'static>> = vec![
        "  • Input: ".into(),
        format_with_separators(usage.non_cached_input()).into(),
    ];
    if usage.cached_input_tokens > 0 {
        let cached = usage.cached_input_tokens;
        input_line_spans.push(format!(" (+ {cached} cached)").into());
    }
    lines.push(Line::from(input_line_spans));
    // Output: <output>
    lines.push(Line::from(vec![
        "  • Output: ".into(),
        format_with_separators(usage.output_tokens).into(),
    ]));
    // Total: <total>
    lines.push(Line::from(vec![
        "  • Total: ".into(),
        format_with_separators(usage.blended_total()).into(),
    ]));

    PlainHistoryCell { lines }
}

/// Render a summary of configured MCP servers from the current `Config`.
pub(crate) fn empty_mcp_output() -> PlainHistoryCell {
    let lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
        "  • No MCP servers configured.".italic().into(),
        Line::from(vec![
            "    See the ".into(),
            "\u{1b}]8;;https://github.com/openai/codex/blob/main/docs/config.md#mcp_servers\u{7}MCP docs\u{1b}]8;;\u{7}".underlined(),
            " to configure them.".into(),
        ])
        .style(Style::default().add_modifier(Modifier::DIM)),
    ];

    PlainHistoryCell { lines }
}

/// Render MCP tools grouped by connection using the fully-qualified tool names.
pub(crate) fn new_mcp_tools_output(
    config: &Config,
    tools: std::collections::HashMap<String, mcp_types::Tool>,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
    ];

    if tools.is_empty() {
        lines.push("  • No MCP tools available.".italic().into());
        lines.push("".into());
        return PlainHistoryCell { lines };
    }

    for (server, cfg) in config.mcp_servers.iter() {
        let prefix = format!("{server}__");
        let mut names: Vec<String> = tools
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect();
        names.sort();

        lines.push(vec!["  • Server: ".into(), server.clone().into()].into());

        if !cfg.command.is_empty() {
            let cmd_display = format!("{} {}", cfg.command, cfg.args.join(" "));

            lines.push(vec!["    • Command: ".into(), cmd_display.into()].into());
        }

        if names.is_empty() {
            lines.push("    • Tools: (none)".into());
        } else {
            lines.push(vec!["    • Tools: ".into(), names.join(", ").into()].into());
        }
        lines.push(Line::from(""));
    }

    PlainHistoryCell { lines }
}

pub(crate) fn new_info_event(message: String, hint: Option<String>) -> PlainHistoryCell {
    let mut line = vec!["> ".into(), message.into()];
    if let Some(hint) = hint {
        line.push(" ".into());
        line.push(hint.dark_gray());
    }
    let lines: Vec<Line<'static>> = vec![line.into()];
    PlainHistoryCell { lines }
}

pub(crate) fn new_error_event(message: String) -> PlainHistoryCell {
    // Use a hair space (U+200A) to create a subtle, near-invisible separation
    // before the text. VS16 is intentionally omitted to keep spacing tighter
    // in terminals like Ghostty.
    let lines: Vec<Line<'static>> = vec![vec![format!("■ {message}").red()].into()];
    PlainHistoryCell { lines }
}

pub(crate) fn new_stream_error_event(message: String) -> PlainHistoryCell {
    let lines: Vec<Line<'static>> = vec![vec![padded_emoji("⚠️").into(), message.dim()].into()];
    PlainHistoryCell { lines }
}

/// Render a user‑friendly plan update styled like a checkbox todo list.
pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> PlanUpdateCell {
    let UpdatePlanArgs { explanation, plan } = update;
    PlanUpdateCell { explanation, plan }
}

#[derive(Debug)]
pub(crate) struct PlanUpdateCell {
    explanation: Option<String>,
    plan: Vec<PlanItemArg>,
}

impl HistoryCell for PlanUpdateCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let render_note = |text: &str| -> Vec<Line<'static>> {
            let wrap_width = width.saturating_sub(4).max(1) as usize;
            textwrap::wrap(text, wrap_width)
                .into_iter()
                .map(|s| s.to_string().dim().italic().into())
                .collect()
        };

        let render_step = |status: &StepStatus, text: &str| -> Vec<Line<'static>> {
            let (box_str, step_style) = match status {
                StepStatus::Completed => ("✔ ", Style::default().crossed_out().dim()),
                StepStatus::InProgress => ("□ ", Style::default().cyan().bold()),
                StepStatus::Pending => ("□ ", Style::default().dim()),
            };
            let wrap_width = (width as usize)
                .saturating_sub(4)
                .saturating_sub(box_str.width())
                .max(1);
            let parts = textwrap::wrap(text, wrap_width);
            let step_text = parts
                .into_iter()
                .map(|s| s.to_string().set_style(step_style).into())
                .collect();
            prefix_lines(step_text, box_str.into(), "  ".into())
        };

        let mut lines: Vec<Line<'static>> = vec![];
        lines.push(vec!["• ".into(), "Updated Plan".bold()].into());

        let mut indented_lines = vec![];
        let note = self
            .explanation
            .as_ref()
            .map(|s| s.trim())
            .filter(|t| !t.is_empty());
        if let Some(expl) = note {
            indented_lines.extend(render_note(expl));
        };

        if self.plan.is_empty() {
            indented_lines.push(Line::from("(no steps provided)".dim().italic()));
        } else {
            for PlanItemArg { step, status } in self.plan.iter() {
                indented_lines.extend(render_step(status, step));
            }
        }
        lines.extend(prefix_lines(indented_lines, "  └ ".into(), "    ".into()));

        lines
    }
}

/// Create a new `PendingPatch` cell that lists the file‑level summary of
/// a proposed patch. The summary lines should already be formatted (e.g.
/// "A path/to/file.rs").
pub(crate) fn new_patch_event(
    event_type: PatchEventType,
    changes: HashMap<PathBuf, FileChange>,
    cwd: &Path,
) -> PatchHistoryCell {
    PatchHistoryCell {
        event_type,
        changes,
        cwd: cwd.to_path_buf(),
    }
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Failure title
    lines.push(Line::from("✘ Failed to apply patch".magenta().bold()));

    if !stderr.trim().is_empty() {
        lines.extend(output_lines(
            Some(&CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr,
                formatted_output: String::new(),
            }),
            true,
            true,
            true,
        ));
    }

    PlainHistoryCell { lines }
}

/// Create a new history cell for a proposed command approval.
/// Renders a header and the command preview similar to how proposed patches
/// show a header and summary.
pub(crate) fn new_proposed_command(command: &[String]) -> PlainHistoryCell {
    let cmd = strip_bash_lc_and_escape(command);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(vec!["• ".into(), "Proposed Command".bold()]));

    let highlighted_lines = crate::render::highlight::highlight_bash_to_lines(&cmd);
    let initial_prefix: Span<'static> = "  └ ".dim();
    let subsequent_prefix: Span<'static> = "    ".into();
    lines.extend(prefix_lines(
        highlighted_lines,
        initial_prefix,
        subsequent_prefix,
    ));

    PlainHistoryCell { lines }
}

pub(crate) fn new_reasoning_block(
    full_reasoning_buffer: String,
    config: &Config,
) -> TranscriptOnlyHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("thinking".magenta().italic()));
    append_markdown(&full_reasoning_buffer, &mut lines, config);
    TranscriptOnlyHistoryCell { lines }
}

pub(crate) fn new_reasoning_summary_block(
    full_reasoning_buffer: String,
    config: &Config,
) -> Vec<Box<dyn HistoryCell>> {
    if config.model_family.reasoning_summary_format == ReasoningSummaryFormat::Experimental {
        // Experimental format is following:
        // ** header **
        //
        // reasoning summary
        //
        // So we need to strip header from reasoning summary
        let full_reasoning_buffer = full_reasoning_buffer.trim();
        if let Some(open) = full_reasoning_buffer.find("**") {
            let after_open = &full_reasoning_buffer[(open + 2)..];
            if let Some(close) = after_open.find("**") {
                let after_close_idx = open + 2 + close + 2;
                // if we don't have anything beyond `after_close_idx`
                // then we don't have a summary to inject into history
                if after_close_idx < full_reasoning_buffer.len() {
                    let header_buffer = full_reasoning_buffer[..after_close_idx].to_string();
                    let summary_buffer = full_reasoning_buffer[after_close_idx..].to_string();

                    let mut header_lines: Vec<Line<'static>> = Vec::new();
                    header_lines.push(Line::from("Thinking".magenta().italic()));
                    append_markdown(&header_buffer, &mut header_lines, config);

                    let mut summary_lines: Vec<Line<'static>> = Vec::new();
                    summary_lines.push(Line::from("Thinking".magenta().bold()));
                    append_markdown(&summary_buffer, &mut summary_lines, config);

                    return vec![
                        Box::new(TranscriptOnlyHistoryCell {
                            lines: header_lines,
                        }),
                        Box::new(AgentMessageCell::new(summary_lines, true)),
                    ];
                }
            }
        }
    }
    vec![Box::new(new_reasoning_block(full_reasoning_buffer, config))]
}

fn output_lines(
    output: Option<&CommandOutput>,
    only_err: bool,
    include_angle_pipe: bool,
    include_prefix: bool,
) -> Vec<Line<'static>> {
    let CommandOutput {
        exit_code,
        stdout,
        stderr,
        ..
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
        let prefix = if !include_prefix {
            ""
        } else if i == 0 && include_angle_pipe {
            "  └ "
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
        out.push(format!("… +{omitted} lines").into());
    }

    let tail_start = if show_ellipsis {
        total - limit
    } else {
        head_end
    };
    for raw in lines[tail_start..].iter() {
        let mut line = ansi_escape_line(raw);
        if include_prefix {
            line.spans.insert(0, "    ".into());
        }
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
        invocation.server.clone().cyan(),
        ".".into(),
        invocation.tool.cyan(),
        "(".into(),
        args_str.dim(),
        ")".into(),
    ];
    invocation_spans.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    fn render_transcript(cell: &dyn HistoryCell) -> Vec<String> {
        render_lines(&cell.transcript_lines())
    }

    #[test]
    fn coalesces_sequential_reads_within_one_call() {
        // Build one exec cell with a Search followed by two Reads
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), "echo".into()],
            parsed: vec![
                ParsedCommand::Search {
                    query: Some("shimmer_spans".into()),
                    path: None,
                    cmd: "rg shimmer_spans".into(),
                },
                ParsedCommand::Read {
                    name: "shimmer.rs".into(),
                    cmd: "cat shimmer.rs".into(),
                },
                ParsedCommand::Read {
                    name: "status_indicator_widget.rs".into(),
                    cmd: "cat status_indicator_widget.rs".into(),
                },
            ],
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        // Mark call complete so markers are ✓
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );

        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn coalesces_reads_across_multiple_calls() {
        let mut cell = ExecCell::new(ExecCall {
            call_id: "c1".to_string(),
            command: vec!["bash".into(), "-lc".into(), "echo".into()],
            parsed: vec![ParsedCommand::Search {
                query: Some("shimmer_spans".into()),
                path: None,
                cmd: "rg shimmer_spans".into(),
            }],
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        // Call 1: Search only
        cell.complete_call(
            "c1",
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        // Call 2: Read A
        cell = cell
            .with_added_call(
                "c2".into(),
                vec!["bash".into(), "-lc".into(), "echo".into()],
                vec![ParsedCommand::Read {
                    name: "shimmer.rs".into(),
                    cmd: "cat shimmer.rs".into(),
                }],
            )
            .unwrap();
        cell.complete_call(
            "c2",
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        // Call 3: Read B
        cell = cell
            .with_added_call(
                "c3".into(),
                vec!["bash".into(), "-lc".into(), "echo".into()],
                vec![ParsedCommand::Read {
                    name: "status_indicator_widget.rs".into(),
                    cmd: "cat status_indicator_widget.rs".into(),
                }],
            )
            .unwrap();
        cell.complete_call(
            "c3",
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );

        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn coalesced_reads_dedupe_names() {
        let mut cell = ExecCell::new(ExecCall {
            call_id: "c1".to_string(),
            command: vec!["bash".into(), "-lc".into(), "echo".into()],
            parsed: vec![
                ParsedCommand::Read {
                    name: "auth.rs".into(),
                    cmd: "cat auth.rs".into(),
                },
                ParsedCommand::Read {
                    name: "auth.rs".into(),
                    cmd: "cat auth.rs".into(),
                },
                ParsedCommand::Read {
                    name: "shimmer.rs".into(),
                    cmd: "cat shimmer.rs".into(),
                },
            ],
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        cell.complete_call(
            "c1",
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_wraps_with_extra_indent_on_subsequent_lines() {
        // Create a completed exec cell with a multiline command
        let cmd = "set -o pipefail\ncargo test --all-features --quiet".to_string();
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), cmd],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        // Mark call complete so it renders as "Ran"
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );

        // Small width to force wrapping on both lines
        let width: u16 = 28;
        let lines = cell.display_lines(width);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn single_line_command_compact_when_fits() {
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["echo".into(), "ok".into()],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        // Wide enough that it fits inline
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn single_line_command_wraps_with_four_space_continuation() {
        let call_id = "c1".to_string();
        let long = "a_very_long_token_without_spaces_to_force_wrapping".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), long],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        let lines = cell.display_lines(24);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_without_wrap_uses_branch_then_eight_spaces() {
        let call_id = "c1".to_string();
        let cmd = "echo one\necho two".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), cmd],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_both_lines_wrap_with_correct_prefixes() {
        let call_id = "c1".to_string();
        let cmd = "first_token_is_long_enough_to_wrap\nsecond_token_is_also_long_enough_to_wrap"
            .to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), cmd],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );
        let lines = cell.display_lines(28);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn stderr_tail_more_than_five_lines_snapshot() {
        // Build an exec cell with a non-zero exit and 10 lines on stderr to exercise
        // the head/tail rendering and gutter prefixes.
        let call_id = "c_err".to_string();
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), "seq 1 10 1>&2 && false".into()],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });
        let stderr: String = (1..=10)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr,
                formatted_output: String::new(),
            },
            Duration::from_millis(1),
        );

        let rendered = cell
            .display_lines(80)
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn ran_cell_multiline_with_stderr_snapshot() {
        // Build an exec cell that completes (so it renders as "Ran") with a
        // command long enough that it must render on its own line under the
        // header, and include a couple of stderr lines to verify the output
        // block prefixes and wrapping.
        let call_id = "c_wrap_err".to_string();
        let long_cmd =
            "echo this_is_a_very_long_single_token_that_will_wrap_across_the_available_width";
        let mut cell = ExecCell::new(ExecCall {
            call_id: call_id.clone(),
            command: vec!["bash".into(), "-lc".into(), long_cmd.to_string()],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
        });

        let stderr = "error: first line on stderr\nerror: second line on stderr".to_string();
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr,
                formatted_output: String::new(),
            },
            Duration::from_millis(5),
        );

        // Narrow width to force the command to render under the header line.
        let width: u16 = 28;
        let rendered = cell
            .display_lines(width)
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(rendered);
    }
    #[test]
    fn user_history_cell_wraps_and_prefixes_each_line_snapshot() {
        let msg = "one two three four five six seven";
        let cell = UserHistoryCell {
            message: msg.to_string(),
        };

        // Small width to force wrapping more clearly. Effective wrap width is width-1 due to the ▌ prefix.
        let width: u16 = 12;
        let lines = cell.display_lines(width);
        let rendered = render_lines(&lines).join("\n");

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn plan_update_with_note_and_wrapping_snapshot() {
        // Long explanation forces wrapping; include long step text to verify step wrapping and alignment.
        let update = UpdatePlanArgs {
            explanation: Some(
                "I’ll update Grafana call error handling by adding retries and clearer messages when the backend is unreachable."
                    .to_string(),
            ),
            plan: vec![
                PlanItemArg {
                    step: "Investigate existing error paths and logging around HTTP timeouts".into(),
                    status: StepStatus::Completed,
                },
                PlanItemArg {
                    step: "Harden Grafana client error handling with retry/backoff and user‑friendly messages".into(),
                    status: StepStatus::InProgress,
                },
                PlanItemArg {
                    step: "Add tests for transient failure scenarios and surfacing to the UI".into(),
                    status: StepStatus::Pending,
                },
            ],
        };

        let cell = new_plan_update(update);
        // Narrow width to force wrapping for both the note and steps
        let lines = cell.display_lines(32);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn plan_update_without_note_snapshot() {
        let update = UpdatePlanArgs {
            explanation: None,
            plan: vec![
                PlanItemArg {
                    step: "Define error taxonomy".into(),
                    status: StepStatus::InProgress,
                },
                PlanItemArg {
                    step: "Implement mapping to user messages".into(),
                    status: StepStatus::Pending,
                },
            ],
        };

        let cell = new_plan_update(update);
        let lines = cell.display_lines(40);
        let rendered = render_lines(&lines).join("\n");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn reasoning_summary_block_returns_reasoning_cell_when_feature_disabled() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cells =
            new_reasoning_summary_block("Detailed reasoning goes here.".to_string(), &config);

        assert_eq!(cells.len(), 1);
        let rendered = render_transcript(cells[0].as_ref());
        assert_eq!(rendered, vec!["thinking", "Detailed reasoning goes here."]);
    }

    #[test]
    fn reasoning_summary_block_falls_back_when_header_is_missing() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cells = new_reasoning_summary_block(
            "**High level reasoning without closing".to_string(),
            &config,
        );

        assert_eq!(cells.len(), 1);
        let rendered = render_transcript(cells[0].as_ref());
        assert_eq!(
            rendered,
            vec!["thinking", "**High level reasoning without closing"]
        );
    }

    #[test]
    fn reasoning_summary_block_falls_back_when_summary_is_missing() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cells = new_reasoning_summary_block(
            "**High level reasoning without closing**".to_string(),
            &config,
        );

        assert_eq!(cells.len(), 1);
        let rendered = render_transcript(cells[0].as_ref());
        assert_eq!(
            rendered,
            vec!["thinking", "High level reasoning without closing"]
        );

        let cells = new_reasoning_summary_block(
            "**High level reasoning without closing**\n\n  ".to_string(),
            &config,
        );

        assert_eq!(cells.len(), 1);
        let rendered = render_transcript(cells[0].as_ref());
        assert_eq!(
            rendered,
            vec!["thinking", "High level reasoning without closing"]
        );
    }

    #[test]
    fn reasoning_summary_block_splits_header_and_summary_when_present() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cells = new_reasoning_summary_block(
            "**High level plan**\n\nWe should fix the bug next.".to_string(),
            &config,
        );

        assert_eq!(cells.len(), 2);

        let header_lines = render_transcript(cells[0].as_ref());
        assert_eq!(header_lines, vec!["Thinking", "High level plan"]);

        let summary_lines = render_transcript(cells[1].as_ref());

        assert_eq!(
            summary_lines,
            vec!["codex", "Thinking", "We should fix the bug next."]
        )
    }
}
