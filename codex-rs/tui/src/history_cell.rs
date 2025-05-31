use crate::cell_widget::CellWidget;
use crate::exec_command::escape_command;
use crate::markdown::append_markdown;
use crate::text_block::TextBlock;
use base64::Engine;
use codex_ansi_escape::ansi_escape_line;
use codex_common::elapsed::format_duration;
use codex_core::config::Config;
use codex_core::protocol::FileChange;
use codex_core::protocol::SessionConfiguredEvent;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageReader;
use lazy_static::lazy_static;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use ratatui_image::Image as TuiImage;
use ratatui_image::Resize as ImgResize;
use ratatui_image::picker::ProtocolType;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
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
    ActiveExecCommand {
        call_id: String,
        /// The shell command, escaped and formatted.
        command: String,
        start: Instant,
        view: TextBlock,
    },

    /// Completed exec tool call.
    CompletedExecCommand { view: TextBlock },

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
        view: TextBlock,
    },

    /// Completed MCP tool call where we show the result serialized as JSON.
    CompletedMcpToolCallWithTextOutput { view: TextBlock },

    /// Completed MCP tool call where the result is an image.
    /// Admittedly, [mcp_types::CallToolResult] can have multiple content types,
    /// which could be a mix of text and images, so we need to tighten this up.
    // NOTE: For image output we keep the *original* image around and lazily
    // compute a resized copy that fits the available cell width.  Caching the
    // resized version avoids doing the potentially expensive rescale twice
    // because the scroll-view first calls `height()` for layouting and then
    // `render_window()` for painting.
    CompletedMcpToolCallWithImageOutput {
        image: DynamicImage,
        /// Cached data derived from the current terminal width.  The cache is
        /// invalidated whenever the width changes (e.g. when the user
        /// resizes the window).
        render_cache: std::cell::RefCell<Option<ImageRenderCache>>,
    },

    /// Background event.
    BackgroundEvent { view: TextBlock },

    /// Error event from the backend.
    ErrorEvent { view: TextBlock },

    /// Info describing the newly-initialized session.
    SessionInfo { view: TextBlock },

    /// A pending code patch that is awaiting user approval. Mirrors the
    /// behaviour of `ActiveExecCommand` so the user sees *what* patch the
    /// model wants to apply before being prompted to approve or deny it.
    PendingPatch { view: TextBlock },
}

const TOOL_CALL_MAX_LINES: usize = 5;

