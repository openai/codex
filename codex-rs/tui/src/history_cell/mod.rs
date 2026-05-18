//! Transcript/history cells for the Codex TUI.
//!
//! A `HistoryCell` is the unit of display in the conversation UI, representing both committed
//! transcript entries and, transiently, an in-flight active cell that can mutate in place while
//! streaming.
//!
//! The transcript overlay (`Ctrl+T`) appends a cached live tail derived from the active cell, and
//! that cached tail is refreshed based on an active-cell cache key. Cells that change based on
//! elapsed time expose `transcript_animation_tick()`, and code that mutates the active cell in place
//! bumps the active-cell revision tracked by `ChatWidget`, so the cache key changes whenever the
//! rendered transcript output can change.

use crate::diff_model::FileChange;
use crate::diff_render::create_diff_summary;
use crate::diff_render::display_path_for;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::OutputLinesParams;
use crate::exec_cell::TOOL_CALL_MAX_LINES;
use crate::exec_cell::output_lines;
use crate::exec_command::relativize_to_home;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::legacy_core::config::Config;
use crate::live_wrap::take_prefix_by_width;
use crate::markdown::append_markdown;
use crate::markdown::append_markdown_agent_with_cwd;
use crate::motion::MotionMode;
use crate::motion::ReducedMotionIndicator;
use crate::motion::activity_indicator;
use crate::render::line_utils::line_to_static;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::render::renderable::Renderable;
use crate::session_state::ThreadSessionState;
use crate::style::proposed_plan_style;
use crate::style::user_message_style;
#[cfg(test)]
use crate::test_support::PathBufExt;
#[cfg(test)]
use crate::test_support::test_path_buf;
use crate::text_formatting::format_and_truncate_tool_result;
use crate::text_formatting::truncate_text;
use crate::tooltips;
use crate::ui_consts::LIVE_PREFIX_COLS;
use crate::update_action::UpdateAction;
use crate::version::CODEX_CLI_VERSION;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use crate::wrapping::adaptive_wrap_lines;
use base64::Engine;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_app_server_protocol::WebSearchAction;
use codex_config::types::McpServerTransportConfig;
#[cfg(test)]
use codex_mcp::qualified_mcp_tool_name_prefix;
use codex_otel::RuntimeMetricsSummary;
use codex_protocol::account::PlanType;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyAmendment;
#[cfg(test)]
use codex_protocol::mcp::Resource;
#[cfg(test)]
use codex_protocol::mcp::ResourceTemplate;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::local_image_label_text;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::user_input::TextElement;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::format_env_display;
use image::DynamicImage;
use image::ImageReader;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::any::Any;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tracing::error;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use url::Url;

const RAW_DIFF_SUMMARY_WIDTH: usize = 10_000;
const RAW_TOOL_OUTPUT_WIDTH: usize = 10_000;

mod approvals;
mod base;
mod exec;
mod hook_cell;
mod mcp;
mod messages;
mod notices;
mod patches;
mod plans;
mod request_user_input;
mod search;
mod separators;
mod session;

pub(crate) use approvals::*;
pub(crate) use base::*;
pub(crate) use exec::*;
pub(crate) use hook_cell::HookCell;
pub(crate) use hook_cell::new_active_hook_cell;
pub(crate) use hook_cell::new_completed_hook_cell;
pub(crate) use mcp::*;
pub(crate) use messages::*;
pub(crate) use notices::*;
pub(crate) use patches::*;
pub(crate) use plans::*;
pub(crate) use request_user_input::*;
pub(crate) use search::*;
pub(crate) use separators::*;
pub(crate) use session::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryRenderMode {
    Rich,
    Raw,
}

pub(crate) fn raw_lines_from_source(source: &str) -> Vec<Line<'static>> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut parts = source.split('\n').collect::<Vec<_>>();
    if source.ends_with('\n') {
        parts.pop();
    }

    parts
        .into_iter()
        .map(|line| Line::from(line.to_string()))
        .collect()
}

pub(crate) fn plain_lines(lines: impl IntoIterator<Item = Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let text = line
                .spans
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>();
            Line::from(text)
        })
        .collect()
}