impl HistoryCell {
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
                    format!(" v{}", VERSION).into(),
                    " (research preview)".dim(),
                ]),
                Line::from(""),
                Line::from(vec![
                    "codex session".magenta().bold(),
                    " ".into(),
                    session_id.to_string().dim(),
                ]),
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
                Line::from(format!("used: {}", model)),
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
            view: TextBlock::new(lines),
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

        HistoryCell::CompletedExecCommand {
            view: TextBlock::new(lines),
        }
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
            view: TextBlock::new(lines),
        }
    }

    fn try_new_completed_mcp_tool_call_with_image_output(
        result: &Result<mcp_types::CallToolResult, String>,
    ) -> Option<Self> {
        match result {
            Ok(mcp_types::CallToolResult { content, .. }) => {
                if let Some(mcp_types::CallToolResultContent::ImageContent(image)) = content.first()
                {
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

                    Some(HistoryCell::CompletedMcpToolCallWithImageOutput {
                        image,
                        render_cache: std::cell::RefCell::new(None),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(crate) fn new_completed_mcp_tool_call(
        fq_tool_name: String,
        invocation: String,
        start: Instant,
        success: bool,
        result: Result<mcp_types::CallToolResult, String>,
    ) -> Self {
        if let Some(cell) = Self::try_new_completed_mcp_tool_call_with_image_output(&result) {
            return cell;
        }

        let duration = format_duration(start.elapsed());
        let status_str = if success { "success" } else { "failed" };
        let title_line = Line::from(vec![
            "tool".magenta(),
            format!(" {fq_tool_name} ({status_str}, duration: {})", duration).dim(),
        ]);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(title_line);
        lines.push(Line::from(format!("$ {invocation}")));

        // Convert result into serde_json::Value early so we don't have to
        // worry about lifetimes inside the match arm.
        let result_val = result.map(|r| {
            serde_json::to_value(r)
                .unwrap_or_else(|_| serde_json::Value::String("<serialization error>".into()))
        });

        if let Ok(res_val) = result_val {
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

        HistoryCell::CompletedMcpToolCallWithTextOutput {
            view: TextBlock::new(lines),
        }
    }

    pub(crate) fn new_background_event(message: String) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("event".dim()));
        lines.extend(message.lines().map(|l| Line::from(l.to_string()).dim()));
        lines.push(Line::from(""));
        HistoryCell::BackgroundEvent {
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
}

// ---------------------------------------------------------------------------
// `CellWidget` implementation – most variants delegate to their internal
// `TextBlock`.  Variants that need custom painting can add their own logic in
// the match arms.
// ---------------------------------------------------------------------------

impl CellWidget for HistoryCell {
    fn height(&self, width: u16) -> usize {
        match self {
            HistoryCell::WelcomeMessage { view }
            | HistoryCell::UserPrompt { view }
            | HistoryCell::AgentMessage { view }
            | HistoryCell::AgentReasoning { view }
            | HistoryCell::BackgroundEvent { view }
            | HistoryCell::ErrorEvent { view }
            | HistoryCell::SessionInfo { view }
            | HistoryCell::CompletedExecCommand { view }
            | HistoryCell::CompletedMcpToolCallWithTextOutput { view }
            | HistoryCell::PendingPatch { view }
            | HistoryCell::ActiveExecCommand { view, .. }
            | HistoryCell::ActiveMcpToolCall { view, .. } => view.height(width),
            HistoryCell::CompletedMcpToolCallWithImageOutput {
                image,
                render_cache,
            } => ensure_image_cache(image, width, render_cache),
        }
    }

    fn render_window(&self, first_visible_line: usize, area: Rect, buf: &mut Buffer) {
        match self {
            HistoryCell::WelcomeMessage { view }
            | HistoryCell::UserPrompt { view }
            | HistoryCell::AgentMessage { view }
            | HistoryCell::AgentReasoning { view }
            | HistoryCell::BackgroundEvent { view }
            | HistoryCell::ErrorEvent { view }
            | HistoryCell::SessionInfo { view }
            | HistoryCell::CompletedExecCommand { view }
            | HistoryCell::CompletedMcpToolCallWithTextOutput { view }
            | HistoryCell::PendingPatch { view }
            | HistoryCell::ActiveExecCommand { view, .. }
            | HistoryCell::ActiveMcpToolCall { view, .. } => {
                view.render_window(first_visible_line, area, buf)
            }
            HistoryCell::CompletedMcpToolCallWithImageOutput {
                image,
                render_cache,
            } => {
                // Ensure we have a cached, resized copy that matches the current width.
                // `height()` should have prepared the cache, but if something invalidated it
                // (e.g. the first `render_window()` call happens *before* `height()` after a
                // resize) we rebuild it here.

                let width_cells = area.width;

                // Ensure the cache is up-to-date and extract the scaled image.
                let _ = ensure_image_cache(image, width_cells, render_cache);

                let Some(resized) = render_cache
                    .borrow()
                    .as_ref()
                    .map(|c| c.scaled_image.clone())
                else {
                    return;
                };

                let picker = &*TERMINAL_PICKER;

                if let Ok(protocol) = picker.new_protocol(resized, area, ImgResize::Fit(None)) {
                    let img_widget = TuiImage::new(&protocol);
                    img_widget.render(area, buf);
                }
            }
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

// -------------------------------------
// Helper types for image rendering
// -------------------------------------

/// Cached information for rendering an image inside a conversation cell.
///
/// The cache ties the resized image to a *specific* content width (in
/// terminal cells).  Whenever the terminal is resized and the width changes
/// we need to re-compute the scaled variant so that it still fits the
/// available space.  Keeping the resized copy around saves a costly rescale
/// between the back-to-back `height()` and `render_window()` calls that the
/// scroll-view performs while laying out the UI.
pub(crate) struct ImageRenderCache {
    /// Width in *terminal cells* the cached image was generated for.
    width_cells: u16,
    /// Height in *terminal rows* that the conversation cell must occupy so
    /// the whole image becomes visible.
    height_rows: usize,
    /// The resized image that fits the given width / height constraints.
    scaled_image: DynamicImage,
}

lazy_static! {
    static ref TERMINAL_PICKER: ratatui_image::picker::Picker = {
        use ratatui_image::picker::Picker;
        use ratatui_image::picker::cap_parser::QueryStdioOptions;

        // Ask the terminal for capabilities and explicit font size.  Request the
        // Kitty *text-sizing protocol* as a fallback mechanism for terminals
        // (like iTerm2) that do not reply to the standard CSI 16/18 queries.
        match Picker::from_query_stdio_with_options(QueryStdioOptions {
            text_sizing_protocol: true,
        }) {
            Ok(picker) => picker,
            Err(err) => {
                // Fall back to the conservative default that assumes ~8×16 px cells.
                // Still better than breaking the build in a headless test run.
                tracing::warn!("terminal capability query failed: {err:?}; using default font size");
                Picker::from_fontsize((8, 16))
            }
        }
    };
}

/// Resize `image` to fit into `width_cells`×10-rows keeping the original aspect
/// ratio. The function updates `render_cache` and returns the number of rows
/// (<= 10) the picture will occupy.
fn ensure_image_cache(
    image: &DynamicImage,
    width_cells: u16,
    render_cache: &std::cell::RefCell<Option<ImageRenderCache>>,
) -> usize {
    if let Some(cache) = render_cache.borrow().as_ref() {
        if cache.width_cells == width_cells {
            return cache.height_rows;
        }
    }

    let picker = &*TERMINAL_PICKER;
    let (char_w_px, char_h_px) = picker.font_size();

    // Heuristic to compensate for Hi-DPI terminals (iTerm2 on Retina Mac) that
    // report logical pixels (≈ 8×16) while the iTerm2 graphics protocol
    // expects *device* pixels.  Empirically the device-pixel-ratio is almost
    // always 2 on macOS Retina panels.
    let hidpi_scale = if picker.protocol_type() == ProtocolType::Iterm2 {
        2.0f64
    } else {
        1.0
    };

    // The fallback Halfblocks protocol encodes two pixel rows per cell, so each
    // terminal *row* represents only half the (possibly scaled) font height.
    let effective_char_h_px: f64 = if picker.protocol_type() == ProtocolType::Halfblocks {
        (char_h_px as f64) * hidpi_scale / 2.0
    } else {
        (char_h_px as f64) * hidpi_scale
    };

    let char_w_px_f64 = (char_w_px as f64) * hidpi_scale;

    const MAX_ROWS: f64 = 10.0;
    let max_height_px: f64 = effective_char_h_px * MAX_ROWS;

    let (orig_w_px, orig_h_px) = {
        let (w, h) = image.dimensions();
        (w as f64, h as f64)
    };

    if orig_w_px == 0.0 || orig_h_px == 0.0 || width_cells == 0 {
        *render_cache.borrow_mut() = None;
        return 0;
    }

    let max_w_px = char_w_px_f64 * width_cells as f64;
    let scale_w = max_w_px / orig_w_px;
    let scale_h = max_height_px / orig_h_px;
    let scale = scale_w.min(scale_h).min(1.0);

    use image::imageops::FilterType;
    let scaled_w_px = (orig_w_px * scale).round().max(1.0) as u32;
    let scaled_h_px = (orig_h_px * scale).round().max(1.0) as u32;

    let scaled_image = image.resize(scaled_w_px, scaled_h_px, FilterType::Lanczos3);

    let height_rows = ((scaled_h_px as f64 / effective_char_h_px).ceil()) as usize;

    let new_cache = ImageRenderCache {
        width_cells,
        height_rows,
        scaled_image,
    };
    *render_cache.borrow_mut() = Some(new_cache);

    height_rows
}