/// A single renderable unit of conversation history.
///
/// Each cell produces logical `Line`s and reports how many viewport
/// rows those lines occupy at a given terminal width. The default
/// height implementations use `Paragraph::wrap` to account for lines
/// that overflow the viewport width (e.g. long URLs that are kept
/// intact by adaptive wrapping). Concrete types only need to override
/// heights when they apply additional layout logic beyond what
/// `Paragraph::line_count` captures.
pub(crate) trait HistoryCell: std::fmt::Debug + Send + Sync + Any {
    /// Returns the logical lines for the main chat viewport.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    /// Returns copy-friendly plain logical lines for raw scrollback mode.
    fn raw_lines(&self) -> Vec<Line<'static>>;

    fn display_lines_for_mode(&self, width: u16, mode: HistoryRenderMode) -> Vec<Line<'static>> {
        match mode {
            HistoryRenderMode::Rich => self.display_lines(width),
            HistoryRenderMode::Raw => self.raw_lines(),
        }
    }

    /// Returns the number of viewport rows needed to render this cell.
    ///
    /// The default delegates to `Paragraph::line_count` with
    /// `Wrap { trim: false }`, which measures the actual row count after
    /// ratatui's viewport-level character wrapping. This is critical
    /// for lines containing URL-like tokens that are wider than the
    /// terminal — the logical line count would undercount.
    fn desired_height(&self, width: u16) -> u16 {
        self.desired_height_for_mode(width, HistoryRenderMode::Rich)
    }

    fn desired_height_for_mode(&self, width: u16, mode: HistoryRenderMode) -> u16 {
        Paragraph::new(Text::from(self.display_lines_for_mode(width, mode)))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    /// Returns lines for the transcript overlay (`Ctrl+T`).
    ///
    /// Defaults to `display_lines`. Override when the transcript
    /// representation differs (e.g. `ExecCell` shows all calls with
    /// `$`-prefixed commands and exit status).
    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    /// Returns the number of viewport rows for the transcript overlay.
    ///
    /// Uses the same `Paragraph::line_count` measurement as
    /// `desired_height`. Contains a workaround for a ratatui bug where
    /// a single whitespace-only line reports 2 rows instead of 1.
    fn desired_transcript_height(&self, width: u16) -> u16 {
        let lines = self.transcript_lines(width);
        // Workaround: ratatui's line_count returns 2 for a single
        // whitespace-only line. Clamp to 1 in that case.
        if let [line] = &lines[..]
            && line
                .spans
                .iter()
                .all(|s| s.content.chars().all(char::is_whitespace))
        {
            return 1;
        }

        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    fn is_stream_continuation(&self) -> bool {
        false
    }

    /// Returns a coarse "animation tick" when transcript output is time-dependent.
    ///
    /// The transcript overlay caches the rendered output of the in-flight active cell, so cells
    /// that include time-based UI (spinner, shimmer, etc.) should return a tick that changes over
    /// time to signal that the cached tail should be recomputed. Returning `None` means the
    /// transcript lines are stable, while returning `Some(tick)` during an in-flight animation
    /// allows the overlay to keep up with the main viewport.
    ///
    /// If a cell uses time-based visuals but always returns `None`, `Ctrl+T` can appear "frozen" on
    /// the first rendered frame even though the main viewport is animating.
    fn transcript_animation_tick(&self) -> Option<u64> {
        None
    }
}

impl Renderable for Box<dyn HistoryCell> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.display_lines(area.width);
        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        let y = if area.height == 0 {
            0
        } else {
            let overflow = paragraph
                .line_count(area.width)
                .saturating_sub(usize::from(area.height));
            u16::try_from(overflow).unwrap_or(u16::MAX)
        };
        // Active-cell content can reflow dramatically during resize/stream updates. Clear the
        // entire draw area first so stale glyphs from previous frames never linger.
        Clear.render(area, buf);
        paragraph.scroll((y, 0)).render(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        HistoryCell::desired_height(self.as_ref(), width)
    }
}

impl dyn HistoryCell {
    pub(crate) fn as_any(&self) -> &dyn Any {
        self
    }

    pub(crate) fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
