use crate::account_label::key_suffix;
use crate::diff_render::create_diff_summary_with_width;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::sanitize::Mode as SanitizeMode;
use crate::sanitize::Options as SanitizeOptions;
use crate::sanitize::sanitize_for_tui;
use crate::slash_command::SlashCommand;
use crate::util::buffer::{fill_rect, write_line};
use crate::insert_history::word_wrap_lines;
use crate::text_formatting::format_json_compact;
use crate::history::compat::{
    AssistantStreamState,
    BackgroundEventRecord,
    ExecAction,
    ExecRecord,
    ExecStatus,
    HistoryId,
    HistoryRecord,
    MergedExecRecord,
    RunningToolState,
    PatchEventType as HistoryPatchEventType,
    PatchRecord,
    ImageRecord,
    PlanIcon,
    PlanProgress,
    PlanStep,
    PlanUpdateState,
    PlainMessageState,
    ToolCallState,
    ToolStatus as HistoryToolStatus,
    UpgradeNoticeState,
};
use crate::history::compat::{ArgumentValue, ToolArgument, ToolResultPreview};
use base64::Engine;
use code_ansi_escape::ansi_escape_line;
use code_common::create_config_summary_entries;
use code_common::elapsed::format_duration;
use code_core::config::Config;
use code_core::config_types::ReasoningEffort;
use code_core::parse_command::ParsedCommand;
use code_core::plan_tool::PlanItemArg;
use code_core::plan_tool::StepStatus;
use code_core::plan_tool::UpdatePlanArgs;
use code_core::protocol::FileChange;
use code_core::protocol::McpInvocation;
use code_core::protocol::SessionConfiguredEvent;
use code_core::protocol::TokenUsage;
use code_protocol::num_format::format_with_separators;
use ::image::ImageReader;
use sha2::{Digest, Sha256};
use mcp_types::EmbeddedResourceResource;
use mcp_types::ResourceLink;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use shlex::Shlex;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use tracing::error;

mod assistant;
mod animated;
mod background;
mod card_style;
mod exec;
mod diff;
mod explore;
mod image;
mod loading;
mod reasoning;
mod tool;
mod browser;
mod agent;
mod auto_drive;
mod web_search;
mod wait_status;
mod plan_update;
mod rate_limits;
mod plain;
mod upgrade;
mod stream;
mod text;

pub(crate) use assistant::{
    assistant_markdown_lines,
    compute_assistant_layout,
    AssistantLayoutCache,
    AssistantMarkdownCell,
};
pub(crate) use animated::AnimatedWelcomeCell;
pub(crate) use background::BackgroundEventCell;
pub(crate) use exec::{
    display_lines_from_record as exec_display_lines_from_record,
    new_active_exec_command,
    new_completed_exec_command,
    ExecCell,
    ParsedExecMetadata,
};
pub(crate) use diff::{
    diff_lines_from_record,
    diff_record_from_string,
    new_diff_cell_from_string,
    DiffCell,
};
pub(crate) use browser::BrowserSessionCell;
pub(crate) use auto_drive::{AutoDriveActionKind, AutoDriveCardCell, AutoDriveStatus};
pub(crate) use web_search::{
    WebSearchSessionCell,
    WebSearchStatus,
};
pub(crate) use agent::{AgentDetail, AgentRunCell, AgentStatusKind, AgentStatusPreview, StepProgress};
pub(crate) use explore::{
    explore_lines_from_record,
    explore_lines_from_record_with_force,
    explore_lines_without_truncation,
    explore_record_push_from_parsed,
    explore_record_update_status,
    ExploreAggregationCell,
};
pub(crate) use rate_limits::RateLimitsCell;
pub(crate) use crate::history::state::ExploreEntryStatus;
pub(crate) use image::ImageOutputCell;
pub(crate) use loading::LoadingCell;
pub(crate) use reasoning::CollapsibleReasoningCell;
pub(crate) use tool::{RunningToolCallCell, ToolCallCell};
pub(crate) use wait_status::WaitStatusCell;
pub(crate) use plan_update::PlanUpdateCell;
pub(crate) use plain::{
    plain_message_state_from_lines,
    plain_message_state_from_paragraphs,
    plain_role_for_kind,
    PlainHistoryCell,
};
pub(crate) use stream::{stream_lines_from_state, StreamingContentCell};
pub(crate) use upgrade::UpgradeNoticeCell;

// ==================== Core Types ====================

#[derive(Clone)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

#[derive(Clone, Copy)]
pub(crate) enum PatchEventType {
    ApprovalRequest,
    ApplyBegin { auto_approved: bool },
    ApplySuccess,
    ApplyFailure,
}

// ==================== HistoryCellType ====================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryCellType {
    Plain,
    User,
    Assistant,
    Reasoning,
    Error,
    Exec { kind: ExecKind, status: ExecStatus },
    Tool { status: ToolCellStatus },
    Patch { kind: PatchKind },
    PlanUpdate,
    BackgroundEvent,
    Notice,
    Diff,
    Image,
    AnimatedWelcome,
    Loading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecKind {
    Read,
    Search,
    List,
    Run,
}

impl From<ExecAction> for ExecKind {
    fn from(action: ExecAction) -> Self {
        match action {
            ExecAction::Read => ExecKind::Read,
            ExecAction::Search => ExecKind::Search,
            ExecAction::List => ExecKind::List,
            ExecAction::Run => ExecKind::Run,
        }
    }
}

impl From<ExecKind> for ExecAction {
    fn from(kind: ExecKind) -> Self {
        match kind {
            ExecKind::Read => ExecAction::Read,
            ExecKind::Search => ExecAction::Search,
            ExecKind::List => ExecAction::List,
            ExecKind::Run => ExecAction::Run,
        }
    }
}


pub(crate) fn action_enum_from_parsed(
    parsed: &[code_core::parse_command::ParsedCommand],
) -> ExecAction {
    use code_core::parse_command::ParsedCommand;
    for p in parsed {
        match p {
            ParsedCommand::Read { .. } => return ExecAction::Read,
            ParsedCommand::Search { .. } => return ExecAction::Search,
            ParsedCommand::ListFiles { .. } => return ExecAction::List,
            _ => {}
        }
    }
    ExecAction::Run
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolCellStatus {
    Running,
    Success,
    Failed,
}

impl From<HistoryToolStatus> for ToolCellStatus {
    fn from(status: HistoryToolStatus) -> Self {
        match status {
            HistoryToolStatus::Running => ToolCellStatus::Running,
            HistoryToolStatus::Success => ToolCellStatus::Success,
            HistoryToolStatus::Failed => ToolCellStatus::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatchKind {
    Proposed,
    ApplyBegin,
    ApplySuccess,
    ApplyFailure,
}

// ==================== HistoryCell Trait ====================

/// Represents an event to display in the conversation history.
/// Returns its `Vec<Line<'static>>` representation to make it easier
/// to display in a scrollable list.
pub(crate) trait HistoryCell {
    fn display_lines(&self) -> Vec<Line<'static>>;
    /// A required, explicit type descriptor for the history cell.
    fn kind(&self) -> HistoryCellType;

    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn std::any::Any {
        // Default implementation that doesn't support downcasting
        // Concrete types that need downcasting should override this
        &() as &dyn std::any::Any
    }
    /// Allow mutable downcasting to concrete types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Get display lines with empty lines trimmed from beginning and end.
    /// This ensures consistent spacing when cells are rendered together.
    fn display_lines_trimmed(&self) -> Vec<Line<'static>> {
        trim_empty_lines(self.display_lines())
    }

    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(self.display_lines_trimmed()))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    fn render_with_skip(&self, area: Rect, buf: &mut Buffer, skip_rows: u16) {
        // Check if this cell has custom rendering
        if self.has_custom_render() {
            // Allow custom renders to handle top skipping explicitly
            self.custom_render_with_skip(area, buf, skip_rows);
            return;
        }

        // Default path: render the full text and use Paragraph.scroll to skip
        // vertical rows AFTER wrapping. Slicing lines before wrapping causes
        // incorrect blank space when lines wrap across multiple rows.
        // IMPORTANT: Explicitly clear the entire area first. While some containers
        // clear broader regions, custom widgets that shrink or scroll can otherwise
        // leave residual glyphs to the right of shorter lines or from prior frames.
        // We paint spaces with the current theme background to guarantee a clean slate.
        // Assistant messages use a subtly tinted background: theme background
        // moved 5% toward the theme info color for a gentle distinction.
        let cell_bg = match self.kind() {
            HistoryCellType::Assistant => crate::colors::assistant_bg(),
            _ => crate::colors::background(),
        };
        let bg_style = Style::default().bg(cell_bg).fg(crate::colors::text());
        if matches!(self.kind(), HistoryCellType::Assistant) {
            fill_rect(buf, area, Some(' '), bg_style);
        }

        // Ensure the entire allocated area is painted with the theme background
        // by attaching a background-styled Block to the Paragraph as well.
        let lines = self.display_lines_trimmed();
        let text = Text::from(lines);

        let bg_block = Block::default().style(Style::default().bg(cell_bg));
        Paragraph::new(text)
            .block(bg_block)
            .wrap(Wrap { trim: false })
            .scroll((skip_rows, 0))
            .style(Style::default().bg(cell_bg))
            .render(area, buf);
    }

    /// Returns true if this cell has custom rendering (e.g., animations)
    fn has_custom_render(&self) -> bool {
        false // Default: most cells use display_lines
    }

    /// Custom render implementation for cells that need it
    fn custom_render(&self, _area: Rect, _buf: &mut Buffer) {
        // Default: do nothing (cells with custom rendering will override)
    }
    /// Custom render with support for skipping top rows
    fn custom_render_with_skip(&self, area: Rect, buf: &mut Buffer, _skip_rows: u16) {
        // Default: fall back to non-skipping custom render
        self.custom_render(area, buf);
    }

    /// Returns true if this cell is currently animating and needs redraws
    fn is_animating(&self) -> bool {
        false // Default: most cells don't animate
    }

    /// Returns true if this is a loading cell that should be removed when streaming starts
    #[allow(dead_code)]
    fn is_loading_cell(&self) -> bool {
        false // Default: most cells are not loading cells
    }

    /// Trigger fade-out animation (for AnimatedWelcomeCell)
    fn trigger_fade(&self) {
        // Default: do nothing (only AnimatedWelcomeCell implements this)
    }

    /// Check if this cell should be removed (e.g., fully faded out)
    fn should_remove(&self) -> bool {
        false // Default: most cells should not be removed
    }

    /// Returns the gutter symbol for this cell type
    /// Returns None if no symbol should be displayed
    fn gutter_symbol(&self) -> Option<&'static str> {
        match self.kind() {
            HistoryCellType::Plain => None,
            HistoryCellType::User => Some("›"),
            // Restore assistant gutter icon
            HistoryCellType::Assistant => Some("•"),
            HistoryCellType::Reasoning => None,
            HistoryCellType::Error => Some("✖"),
            HistoryCellType::Tool { status } => Some(match status {
                ToolCellStatus::Running => "⚙",
                ToolCellStatus::Success => "✔",
                ToolCellStatus::Failed => "✖",
            }),
            HistoryCellType::Exec { kind, status } => {
                // Show ❯ only for Run executions; hide for read/search/list summaries
                match (kind, status) {
                    (ExecKind::Run, ExecStatus::Error) => Some("✖"),
                    (ExecKind::Run, _) => Some("❯"),
                    _ => None,
                }
            }
            HistoryCellType::Patch { .. } => Some("↯"),
            // Plan updates supply their own gutter glyph dynamically.
            HistoryCellType::PlanUpdate => None,
            HistoryCellType::BackgroundEvent => Some("»"),
            HistoryCellType::Notice => Some("★"),
            HistoryCellType::Diff => Some("↯"),
            HistoryCellType::Image => None,
            HistoryCellType::AnimatedWelcome => None,
            HistoryCellType::Loading => None,
        }
    }
}

// Allow Box<dyn HistoryCell> to implement HistoryCell
impl HistoryCell for Box<dyn HistoryCell> {
    fn as_any(&self) -> &dyn std::any::Any {
        self.as_ref().as_any()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self.as_mut().as_any_mut()
    }
    fn kind(&self) -> HistoryCellType {
        self.as_ref().kind()
    }

    fn display_lines(&self) -> Vec<Line<'static>> {
        self.as_ref().display_lines()
    }

    fn display_lines_trimmed(&self) -> Vec<Line<'static>> {
        self.as_ref().display_lines_trimmed()
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_ref().desired_height(width)
    }

    fn render_with_skip(&self, area: Rect, buf: &mut Buffer, skip_rows: u16) {
        self.as_ref().render_with_skip(area, buf, skip_rows)
    }

    fn has_custom_render(&self) -> bool {
        self.as_ref().has_custom_render()
    }

    fn custom_render(&self, area: Rect, buf: &mut Buffer) {
        self.as_ref().custom_render(area, buf)
    }

    fn is_animating(&self) -> bool {
        self.as_ref().is_animating()
    }

    fn is_loading_cell(&self) -> bool {
        self.as_ref().is_loading_cell()
    }

    fn trigger_fade(&self) {
        self.as_ref().trigger_fade()
    }

    fn should_remove(&self) -> bool {
        self.as_ref().should_remove()
    }

    fn gutter_symbol(&self) -> Option<&'static str> {
        self.as_ref().gutter_symbol()
    }
}

// ==================== ExploreAggregationCell ====================
// Collapses consecutive Read/Search/List commands into a single "Exploring" cell
// while commands are executing, updating the entry status once the command finishes.

pub(crate) fn clean_wait_command(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some((first_token, rest)) = split_token(trimmed) else {
        return trimmed.to_string();
    };
    if !looks_like_shell(first_token) {
        return trimmed.to_string();
    }
    let rest = rest.trim_start();
    let Some((second_token, remainder)) = split_token(rest) else {
        return trimmed.to_string();
    };
    if second_token != "-lc" {
        return trimmed.to_string();
    }
    let mut command = remainder.trim_start();
    if command.len() >= 2 {
        let bytes = command.as_bytes();
        let first_char = bytes[0] as char;
        let last_char = bytes[bytes.len().saturating_sub(1)] as char;
        if (first_char == '"' && last_char == '"') || (first_char == '\'' && last_char == '\'') {
            command = &command[1..command.len().saturating_sub(1)];
        }
    }
    if command.is_empty() {
        trimmed.to_string()
    } else {
        command.to_string()
    }
}

fn split_token(input: &str) -> Option<(&str, &str)> {
    let s = input.trim_start();
    if s.is_empty() {
        return None;
    }
    if let Some(idx) = s.find(char::is_whitespace) {
        let (token, rest) = s.split_at(idx);
        Some((token, rest))
    } else {
        Some((s, ""))
    }
}

fn looks_like_shell(token: &str) -> bool {
    let trimmed = token.trim_matches('"').trim_matches('\'');
    let basename = trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    matches!(
        basename.as_str(),
        "bash"
            | "bash.exe"
            | "sh"
            | "sh.exe"
            | "zsh"
            | "zsh.exe"
            | "dash"
            | "dash.exe"
            | "ksh"
            | "ksh.exe"
            | "busybox"
    )
}

// Remove formatting-only pipes (sed/head/tail) when we already provide a line-range
// annotation alongside the command summary. Keeps the core command intact for display.
// ==================== MergedExecCell ====================
// Represents multiple completed exec results merged into one cell while preserving
// the bordered, dimmed output styling for each command's stdout/stderr preview.

struct MergedExecSegment {
    record: ExecRecord,
}

impl MergedExecSegment {
    fn new(record: ExecRecord) -> Self {
        Self { record }
    }

    fn exec_parts(&self) -> (Vec<Line<'static>>, Vec<Line<'static>>, Option<Line<'static>>) {
        let exec_cell = ExecCell::from_record(self.record.clone());
        exec_cell.exec_render_parts()
    }

    fn lines(&self) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
        let (pre, mut out, status_line) = self.exec_parts();
        if let Some(status) = status_line {
            out.push(status);
        }
        (pre, out)
    }
}

pub(crate) struct MergedExecCell {
    segments: Vec<MergedExecSegment>,
    kind: ExecKind,
    history_id: HistoryId,
}

impl MergedExecCell {
    pub(crate) fn rebuild_with_theme(&self) {}

    pub(crate) fn set_history_id(&mut self, id: HistoryId) {
        self.history_id = id;
    }

    pub(crate) fn to_record(&self) -> MergedExecRecord {
        MergedExecRecord {
            id: self.history_id,
            action: self.kind.into(),
            segments: self
                .segments
                .iter()
                .map(|segment| segment.record.clone())
                .collect(),
        }
    }

    pub(crate) fn from_records(
        history_id: HistoryId,
        action: ExecAction,
        segments: Vec<ExecRecord>,
    ) -> Self {
        Self {
            segments: segments.into_iter().map(MergedExecSegment::new).collect(),
            kind: action.into(),
            history_id,
        }
    }

    pub(crate) fn from_state(record: MergedExecRecord) -> Self {
        let history_id = record.id;
        let kind: ExecKind = record.action.into();
        let segments = record
            .segments
            .into_iter()
            .map(MergedExecSegment::new)
            .collect();
        Self {
            segments,
            kind,
            history_id,
        }
    }

    fn aggregated_read_preamble_lines(&self) -> Option<Vec<Line<'static>>> {
        if self.kind != ExecKind::Read {
            return None;
        }
        use ratatui::text::Span;

        fn parse_read_line(line: &Line<'_>) -> Option<(String, u32, u32)> {
            if line.spans.is_empty() {
                return None;
            }
            let first = line.spans[0].content.as_ref();
            if !(first == "└ " || first == "  ") {
                return None;
            }
            let rest: String = line
                .spans
                .iter()
                .skip(1)
                .map(|s| s.content.as_ref())
                .collect();
            if let Some(idx) = rest.rfind(" (lines ") {
                let fname = rest[..idx].to_string();
                let tail = &rest[idx + 1..];
                if tail.starts_with("(lines ") && tail.ends_with(")") {
                    let inner = &tail[7..tail.len().saturating_sub(1)];
                    if let Some((a, b)) = inner.split_once(" to ") {
                        if let (Ok(s), Ok(e)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                            return Some((fname, s, e));
                        }
                    }
                }
            }
            None
        }

        fn is_search_like(line: &Line<'_>) -> bool {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let t = text.trim();
            t.contains(" (in ")
                || t.rsplit_once(" in ")
                    .map(|(_, rhs)| rhs.trim_end().ends_with('/'))
                    .unwrap_or(false)
        }

        let mut kept: Vec<Line<'static>> = Vec::new();
        for (seg_idx, segment) in self.segments.iter().enumerate() {
            let (pre_raw, _, _) = segment.exec_parts();
            let mut pre = trim_empty_lines(pre_raw);
            if !pre.is_empty() {
                pre.remove(0);
            }
            for line in pre.into_iter() {
                if is_search_like(&line) {
                    continue;
                }
                let keep = parse_read_line(&line).is_some() || seg_idx == 0;
                if keep {
                    kept.push(line);
                }
            }
        }

        if kept.is_empty() {
            return Some(kept);
        }

        if let Some(first) = kept.first_mut() {
            let flat: String = first.spans.iter().map(|s| s.content.as_ref()).collect();
            let has_connector = flat.trim_start().starts_with("└ ");
            if !has_connector {
                first.spans.insert(
                    0,
                    Span::styled("└ ", Style::default().fg(crate::colors::text_dim())),
                );
            }
        }
        for line in kept.iter_mut().skip(1) {
            if let Some(span0) = line.spans.get_mut(0) {
                if span0.content.as_ref() == "└ " {
                    span0.content = "  ".into();
                    span0.style = span0.style.add_modifier(Modifier::DIM);
                }
            }
        }

        coalesce_read_ranges_in_lines_local(&mut kept);
        Some(kept)
    }
}


impl HistoryCell for MergedExecCell {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn kind(&self) -> HistoryCellType {
        HistoryCellType::Exec {
            kind: self.kind,
            status: ExecStatus::Success,
        }
    }
    fn desired_height(&self, width: u16) -> u16 {
        let header_rows = if self.kind == ExecKind::Run { 0 } else { 1 };
        let pre_wrap_width = width;
        let out_wrap_width = width.saturating_sub(2);
        let mut total: u16 = header_rows;

        if let Some(agg_pre) = self.aggregated_read_preamble_lines() {
            let pre_rows: u16 = Paragraph::new(Text::from(agg_pre))
                .wrap(Wrap { trim: false })
                .line_count(pre_wrap_width)
                .try_into()
                .unwrap_or(0);
            total = total.saturating_add(pre_rows);
            for segment in &self.segments {
                let (_, out_raw) = segment.lines();
                let out = trim_empty_lines(out_raw);
                let out_rows: u16 = Paragraph::new(Text::from(out))
                    .wrap(Wrap { trim: false })
                    .line_count(out_wrap_width)
                    .try_into()
                    .unwrap_or(0);
                total = total.saturating_add(out_rows);
            }
            return total;
        }

        let mut added_corner = false;
        for segment in &self.segments {
            let (pre_raw, out_raw) = segment.lines();
            let mut pre = trim_empty_lines(pre_raw);
            if self.kind != ExecKind::Run && !pre.is_empty() {
                pre.remove(0);
            }
            if self.kind != ExecKind::Run {
                if let Some(first) = pre.first_mut() {
                    let flat: String = first.spans.iter().map(|s| s.content.as_ref()).collect();
                    let has_corner = flat.trim_start().starts_with("└ ");
                    let has_spaced_corner = flat.trim_start().starts_with("  └ ");
                    if !added_corner {
                        if !(has_corner || has_spaced_corner) {
                            first.spans.insert(
                                0,
                                Span::styled("└ ", Style::default().fg(crate::colors::text_dim())),
                            );
                        }
                        added_corner = true;
                    } else if let Some(sp0) = first.spans.get_mut(0) {
                        if sp0.content.as_ref() == "└ " {
                            sp0.content = "  ".into();
                            sp0.style = sp0.style.add_modifier(Modifier::DIM);
                        }
                    }
                }
            }
            let out = trim_empty_lines(out_raw);
            let pre_rows: u16 = Paragraph::new(Text::from(pre))
                .wrap(Wrap { trim: false })
                .line_count(pre_wrap_width)
                .try_into()
                .unwrap_or(0);
            let out_rows: u16 = Paragraph::new(Text::from(out))
                .wrap(Wrap { trim: false })
                .line_count(out_wrap_width)
                .try_into()
                .unwrap_or(0);
            total = total.saturating_add(pre_rows).saturating_add(out_rows);
        }

        total
    }
    fn display_lines(&self) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        for (i, segment) in self.segments.iter().enumerate() {
            let (pre_raw, out_raw) = segment.lines();
            if i > 0 {
                out.push(Line::from(""));
            }
            out.extend(trim_empty_lines(pre_raw));
            out.extend(trim_empty_lines(out_raw));
        }
        out
    }
    fn has_custom_render(&self) -> bool {
        true
    }
    fn custom_render_with_skip(&self, area: Rect, buf: &mut Buffer, mut skip_rows: u16) {
        let bg = Style::default()
            .bg(crate::colors::background())
            .fg(crate::colors::text());
        fill_rect(buf, area, Some(' '), bg);

        // Build one header line based on exec kind
        let header_line = match self.kind {
            ExecKind::Read => Some(Line::styled(
                "Read",
                Style::default().fg(crate::colors::text()),
            )),
            ExecKind::Search => Some(Line::styled(
                "Search",
                Style::default().fg(crate::colors::text_dim()),
            )),
            ExecKind::List => Some(Line::styled(
                "List",
                Style::default().fg(crate::colors::text()),
            )),
            ExecKind::Run => None,
        };

        let mut cur_y = area.y;
        let end_y = area.y.saturating_add(area.height);

        // Render or skip header line
        if let Some(header_line) = header_line {
            if skip_rows == 0 {
                if cur_y < end_y {
                    let txt = Text::from(vec![header_line]);
                    Paragraph::new(txt)
                        .block(Block::default().style(bg))
                        .wrap(Wrap { trim: false })
                        .render(
                            Rect {
                                x: area.x,
                                y: cur_y,
                                width: area.width,
                                height: 1,
                            },
                            buf,
                        );
                    cur_y = cur_y.saturating_add(1);
                }
            } else {
                skip_rows = skip_rows.saturating_sub(1);
            }
        }

        let mut added_corner: bool = false;
        let mut ensure_prefix = |lines: &mut Vec<Line<'static>>| {
            if self.kind == ExecKind::Run {
                return;
            }
            if let Some(first) = lines.first_mut() {
                let flat: String = first.spans.iter().map(|s| s.content.as_ref()).collect();
                let has_corner = flat.trim_start().starts_with("└ ");
                let has_spaced_corner = flat.trim_start().starts_with("  └ ");
                if !added_corner {
                    if !(has_corner || has_spaced_corner) {
                        first.spans.insert(
                            0,
                            Span::styled("└ ", Style::default().fg(crate::colors::text_dim())),
                        );
                    }
                    added_corner = true;
                } else {
                    // For subsequent segments, replace any leading corner with two spaces
                    if let Some(sp0) = first.spans.get_mut(0) {
                        if sp0.content.as_ref() == "└ " {
                            sp0.content = "  ".into();
                            sp0.style = sp0.style.add_modifier(Modifier::DIM);
                        }
                    }
                }
            }
        };

        // Special aggregated rendering for Read: collapse file ranges
        if self.kind == ExecKind::Read {
            if let Some(agg_pre) = self.aggregated_read_preamble_lines() {
                let pre_text = Text::from(agg_pre);
                let pre_wrap_width = area.width;
                let pre_total: u16 = Paragraph::new(pre_text.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(pre_wrap_width)
                    .try_into()
                    .unwrap_or(0);
                if cur_y < end_y {
                    let pre_skip = skip_rows.min(pre_total);
                    let pre_remaining = pre_total.saturating_sub(pre_skip);
                    let pre_height = pre_remaining.min(end_y.saturating_sub(cur_y));
                    if pre_height > 0 {
                        Paragraph::new(pre_text)
                            .block(Block::default().style(bg))
                            .wrap(Wrap { trim: false })
                            .scroll((pre_skip, 0))
                            .style(bg)
                            .render(
                                Rect {
                                    x: area.x,
                                    y: cur_y,
                                    width: area.width,
                                    height: pre_height,
                                },
                                buf,
                            );
                        cur_y = cur_y.saturating_add(pre_height);
                    }
                    skip_rows = skip_rows.saturating_sub(pre_skip);
                }

                let out_wrap_width = area.width.saturating_sub(2);
                for segment in &self.segments {
                    if cur_y >= end_y {
                        break;
                    }
                    let (_, out_raw) = segment.lines();
                    let out = trim_empty_lines(out_raw);
                    let out_text = Text::from(out.clone());
                    let out_total: u16 = Paragraph::new(out_text.clone())
                        .wrap(Wrap { trim: false })
                        .line_count(out_wrap_width)
                        .try_into()
                        .unwrap_or(0);
                    let out_skip = skip_rows.min(out_total);
                    let out_remaining = out_total.saturating_sub(out_skip);
                    let out_height = out_remaining.min(end_y.saturating_sub(cur_y));
                    if out_height > 0 {
                        let out_area = Rect {
                            x: area.x,
                            y: cur_y,
                            width: area.width,
                            height: out_height,
                        };
                        let block = Block::default()
                            .borders(Borders::LEFT)
                            .border_style(
                                Style::default()
                                    .fg(crate::colors::border_dim())
                                    .bg(crate::colors::background()),
                            )
                            .style(Style::default().bg(crate::colors::background()))
                            .padding(Padding {
                                left: 1,
                                right: 0,
                                top: 0,
                                bottom: 0,
                            });
                        Paragraph::new(out_text)
                            .block(block)
                            .wrap(Wrap { trim: false })
                            .scroll((out_skip, 0))
                            .style(
                                Style::default()
                                    .bg(crate::colors::background())
                                    .fg(crate::colors::text_dim()),
                            )
                            .render(out_area, buf);
                        cur_y = cur_y.saturating_add(out_height);
                    }
                    skip_rows = skip_rows.saturating_sub(out_skip);
                }
                return;
            }

            // Fallback: each segment retains its own preamble and output
        }

        for segment in &self.segments {
            if cur_y >= end_y {
                break;
            }
            let (pre_raw, out_raw) = segment.lines();
            let mut pre = trim_empty_lines(pre_raw);
            if self.kind != ExecKind::Run && !pre.is_empty() {
                pre.remove(0);
            }
            ensure_prefix(&mut pre);

            let out = trim_empty_lines(out_raw);
            let out_text = Text::from(out.clone());

            let pre_text = Text::from(pre.clone());
            let pre_wrap_width = area.width;
            let out_wrap_width = area.width.saturating_sub(2);
            let pre_total: u16 = Paragraph::new(pre_text.clone())
                .wrap(Wrap { trim: false })
                .line_count(pre_wrap_width)
                .try_into()
                .unwrap_or(0);
            let out_total: u16 = Paragraph::new(out_text.clone())
                .wrap(Wrap { trim: false })
                .line_count(out_wrap_width)
                .try_into()
                .unwrap_or(0);

            // Apply skip to pre, then out
            let pre_skip = skip_rows.min(pre_total);
            let out_skip = skip_rows.saturating_sub(pre_total).min(out_total);

            // Render pre
            let pre_remaining = pre_total.saturating_sub(pre_skip);
            let pre_height = pre_remaining.min(end_y.saturating_sub(cur_y));
            if pre_height > 0 {
                Paragraph::new(pre_text)
                    .block(Block::default().style(bg))
                    .wrap(Wrap { trim: false })
                    .scroll((pre_skip, 0))
                    .style(bg)
                    .render(
                        Rect {
                            x: area.x,
                            y: cur_y,
                            width: area.width,
                            height: pre_height,
                        },
                        buf,
                    );
                cur_y = cur_y.saturating_add(pre_height);
            }

            if cur_y >= end_y {
                break;
            }
            // Render out as bordered, dim block
            let out_remaining = out_total.saturating_sub(out_skip);
            let out_height = out_remaining.min(end_y.saturating_sub(cur_y));
            if out_height > 0 {
                let out_area = Rect {
                    x: area.x,
                    y: cur_y,
                    width: area.width,
                    height: out_height,
                };
                let block = Block::default()
                    .borders(Borders::LEFT)
                    .border_style(
                        Style::default()
                            .fg(crate::colors::border_dim())
                            .bg(crate::colors::background()),
                    )
                    .style(Style::default().bg(crate::colors::background()))
                    .padding(Padding {
                        left: 1,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    });
                Paragraph::new(out_text)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .scroll((out_skip, 0))
                    .style(
                        Style::default()
                            .bg(crate::colors::background())
                            .fg(crate::colors::text_dim()),
                    )
                    .render(out_area, buf);
                cur_y = cur_y.saturating_add(out_height);
            }

            // Consume skip rows used in this segment
            let consumed = pre_total + out_total;
            skip_rows = skip_rows.saturating_sub(consumed);
        }
    }
}

fn exec_render_parts_parsed_with_meta(
    parsed_commands: &[ParsedCommand],
    meta: &ParsedExecMetadata,
    output: Option<&CommandOutput>,
    stream_preview: Option<&CommandOutput>,
    elapsed_since_start: Option<Duration>,
    status_label: &str,
) -> (
    Vec<Line<'static>>,
    Vec<Line<'static>>,
    Option<Line<'static>>,
) {
    let action = meta.action;
    let ctx_path = meta.ctx_path.as_deref();
    let suppress_run_header = matches!(action, ExecAction::Run) && output.is_some();
    let mut pre: Vec<Line<'static>> = Vec::new();
    let mut running_status: Option<Line<'static>> = None;
    if !suppress_run_header {
        match output {
            None => match action {
                ExecAction::Read => pre.push(Line::styled(
                    "Read",
                    Style::default().fg(crate::colors::text()),
                )),
                ExecAction::Search => pre.push(Line::styled(
                    "Search",
                    Style::default().fg(crate::colors::text_dim()),
                )),
                ExecAction::List => pre.push(Line::styled(
                    "List",
                    Style::default().fg(crate::colors::text()),
                )),
                ExecAction::Run => {
                    let mut message = match &ctx_path {
                        Some(p) => format!("{}... in {p}", status_label),
                        None => format!("{}...", status_label),
                    };
                    if let Some(elapsed) = elapsed_since_start {
                        message = format!("{message} ({})", format_duration(elapsed));
                    }
                    running_status = Some(running_status_line(message));
                }
            },
            Some(o) if o.exit_code == 0 => {
                let done = match action {
                    ExecAction::Read => "Read".to_string(),
                    ExecAction::Search => "Search".to_string(),
                    ExecAction::List => "List".to_string(),
                    ExecAction::Run => match &ctx_path {
                        Some(p) => format!("Ran in {}", p),
                        None => "Ran".to_string(),
                    },
                };
                if matches!(
                    action,
                    ExecAction::Read | ExecAction::Search | ExecAction::List
                ) {
                    pre.push(Line::styled(
                        done,
                        Style::default().fg(crate::colors::text_dim()),
                    ));
                } else {
                    pre.push(Line::styled(
                        done,
                        Style::default()
                            .fg(crate::colors::text_bright())
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
            Some(_) => {
                let done = match action {
                    ExecAction::Read => "Read".to_string(),
                    ExecAction::Search => "Search".to_string(),
                    ExecAction::List => "List".to_string(),
                    ExecAction::Run => match &ctx_path {
                        Some(p) => format!("Ran in {}", p),
                        None => "Ran".to_string(),
                    },
                };
                if matches!(
                    action,
                    ExecAction::Read | ExecAction::Search | ExecAction::List
                ) {
                    pre.push(Line::styled(
                        done,
                        Style::default().fg(crate::colors::text_dim()),
                    ));
                } else {
                    pre.push(Line::styled(
                        done,
                        Style::default()
                            .fg(crate::colors::text_bright())
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }
    }

    // Reuse the same parsed-content rendering as new_parsed_command
    let search_paths = &meta.search_paths;
    // Compute output preview first to know whether to draw the downward corner.
    let show_stdout = matches!(action, ExecAction::Run);
    let display_output = output.or(stream_preview);
    let mut out = output_lines(display_output, !show_stdout, false);
    let mut any_content_emitted = false;
    // Determine allowed label(s) for this cell's primary action
    let expected_label: Option<&'static str> = match action {
        ExecAction::Read => Some("Read"),
        ExecAction::Search => Some("Search"),
        ExecAction::List => Some("List"),
        ExecAction::Run => None, // run: allow a set of labels
    };
    let use_content_connectors = !(matches!(action, ExecAction::Run) && output.is_none());

    for parsed in parsed_commands.iter() {
        let (label, content) = match parsed {
            ParsedCommand::Read { name, cmd, .. } => {
                let mut c = name.clone();
                if let Some(ann) = parse_read_line_annotation(cmd) {
                    c = format!("{} {}", c, ann);
                }
                ("Read".to_string(), c)
            }
            ParsedCommand::ListFiles { cmd: _, path } => match path {
                Some(p) => {
                    if search_paths.contains(p) {
                        (String::new(), String::new())
                    } else {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{}/", p)
                        };
                        ("List".to_string(), format!("{}", display_p))
                    }
                }
                None => ("List".to_string(), "./".to_string()),
            },
            ParsedCommand::Search { query, path, cmd } => {
                // Make search terms human-readable:
                // - Unescape any backslash-escaped character (e.g., "\?" -> "?")
                // - Close unbalanced pairs for '(' and '{' to avoid dangling text in UI
                let prettify_term = |s: &str| -> String {
                    // General unescape: remove backslashes that escape the next char
                    let mut out = String::with_capacity(s.len());
                    let mut iter = s.chars();
                    while let Some(ch) = iter.next() {
                        if ch == '\\' {
                            if let Some(next) = iter.next() {
                                out.push(next);
                            } else {
                                out.push('\\');
                            }
                        } else {
                            out.push(ch);
                        }
                    }

                    // Balance parentheses
                    let opens_paren = out.matches("(").count();
                    let closes_paren = out.matches(")").count();
                    for _ in 0..opens_paren.saturating_sub(closes_paren) {
                        out.push(')');
                    }

                    // Balance curly braces
                    let opens_curly = out.matches("{").count();
                    let closes_curly = out.matches("}").count();
                    for _ in 0..opens_curly.saturating_sub(closes_curly) {
                        out.push('}');
                    }

                    out
                };
                let fmt_query = |q: &str| -> String {
                    let mut parts: Vec<String> = q
                        .split('|')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(prettify_term)
                        .collect();
                    match parts.len() {
                        0 => String::new(),
                        1 => parts.remove(0),
                        2 => format!("{} and {}", parts[0], parts[1]),
                        _ => {
                            let last = parts.last().cloned().unwrap_or_default();
                            let head = &parts[..parts.len() - 1];
                            format!("{} and {}", head.join(", "), last)
                        }
                    }
                };
                match (query, path) {
                    (Some(q), Some(p)) => {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{}/", p)
                        };
                        (
                            "Search".to_string(),
                            format!("{} in {}", fmt_query(q), display_p),
                        )
                    }
                    (Some(q), None) => ("Search".to_string(), format!("{}", fmt_query(q))),
                    (None, Some(p)) => {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{}/", p)
                        };
                        ("Search".to_string(), format!(" in {}", display_p))
                    }
                    (None, None) => ("Search".to_string(), cmd.clone()),
                }
            }
            ParsedCommand::ReadCommand { cmd } => ("Run".to_string(), cmd.clone()),
            // Upstream variants not present in our core parser are ignored or treated as generic runs
            ParsedCommand::Unknown { cmd } => {
                // Suppress separator helpers like `echo ---` which are used
                // internally to delimit chunks when reading files.
                let t = cmd.trim();
                let lower = t.to_lowercase();
                if lower.starts_with("echo") && lower.contains("---") {
                    (String::new(), String::new()) // drop from preamble
                } else {
                    ("Run".to_string(), format_inline_script_for_display(cmd))
                }
            } // Noop variant not present in our core parser
              // ParsedCommand::Noop { .. } => continue,
        };
        // Enforce per-action grouping: only keep entries matching this cell's action.
        if let Some(exp) = expected_label {
            if label != exp {
                continue;
            }
        } else if !(label == "Run" || label == "Search") {
            // For generic "run" header, keep common run-like labels only.
            continue;
        }
        if label.is_empty() && content.is_empty() {
            continue;
        }
        for line_text in content.lines() {
            if line_text.is_empty() {
                continue;
            }
            let prefix = if !any_content_emitted {
                if suppress_run_header || !use_content_connectors {
                    ""
                } else {
                    "└ "
                }
            } else if suppress_run_header || !use_content_connectors {
                ""
            } else {
                "  "
            };
            let mut spans: Vec<Span<'static>> = Vec::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(
                    prefix,
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            match label.as_str() {
                "Search" => {
                    let remaining = line_text.to_string();
                    let (terms_part, path_part) = if let Some(idx) = remaining.rfind(" (in ") {
                        (
                            remaining[..idx].to_string(),
                            Some(remaining[idx..].to_string()),
                        )
                    } else if let Some(idx) = remaining.rfind(" in ") {
                        let suffix = &remaining[idx + 1..];
                        if suffix.trim_end().ends_with('/') {
                            (
                                remaining[..idx].to_string(),
                                Some(remaining[idx..].to_string()),
                            )
                        } else {
                            (remaining.clone(), None)
                        }
                    } else {
                        (remaining.clone(), None)
                    };
                    let tmp = terms_part.clone();
                    let chunks: Vec<String> = if tmp.contains(", ") {
                        tmp.split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![tmp.clone()]
                    };
                    for (i, chunk) in chunks.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled(
                                ", ",
                                Style::default().fg(crate::colors::text_dim()),
                            ));
                        }
                        if let Some((left, right)) = chunk.rsplit_once(" and ") {
                            if !left.is_empty() {
                                spans.push(Span::styled(
                                    left.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                                spans.push(Span::styled(
                                    " and ",
                                    Style::default().fg(crate::colors::text_dim()),
                                ));
                                spans.push(Span::styled(
                                    right.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                            } else {
                                spans.push(Span::styled(
                                    chunk.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                            }
                        } else {
                            spans.push(Span::styled(
                                chunk.to_string(),
                                Style::default().fg(crate::colors::text()),
                            ));
                        }
                    }
                    if let Some(p) = path_part {
                        spans.push(Span::styled(
                            p,
                            Style::default().fg(crate::colors::text_dim()),
                        ));
                    }
                }
                "Read" => {
                    if let Some(idx) = line_text.find(" (") {
                        let (fname, rest) = line_text.split_at(idx);
                        spans.push(Span::styled(
                            fname.to_string(),
                            Style::default().fg(crate::colors::text()),
                        ));
                        spans.push(Span::styled(
                            rest.to_string(),
                            Style::default().fg(crate::colors::text_dim()),
                        ));
                    } else {
                        spans.push(Span::styled(
                            line_text.to_string(),
                            Style::default().fg(crate::colors::text()),
                        ));
                    }
                }
                "List" => {
                    spans.push(Span::styled(
                        line_text.to_string(),
                        Style::default().fg(crate::colors::text()),
                    ));
                }
                _ => {
                    // Apply shell syntax highlighting to executed command lines.
                    // We highlight the single logical line as bash and append its spans inline.
                    let normalized = normalize_shell_command_display(line_text);
                    let display_line = insert_line_breaks_after_double_ampersand(&normalized);
                    let mut hl =
                        crate::syntax_highlight::highlight_code_block(&display_line, Some("bash"));
                    if let Some(mut first_line) = hl.pop() {
                        emphasize_shell_command_name(&mut first_line);
                        spans.extend(first_line.spans.into_iter());
                    } else {
                        spans.push(Span::styled(
                            display_line,
                            Style::default().fg(crate::colors::text()),
                        ));
                    }
                }
            }
            pre.push(Line::from(spans));
            any_content_emitted = true;
        }
    }

    // If this is a List cell and nothing emitted (e.g., suppressed due to matching Search path),
    // still show a single contextual line so users can see where we listed.
    if matches!(action, ExecAction::List) && !any_content_emitted {
        let display_p = match &ctx_path {
            Some(p) if !p.is_empty() => {
                if p.ends_with('/') {
                    p.to_string()
                } else {
                    format!("{p}/")
                }
            }
            _ => "./".to_string(),
        };
        pre.push(Line::from(vec![
            Span::styled("└ ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                format!("{display_p}"),
                Style::default().fg(crate::colors::text()),
            ),
        ]));
    }

    // Collapse adjacent Read ranges for the same file inside a single exec's preamble
    coalesce_read_ranges_in_lines_local(&mut pre);

    // Output: show stdout only for real run commands; errors always included
    // Collapse adjacent Read ranges for the same file inside a single exec's preamble
    coalesce_read_ranges_in_lines_local(&mut pre);

    if running_status.is_some() {
        if let Some(last) = out.last() {
            let is_blank = last
                .spans
                .iter()
                .all(|sp| sp.content.as_ref().trim().is_empty());
            if is_blank {
                out.pop();
            }
        }
    }

    (pre, out, running_status)
}

fn exec_render_parts_parsed(
    parsed_commands: &[ParsedCommand],
    output: Option<&CommandOutput>,
    stream_preview: Option<&CommandOutput>,
    elapsed_since_start: Option<Duration>,
    status_label: &str,
) -> (
    Vec<Line<'static>>,
    Vec<Line<'static>>,
    Option<Line<'static>>,
) {
    let meta = ParsedExecMetadata::from_commands(parsed_commands);
    exec_render_parts_parsed_with_meta(
        parsed_commands,
        &meta,
        output,
        stream_preview,
        elapsed_since_start,
        status_label,
    )
}

// Local helper: coalesce "<file> (lines A to B)" entries when contiguous.
fn coalesce_read_ranges_in_lines_local(lines: &mut Vec<Line<'static>>) {
    use ratatui::style::Modifier;
    use ratatui::style::Style;
    use ratatui::text::Span;
    // Nothing to do for empty/single line vectors
    if lines.len() <= 1 {
        return;
    }

    // Parse a content line of the form
    //   "└ <file> (lines A to B)" or "  <file> (lines A to B)"
    // into (filename, start, end, prefix, original_index).
    fn parse_read_line_with_index(
        idx: usize,
        line: &Line<'_>,
    ) -> Option<(String, u32, u32, String, usize)> {
        if line.spans.is_empty() {
            return None;
        }
        let prefix = line.spans[0].content.to_string();
        if !(prefix == "└ " || prefix == "  ") {
            return None;
        }
        let rest: String = line
            .spans
            .iter()
            .skip(1)
            .map(|s| s.content.as_ref())
            .collect();
        if let Some(i) = rest.rfind(" (lines ") {
            let fname = rest[..i].to_string();
            let tail = &rest[i + 1..];
            if tail.starts_with("(lines ") && tail.ends_with(")") {
                let inner = &tail[7..tail.len() - 1];
                if let Some((s1, s2)) = inner.split_once(" to ") {
                    if let (Ok(a), Ok(b)) = (s1.trim().parse::<u32>(), s2.trim().parse::<u32>()) {
                        return Some((fname, a, b, prefix, idx));
                    }
                }
            }
        }
        None
    }

    // Collect read ranges grouped by filename, preserving first-seen order.
    // Also track the earliest prefix to reuse when emitting a single line per file.
    #[derive(Default)]
    struct FileRanges {
        prefix: String,
        first_index: usize,
        ranges: Vec<(u32, u32)>,
    }

    let mut files: Vec<(String, FileRanges)> = Vec::new();
    let mut non_read_lines: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if let Some((fname, a, b, prefix, orig_idx)) = parse_read_line_with_index(idx, line) {
            // Insert or update entry for this file, preserving encounter order
            if let Some((_name, fr)) = files.iter_mut().find(|(n, _)| n == &fname) {
                fr.ranges.push((a.min(b), a.max(b)));
                // Keep earliest index as stable ordering anchor
                if orig_idx < fr.first_index {
                    fr.first_index = orig_idx;
                }
            } else {
                files.push((
                    fname,
                    FileRanges {
                        prefix,
                        first_index: orig_idx,
                        ranges: vec![(a.min(b), a.max(b))],
                    },
                ));
            }
        } else {
            non_read_lines.push(line.clone());
        }
    }

    if files.is_empty() {
        return;
    }

    // For each file: merge overlapping/touching ranges; then sort ascending and emit one line.
    fn merge_and_sort(mut v: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
        if v.len() <= 1 {
            return v;
        }
        v.sort_by_key(|(s, _)| *s);
        let mut out: Vec<(u32, u32)> = Vec::with_capacity(v.len());
        let mut cur = v[0];
        for &(s, e) in v.iter().skip(1) {
            if s <= cur.1.saturating_add(1) {
                // touching or overlap
                cur.1 = cur.1.max(e);
            } else {
                out.push(cur);
                cur = (s, e);
            }
        }
        out.push(cur);
        out
    }

    // Rebuild the lines vector: keep header (if present) and any non-read lines,
    // then append one consolidated line per file in first-seen order by index.
    let mut rebuilt: Vec<Line<'static>> = Vec::with_capacity(lines.len());

    // Heuristic: preserve an initial header line that does not start with a connector.
    if !lines.is_empty() {
        if lines[0]
            .spans
            .first()
            .map(|s| s.content.as_ref() != "└ " && s.content.as_ref() != "  ")
            .unwrap_or(false)
        {
            rebuilt.push(lines[0].clone());
        }
    }

    // Sort files by their first appearance index to keep stable ordering with other files.
    files.sort_by_key(|(_n, fr)| fr.first_index);

    for (name, mut fr) in files.into_iter() {
        fr.ranges = merge_and_sort(fr.ranges);
        // Build range annotation: " (lines S1 to E1, S2 to E2, ...)"
        let mut ann = String::new();
        ann.push_str(" (");
        ann.push_str("lines ");
        for (i, (s, e)) in fr.ranges.iter().enumerate() {
            if i > 0 {
                ann.push_str(", ");
            }
            ann.push_str(&format!("{} to {}", s, e));
        }
        ann.push(')');

        let spans: Vec<Span<'static>> = vec![
            Span::styled(fr.prefix, Style::default().add_modifier(Modifier::DIM)),
            Span::styled(name, Style::default().fg(crate::colors::text())),
            Span::styled(ann, Style::default().fg(crate::colors::text_dim())),
        ];
        rebuilt.push(Line::from(spans));
    }

    // Append any other non-read lines (rare for Read sections, but safe)
    // Note: keep their original order after consolidated entries
    rebuilt.extend(non_read_lines.into_iter());

    *lines = rebuilt;
}

impl WidgetRef for &ExecCell {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Text::from(self.display_lines_trimmed()))
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(crate::colors::background()))
            .render(area, buf);
    }
}

/// Return the emoji followed by a hair space (U+200A) and a normal space.
/// This creates a reasonable gap across different terminals,
/// in particular Terminal.app and iTerm, which render too tightly with just a single normal space.
///
/// Improvements here could be to condition this behavior on terminal,
/// or possibly on emoji.
// Removed unused helpers padded_emoji and padded_emoji_with.

pub(crate) fn new_completed_wait_tool_call(target: String, duration: Duration) -> WaitStatusCell {
    let mut duration_str = format_duration(duration);
    if duration_str.ends_with(" 00s") {
        duration_str.truncate(duration_str.len().saturating_sub(4));
    }

    let header = crate::history::WaitStatusHeader {
        title: "Waited".to_string(),
        title_tone: crate::history::TextTone::Success,
        summary: Some(duration_str),
        summary_tone: crate::history::TextTone::Dim,
    };

    let mut details: Vec<crate::history::WaitStatusDetail> = Vec::new();
    if !target.is_empty() {
        details.push(crate::history::WaitStatusDetail {
            label: "for".to_string(),
            value: Some(target),
            tone: crate::history::TextTone::Dim,
        });
    }

    let state = crate::history::WaitStatusState {
        id: crate::history::HistoryId::ZERO,
        header,
        details,
    };

    WaitStatusCell::new(state)
}

// ==================== Helper Functions ====================

// Unified preview format: show first 2 and last 5 non-empty lines with an ellipsis between.
const PREVIEW_HEAD_LINES: usize = 2;
const PREVIEW_TAIL_LINES: usize = 5;
const STREAMING_EXIT_CODE: i32 = i32::MIN;

/// Normalize common TTY overwrite sequences within a text block so that
/// progress lines using carriage returns, backspaces, or ESC[K erase behave as
/// expected when rendered in a pure-buffered UI (no cursor movement).
pub(crate) fn normalize_overwrite_sequences(input: &str) -> String {
    // Process per line, but keep CR/BS/CSI semantics within logical lines.
    // Treat "\n" as committing a line and resetting the cursor.
    let mut out = String::with_capacity(input.len());
    let mut line: Vec<char> = Vec::new(); // visible chars only
    let mut cursor: usize = 0; // column in visible chars

    // Helper to flush current line to out
    let flush_line = |line: &mut Vec<char>, cursor: &mut usize, out: &mut String| {
        if !line.is_empty() {
            out.push_str(&line.iter().collect::<String>());
        }
        out.push('\n');
        line.clear();
        *cursor = 0;
    };

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\n' => {
                flush_line(&mut line, &mut cursor, &mut out);
                i += 1;
            }
            '\r' => {
                // Carriage return: move cursor to column 0
                cursor = 0;
                i += 1;
            }
            '\u{0008}' => {
                // Backspace: move left one column if possible
                if cursor > 0 {
                    cursor -= 1;
                }
                i += 1;
            }
            '\u{001B}' => {
                // CSI: ESC [ ... <cmd>
                if i + 1 < chars.len() && chars[i + 1] == '[' {
                    // Find final byte (alphabetic)
                    let mut j = i + 2;
                    while j < chars.len() && !chars[j].is_alphabetic() {
                        j += 1;
                    }
                    if j < chars.len() {
                        let cmd = chars[j];
                        // Extract numeric prefix (first parameter only)
                        let num: usize = chars[i + 2..j]
                            .iter()
                            .take_while(|c| c.is_ascii_digit())
                            .collect::<String>()
                            .parse()
                            .unwrap_or(0);

                        match cmd {
                            // Erase in Line: 0/None = cursor..end, 1 = start..cursor, 2 = entire line
                            'K' => {
                                let n = num; // default 0 when absent
                                match n {
                                    0 => {
                                        if cursor < line.len() {
                                            line.truncate(cursor);
                                        }
                                    }
                                    1 => {
                                        // Replace from start to cursor with spaces to keep remaining columns stable
                                        let end = cursor.min(line.len());
                                        for k in 0..end {
                                            line[k] = ' ';
                                        }
                                        // Trim leading spaces if the whole line became spaces
                                        while line.last().map_or(false, |c| *c == ' ') {
                                            line.pop();
                                        }
                                    }
                                    2 => {
                                        line.clear();
                                        cursor = 0;
                                    }
                                    _ => {}
                                }
                                i = j + 1;
                                continue;
                            }
                            // Cursor horizontal absolute (1-based)
                            'G' => {
                                let pos = num.saturating_sub(1);
                                cursor = pos.min(line.len());
                                i = j + 1;
                                continue;
                            }
                            // Cursor forward/backward
                            'C' => {
                                cursor = cursor.saturating_add(num);
                                i = j + 1;
                                continue;
                            }
                            'D' => {
                                cursor = cursor.saturating_sub(num);
                                i = j + 1;
                                continue;
                            }
                            _ => {
                                // Unknown/unsupported CSI (incl. SGR 'm'): keep styling intact by
                                // copying the entire sequence verbatim into the output so ANSI
                                // parsing can apply later, but do not affect cursor position.
                                // First, splice current visible buffer into out to preserve order
                                if !line.is_empty() {
                                    out.push_str(&line.iter().collect::<String>());
                                    line.clear();
                                    cursor = 0;
                                }
                                for k in i..=j {
                                    out.push(chars[k]);
                                }
                                i = j + 1;
                                continue;
                            }
                        }
                    } else {
                        // Malformed CSI: drop it entirely by exiting the loop
                        break;
                    }
                } else {
                    // Other ESC sequences (e.g., OSC): pass through verbatim without affecting cursor
                    // Copy ESC and advance one; do not attempt to parse full OSC payload here.
                    if !line.is_empty() {
                        out.push_str(&line.iter().collect::<String>());
                        line.clear();
                        cursor = 0;
                    }
                    out.push(ch);
                    i += 1;
                }
            }
            _ => {
                // Put visible char at cursor, expanding with spaces if needed
                if cursor < line.len() {
                    line[cursor] = ch;
                } else {
                    while line.len() < cursor {
                        line.push(' ');
                    }
                    line.push(ch);
                }
                cursor += 1;
                i += 1;
            }
        }
    }
    // Flush any remaining visible text
    if !line.is_empty() {
        out.push_str(&line.iter().collect::<String>());
    }
    out
}

fn build_preview_lines(text: &str, _include_left_pipe: bool) -> Vec<Line<'static>> {
    // Prefer UI‑themed JSON highlighting when the (ANSI‑stripped) text parses as JSON.
    let stripped_plain = sanitize_for_tui(
        text,
        SanitizeMode::Plain,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&stripped_plain) {
        let pretty =
            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| json_val.to_string());
        let highlighted = crate::syntax_highlight::highlight_code_block(&pretty, Some("json"));
        return select_preview_from_lines(&highlighted, PREVIEW_HEAD_LINES, PREVIEW_TAIL_LINES);
    }

    // Otherwise, compact valid JSON (without ANSI) to improve wrap, or pass original through.
    let processed = format_json_compact(text).unwrap_or_else(|| text.to_string());
    let processed = normalize_overwrite_sequences(&processed);
    let processed = sanitize_for_tui(
        &processed,
        SanitizeMode::AnsiPreserving,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    let non_empty: Vec<&str> = processed.lines().filter(|line| !line.is_empty()).collect();

    enum Seg<'a> {
        Line(&'a str),
        Ellipsis,
    }
    let segments: Vec<Seg> = if non_empty.len() <= PREVIEW_HEAD_LINES + PREVIEW_TAIL_LINES {
        non_empty.iter().map(|s| Seg::Line(s)).collect()
    } else {
        let mut v: Vec<Seg> = Vec::with_capacity(PREVIEW_HEAD_LINES + PREVIEW_TAIL_LINES + 1);
        // Head
        for i in 0..PREVIEW_HEAD_LINES {
            v.push(Seg::Line(non_empty[i]));
        }
        v.push(Seg::Ellipsis);
        // Tail
        let start = non_empty.len().saturating_sub(PREVIEW_TAIL_LINES);
        for s in &non_empty[start..] {
            v.push(Seg::Line(s));
        }
        v
    };

    fn ansi_line_with_theme_bg(s: &str) -> Line<'static> {
        let mut ln = ansi_escape_line(s);
        for sp in ln.spans.iter_mut() {
            sp.style.bg = None;
        }
        ln
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    for seg in segments {
        match seg {
            Seg::Line(line) => out.push(ansi_line_with_theme_bg(line)),
            Seg::Ellipsis => out.push(Line::from("⋮".dim())),
        }
    }
    out
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
        Some(o) => o,
        None => return Vec::new(),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    let is_streaming_preview = *exit_code == STREAMING_EXIT_CODE;

    if !only_err && !stdout.is_empty() {
        lines.extend(build_preview_lines(stdout, include_angle_pipe));
    }

    if !stderr.is_empty() && (is_streaming_preview || *exit_code != 0) {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        if !is_streaming_preview {
            lines.push(Line::styled(
                format!("Error (exit code {})", exit_code),
                Style::default().fg(crate::colors::error()),
            ));
        }
        let stderr_norm = sanitize_for_tui(
            &normalize_overwrite_sequences(stderr),
            SanitizeMode::AnsiPreserving,
            SanitizeOptions {
                expand_tabs: true,
                tabstop: 4,
                debug_markers: false,
            },
        );
        for line in stderr_norm.lines().filter(|line| !line.is_empty()) {
            lines.push(ansi_escape_line(line).style(Style::default().fg(crate::colors::error())));
        }
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }

    lines
}

fn format_mcp_invocation(invocation: McpInvocation) -> Line<'static> {
    let provider_name = pretty_provider_name(&invocation.server);
    let invocation_str = if let Some(args) = invocation.arguments {
        format!("{}.{}({})", provider_name, invocation.tool, args)
    } else {
        format!("{}.{}()", provider_name, invocation.tool)
    };

    Line::styled(
        invocation_str,
        Style::default()
            .fg(crate::colors::text_dim())
            .add_modifier(Modifier::ITALIC),
    )
}

fn pretty_provider_name(id: &str) -> String {
    // Special case common providers with human-friendly names
    match id {
        "brave-search" => "brave",
        "screenshot-website-fast" => "screenshot",
        "read-website-fast" => "readweb",
        "sequential-thinking" => "think",
        "discord-bot" => "discord",
        _ => id,
    }
    .to_string()
}

// ==================== Factory Functions ====================

pub(crate) fn new_background_event(message: String) -> BackgroundEventCell {
    let normalized = normalize_overwrite_sequences(&message);
    let mut collected: Vec<String> = Vec::new();
    for line in normalized.lines() {
        let sanitized_line = ansi_escape_line(line)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        collected.push(sanitized_line);
    }
    let description = collected.join("\n");
    let record = BackgroundEventRecord {
        id: HistoryId::ZERO,
        title: String::new(),
        description,
    };
    BackgroundEventCell::new(record)
}

pub(crate) fn new_session_info(
    config: &Config,
    event: SessionConfiguredEvent,
    is_first_event: bool,
    latest_version: Option<&str>,
) -> PlainMessageState {
    let SessionConfiguredEvent {
        model,
        session_id: _,
        history_log_id: _,
        history_entry_count: _,
        ..
    } = event;

    if is_first_event {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("notice".dim()));
        lines.extend(popular_commands_lines(latest_version));
        plain_message_state_from_lines(lines, HistoryCellType::Notice)
    } else if config.model == model {
        plain_message_state_from_lines(Vec::new(), HistoryCellType::Notice)
    } else {
        let lines = vec![
            Line::from("model changed:")
                .fg(crate::colors::keyword())
                .bold(),
            Line::from(format!("requested: {}", config.model)),
            Line::from(format!("used: {model}")),
            // No empty line at end - trimming and spacing handled by renderer
        ];
        plain_message_state_from_lines(lines, HistoryCellType::Notice)
    }
}

/// Build the common lines for the "Popular commands" section (without the leading
/// "notice" marker). Shared between the initial session info and the startup prelude.
fn popular_commands_lines(_latest_version: Option<&str>) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::styled(
        "Popular commands:",
        Style::default().fg(crate::colors::text_bright()),
    ));
    lines.push(Line::from(vec![
        Span::styled("/settings", Style::default().fg(crate::colors::primary())),
        Span::from(" - "),
        Span::from(SlashCommand::Settings.description())
            .style(Style::default().add_modifier(Modifier::DIM)),
        Span::styled(
            " NEW",
            Style::default().fg(crate::colors::primary()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("/auto", Style::default().fg(crate::colors::primary())),
        Span::from(" - "),
        Span::from(SlashCommand::Auto.description())
            .style(Style::default().add_modifier(Modifier::DIM)),
        Span::styled(
            " UPDATED",
            Style::default().fg(crate::colors::primary()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("/chrome", Style::default().fg(crate::colors::primary())),
        Span::from(" - "),
        Span::from(SlashCommand::Chrome.description())
            .style(Style::default().add_modifier(Modifier::DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("/plan", Style::default().fg(crate::colors::primary())),
        Span::from(" - "),
        Span::from(SlashCommand::Plan.description())
            .style(Style::default().add_modifier(Modifier::DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("/code", Style::default().fg(crate::colors::primary())),
        Span::from(" - "),
        Span::from(SlashCommand::Code.description())
            .style(Style::default().add_modifier(Modifier::DIM)),
    ]));

    lines
}

/// Create a notice cell that shows the "Popular commands" immediately.
/// If `connecting_mcp` is true, include a dim status line to inform users
/// that external MCP servers are being connected in the background.
pub(crate) fn new_upgrade_prelude(
    latest_version: Option<&str>,
) -> Option<UpgradeNoticeCell> {
    if !crate::updates::upgrade_ui_enabled() {
        return None;
    }
    let latest = latest_version?.trim();
    if latest.is_empty() {
        return None;
    }

    let current = code_version::version();
    if latest == current {
        return None;
    }

    let state = UpgradeNoticeState {
        id: HistoryId::ZERO,
        current_version: current.trim().to_string(),
        latest_version: latest.trim().to_string(),
        message: "Use /upgrade to upgrade now or enable auto-update.".to_string(),
    };

    Some(UpgradeNoticeCell::new(state))
}

pub(crate) fn new_popular_commands_notice(
    _connecting_mcp: bool,
    latest_version: Option<&str>,
) -> PlainMessageState {
    if crate::chatwidget::is_test_mode() {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        let legacy_line = "  /code - perform a coding task (multiple agents)";
        #[cfg(any(test, feature = "test-helpers"))]
        println!("legacy command line: {legacy_line}");
        lines.push(Line::from(legacy_line));
        return plain_message_state_from_lines(lines, HistoryCellType::Notice);
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("notice".dim()));
    lines.extend(popular_commands_lines(latest_version));
    // Connecting status is now rendered as a separate BackgroundEvent cell
    // with its own gutter icon and spacing. Keep this notice focused.
    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

/// Background status cell shown during startup while external MCP servers
/// are being connected. Uses the standard background-event gutter (»)
/// and inserts a blank line above the message for visual separation from
/// the Popular commands block.
pub(crate) fn new_connecting_mcp_status() -> BackgroundEventCell {
    let record = BackgroundEventRecord {
        id: HistoryId::ZERO,
        title: String::new(),
        description: "\nConnecting MCP servers…".to_string(),
    };
    BackgroundEventCell::new(record)
}

pub(crate) fn new_user_prompt(message: String) -> PlainMessageState {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("user"));
    // Sanitize user-provided text for terminal safety and stable layout:
    // - Normalize common TTY overwrite sequences (\r, \x08, ESC[K)
    // - Expand tabs to spaces with a fixed tab stop so wrapping is deterministic
    // - Parse ANSI sequences into spans so we never emit raw control bytes
    let normalized = normalize_overwrite_sequences(&message);
    let sanitized = sanitize_for_tui(
        &normalized,
        SanitizeMode::AnsiPreserving,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    // Build content lines with ANSI converted to styled spans
    let content: Vec<Line<'static>> = sanitized.lines().map(|l| ansi_escape_line(l)).collect();
    let content = trim_empty_lines(content);
    lines.extend(content);
    // No empty line at end - trimming and spacing handled by renderer
    plain_message_state_from_lines(lines, HistoryCellType::User)
}

/// Render a queued user message that will be sent in the next turn.
/// Visually identical to a normal user cell, but the header shows a
/// small "(queued)" suffix so it’s clear it hasn’t been executed yet.
pub(crate) fn new_queued_user_prompt(message: String) -> PlainMessageState {
    use ratatui::style::Style;
    use ratatui::text::Span;
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Header: "user (queued)"
    lines.push(Line::from(vec![
        Span::from("user "),
        Span::from("(queued)").style(Style::default().fg(crate::colors::text_dim())),
    ]));
    // Normalize and render body like normal user messages
    let normalized = normalize_overwrite_sequences(&message);
    let sanitized = sanitize_for_tui(
        &normalized,
        SanitizeMode::AnsiPreserving,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    let content: Vec<Line<'static>> = sanitized.lines().map(|l| ansi_escape_line(l)).collect();
    let content = trim_empty_lines(content);
    lines.extend(content);
    plain_message_state_from_lines(lines, HistoryCellType::User)
}

/// Expand horizontal tabs to spaces using a fixed tab stop.
/// This prevents terminals from applying their own tab expansion after
/// ratatui has computed layout, which can otherwise cause glyphs to appear
/// to "hang" or smear until overwritten.
// Tab expansion and control stripping are centralized in crate::sanitize

#[allow(dead_code)]
pub(crate) fn new_text_line(line: Line<'static>) -> PlainMessageState {
    plain_message_state_from_lines(vec![line], HistoryCellType::Notice)
}

pub(crate) fn new_streaming_content(
    state: AssistantStreamState,
    cfg: &Config,
) -> StreamingContentCell {
    StreamingContentCell::from_state(state, cfg.file_opener, cfg.cwd.clone())
}

pub(crate) fn new_animated_welcome() -> AnimatedWelcomeCell {
    AnimatedWelcomeCell::new()
}

#[allow(dead_code)]
pub(crate) fn new_loading_cell(message: String) -> LoadingCell {
    LoadingCell::new(message)
}

fn exec_command_lines(
    command: &[String],
    parsed: &[ParsedCommand],
    output: Option<&CommandOutput>,
    stream_preview: Option<&CommandOutput>,
    start_time: Option<Instant>,
) -> Vec<Line<'static>> {
    match parsed.is_empty() {
        true => new_exec_command_generic(command, output, stream_preview, start_time),
        false => new_parsed_command(parsed, output, stream_preview, start_time),
    }
}

// Legacy helper removed in favor of ExecAction (action_enum_from_parsed)

fn first_context_path(parsed_commands: &[ParsedCommand]) -> Option<String> {
    for parsed in parsed_commands.iter() {
        match parsed {
            ParsedCommand::ListFiles { path, .. } => {
                if let Some(p) = path {
                    return Some(p.clone());
                }
            }
            ParsedCommand::Search { path, .. } => {
                if let Some(p) = path {
                    return Some(p.clone());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_read_line_annotation_with_range(cmd: &str) -> (Option<String>, Option<(u32, u32)>) {
    let lower = cmd.to_lowercase();
    // Try sed -n '<start>,<end>p'
    if lower.contains("sed") && lower.contains("-n") {
        // Look for a token like 123,456p possibly quoted
        for raw in cmd.split(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
            let token = raw.trim();
            if token.ends_with('p') {
                let core = &token[..token.len().saturating_sub(1)];
                if let Some((a, b)) = core.split_once(',') {
                    if let (Ok(start), Ok(end)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>())
                    {
                        return (
                            Some(format!("(lines {} to {})", start, end)),
                            Some((start, end)),
                        );
                    }
                }
            }
        }
    }
    // head -n N => lines 1..N
    if lower.contains("head") && lower.contains("-n") {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        // Find the position of "head" command first
        let head_pos = parts.iter().position(|p| {
            let lower = p.to_lowercase();
            lower == "head" || lower.ends_with("/head")
        });

        if let Some(head_idx) = head_pos {
            // Only look for -n after the head command position
            for i in head_idx..parts.len() {
                if parts[i] == "-n" && i + 1 < parts.len() {
                    if let Ok(n) = parts[i + 1]
                        .trim_matches('"')
                        .trim_matches('\'')
                        .parse::<u32>()
                    {
                        return (Some(format!("(lines 1 to {})", n)), Some((1, n)));
                    }
                }
            }
        }
    }
    // bare `head` => default 10 lines
    if lower.contains("head") && !lower.contains("-n") {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.iter().any(|p| *p == "head") {
            return (Some("(lines 1 to 10)".to_string()), Some((1, 10)));
        }
    }
    // tail -n +K => from K to end; tail -n N => last N lines
    if lower.contains("tail") && lower.contains("-n") {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        // Find the position of "tail" command first
        let tail_pos = parts.iter().position(|p| {
            let lower = p.to_lowercase();
            lower == "tail" || lower.ends_with("/tail")
        });

        if let Some(tail_idx) = tail_pos {
            // Only look for -n after the tail command position
            for i in tail_idx..parts.len() {
                if parts[i] == "-n" && i + 1 < parts.len() {
                    let val = parts[i + 1].trim_matches('"').trim_matches('\'');
                    if let Some(rest) = val.strip_prefix('+') {
                        if let Ok(k) = rest.parse::<u32>() {
                            return (Some(format!("(from {} to end)", k)), Some((k, u32::MAX)));
                        }
                    } else if let Ok(n) = val.parse::<u32>() {
                        return (Some(format!("(last {} lines)", n)), None);
                    }
                }
            }
        }
    }
    // bare `tail` => default 10 lines
    if lower.contains("tail") && !lower.contains("-n") {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.iter().any(|p| *p == "tail") {
            return (Some("(last 10 lines)".to_string()), None);
        }
    }
    (None, None)
}

fn parse_read_line_annotation(cmd: &str) -> Option<String> {
    parse_read_line_annotation_with_range(cmd).0
}

fn normalize_shell_command_display(cmd: &str) -> String {
    let first_non_ws = cmd
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx);
    let Some(start) = first_non_ws else {
        return cmd.to_string();
    };
    if cmd[start..].starts_with("./") {
        let mut normalized = String::with_capacity(cmd.len().saturating_sub(2));
        normalized.push_str(&cmd[..start]);
        normalized.push_str(&cmd[start + 2..]);
        normalized
    } else {
        cmd.to_string()
    }
}

fn insert_line_breaks_after_double_ampersand(cmd: &str) -> String {
    if !cmd.contains("&&") {
        return cmd.to_string();
    }

    let mut result = String::with_capacity(cmd.len() + 8);
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;

    while i < cmd.len() {
        let ch = cmd[i..].chars().next().expect("valid char boundary");
        let ch_len = ch.len_utf8();

        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                result.push(ch);
                i += ch_len;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                result.push(ch);
                i += ch_len;
                continue;
            }
            '&' if !in_single && !in_double => {
                let next_idx = i + ch_len;
                if next_idx < cmd.len() {
                    if let Some(next_ch) = cmd[next_idx..].chars().next() {
                        if next_ch == '&' {
                            result.push('&');
                            result.push('&');
                            i = next_idx + next_ch.len_utf8();
                            while i < cmd.len() {
                                let ahead = cmd[i..].chars().next().expect("valid char boundary");
                                if ahead.is_whitespace() {
                                    i += ahead.len_utf8();
                                    continue;
                                }
                                break;
                            }
                            if i < cmd.len() {
                                result.push('\n');
                            }
                            continue;
                        }
                    }
                }
            }
            _ => {}
        }

        result.push(ch);
        i += ch_len;
    }

    result
}

fn emphasize_shell_command_name(line: &mut Line<'static>) {
    let mut emphasized = false;
    let mut rebuilt: Vec<Span<'static>> = Vec::with_capacity(line.spans.len());

    for span in line.spans.drain(..) {
        if emphasized {
            rebuilt.push(span);
            continue;
        }

        let style = span.style;
        let content_owned = span.content.into_owned();

        if content_owned.trim().is_empty() {
            rebuilt.push(Span::styled(content_owned, style));
            continue;
        }

        let mut token_start: Option<usize> = None;
        for (idx, ch) in content_owned.char_indices() {
            if !ch.is_whitespace() {
                token_start = Some(idx);
                break;
            }
        }

        let Some(start) = token_start else {
            rebuilt.push(Span::styled(content_owned, style));
            continue;
        };

        let mut end = content_owned.len();
        for (offset, ch) in content_owned[start..].char_indices() {
            if ch.is_whitespace() {
                end = start + offset;
                break;
            }
        }

        let before = &content_owned[..start];
        let token = &content_owned[start..end];
        let after = &content_owned[end..];

        if !before.is_empty() {
            rebuilt.push(Span::styled(before.to_string(), style));
        }

        if token.chars().count() <= 4 {
            rebuilt.push(Span::styled(token.to_string(), style));
        } else {
            let bright_style = style
                .fg(crate::colors::text_bright())
                .add_modifier(Modifier::BOLD);
            rebuilt.push(Span::styled(token.to_string(), bright_style));
        }

        if !after.is_empty() {
            rebuilt.push(Span::styled(after.to_string(), style));
        }

        emphasized = true;
    }

    if emphasized {
        line.spans = rebuilt;
    } else if !rebuilt.is_empty() {
        line.spans = rebuilt;
    }
}

fn format_inline_script_for_display(command_escaped: &str) -> String {
    if let Some(formatted) = try_format_inline_python(command_escaped) {
        return formatted;
    }
    if let Some(formatted) = format_inline_node_for_display(command_escaped) {
        return formatted;
    }
    if let Some(formatted) = format_inline_shell_for_display(command_escaped) {
        return formatted;
    }
    command_escaped.to_string()
}

fn try_format_inline_python(command_escaped: &str) -> Option<String> {
    if let Some(formatted) = format_python_dash_c(command_escaped) {
        return Some(formatted);
    }
    if let Some(formatted) = format_python_heredoc(command_escaped) {
        return Some(formatted);
    }
    None
}

fn format_python_dash_c(command_escaped: &str) -> Option<String> {
    let tokens: Vec<String> = Shlex::new(command_escaped).collect();
    if tokens.len() < 3 {
        return None;
    }

    let python_idx = tokens
        .iter()
        .position(|token| is_python_invocation_token(token))?;

    let c_idx = tokens
        .iter()
        .enumerate()
        .skip(python_idx + 1)
        .find_map(|(idx, token)| if token == "-c" { Some(idx) } else { None })?;

    let script_idx = c_idx + 1;
    if script_idx >= tokens.len() {
        return None;
    }

    let script_raw = tokens[script_idx].as_str();
    if script_raw.is_empty() {
        return None;
    }

    let script_block = build_python_script_block(script_raw)?;

    let mut parts: Vec<String> = Vec::with_capacity(tokens.len());
    for (idx, token) in tokens.iter().enumerate() {
        if idx == script_idx {
            parts.push(script_block.clone());
        } else {
            parts.push(escape_token_for_display(token));
        }
    }

    Some(parts.join(" "))
}

fn build_python_script_block(script: &str) -> Option<String> {
    let normalized = script.replace("\r\n", "\n");
    let lines: Vec<String> = if normalized.contains('\n') {
        normalized
            .lines()
            .map(|line| line.trim_end().to_string())
            .collect()
    } else if script_has_semicolon_outside_quotes(&normalized) {
        split_semicolon_statements(&normalized)
    } else {
        return None;
    };

    let meaningful: Vec<String> = merge_from_import_lines(lines)
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.trim().is_empty())
        .collect();

    if meaningful.len() <= 1 {
        return None;
    }

    let indented = indent_python_lines(meaningful);

    let mut block = String::from("'\n");
    for line in indented {
        block.push_str("    ");
        block.push_str(line.as_str());
        block.push('\n');
    }
    block.push('\'');
    Some(block)
}

fn format_python_heredoc(command_escaped: &str) -> Option<String> {
    let tokens: Vec<String> = Shlex::new(command_escaped).collect();
    if tokens.len() < 3 {
        return None;
    }

    let python_idx = tokens
        .iter()
        .position(|token| is_python_invocation_token(token))?;

    let heredoc_idx = tokens
        .iter()
        .enumerate()
        .skip(python_idx + 1)
        .find_map(|(idx, token)| heredoc_delimiter(token).map(|delim| (idx, delim)))?;

    let (marker_idx, terminator) = heredoc_idx;
    let closing_idx = tokens
        .iter()
        .enumerate()
        .skip(marker_idx + 1)
        .rev()
        .find_map(|(idx, token)| (token == &terminator).then_some(idx))?;

    if closing_idx <= marker_idx + 1 {
        return None;
    }

    let script_tokens = &tokens[marker_idx + 1..closing_idx];
    if script_tokens.is_empty() {
        return None;
    }

    let script_lines = split_heredoc_script_lines(script_tokens);
    if script_lines.is_empty() {
        return None;
    }

    let script_lines = indent_python_lines(merge_from_import_lines(script_lines));

    let header_tokens: Vec<String> = tokens[..=marker_idx]
        .iter()
        .map(|t| escape_token_for_display(t))
        .collect();

    let mut result = header_tokens.join(" ");
    if !result.ends_with('\n') {
        result.push('\n');
    }

    for line in script_lines {
        result.push_str("    ");
        result.push_str(line.trim_end());
        result.push('\n');
    }

    result.push_str(&escape_token_for_display(&tokens[closing_idx]));

    if closing_idx + 1 < tokens.len() {
        let tail: Vec<String> = tokens[closing_idx + 1..]
            .iter()
            .map(|t| escape_token_for_display(t))
            .collect();
        if !tail.is_empty() {
            result.push(' ');
            result.push_str(&tail.join(" "));
        }
    }

    Some(result)
}

fn heredoc_delimiter(token: &str) -> Option<String> {
    if !token.starts_with("<<") {
        return None;
    }
    let mut delim = token.trim_start_matches("<<").to_string();
    if delim.is_empty() {
        return None;
    }
    if delim.starts_with('"') && delim.ends_with('"') && delim.len() >= 2 {
        delim = delim[1..delim.len() - 1].to_string();
    } else if delim.starts_with('\'') && delim.ends_with('\'') && delim.len() >= 2 {
        delim = delim[1..delim.len() - 1].to_string();
    }
    if delim.is_empty() {
        None
    } else {
        Some(delim)
    }
}

fn split_heredoc_script_lines(script_tokens: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut current_has_assignment = false;

    for (idx, token) in script_tokens.iter().enumerate() {
        if !current.is_empty()
            && paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
        {
            let token_lower = token.to_ascii_lowercase();
            let current_first = current.first().map(|s| s.to_ascii_lowercase());
            let should_flush_before = is_statement_boundary_token(token)
                && !(token_lower == "import"
                    && current_first.as_deref() == Some("from"));
            if should_flush_before {
                let line = current.join(" ");
                lines.push(line.trim().to_string());
                current.clear();
                current_has_assignment = false;
            }
        }

        current.push(token.clone());
        adjust_bracket_depth(token, &mut paren_depth, &mut bracket_depth, &mut brace_depth);

        if is_assignment_operator(token) {
            current_has_assignment = true;
        }

        let next = script_tokens.get(idx + 1);
        let mut should_break = false;
        let mut break_here = false;

        if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            if next.is_none() {
                should_break = true;
            } else {
                let next_token = next.unwrap();
                if is_statement_boundary_token(next_token) {
                    should_break = true;
                } else if current
                    .first()
                    .map(|s| s.as_str() == "import" || s.as_str() == "from")
                    .unwrap_or(false)
                {
                    if current.len() > 1 && next_token != "as" && next_token != "," {
                        should_break = true;
                    }
                } else if current_has_assignment
                    && !is_assignment_operator(token)
                    && next_token
                        .chars()
                        .next()
                        .map(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        .unwrap_or(false)
                    && !next_token.contains('(')
                {
                    should_break = true;
                }

                let token_trimmed = token.trim_matches(|c| c == ')' || c == ']' || c == '}');
                if token_trimmed.ends_with(':') {
                    break_here = true;
                }

                let lowered = token.trim().to_ascii_lowercase();
                if matches!(lowered.as_str(), "return" | "break" | "continue" | "pass") {
                    break_here = true;
                }

                if let Some(next_token) = next {
                    let next_str = next_token.as_str();
                    if token.ends_with(')')
                        && (next_str.contains('.')
                            || next_str.contains('=')
                            || next_str.starts_with("print"))
                    {
                        break_here = true;
                    }
                }
            }
        }

        if break_here {
            let line = current.join(" ");
            lines.push(line.trim().to_string());
            current.clear();
            current_has_assignment = false;
            continue;
        }

        if should_break {
            let line = current.join(" ");
            lines.push(line.trim().to_string());
            current.clear();
            current_has_assignment = false;
        }
    }

    if !current.is_empty() {
        let line = current.join(" ");
        lines.push(line.trim().to_string());
    }

    lines.into_iter().filter(|line| !line.is_empty()).collect()
}

fn is_statement_boundary_token(token: &str) -> bool {
    matches!(
        token,
        "import"
            | "from"
            | "def"
            | "class"
            | "if"
            | "elif"
            | "else"
            | "for"
            | "while"
            | "try"
            | "except"
            | "with"
            | "return"
            | "raise"
            | "pass"
            | "continue"
            | "break"
    ) || token.starts_with("print")
}

fn indent_python_lines(lines: Vec<String>) -> Vec<String> {
    let mut indented: Vec<String> = Vec::with_capacity(lines.len());
    let mut indent_level: usize = 0;
    let mut pending_dedent_after_flow = false;

    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            indented.push(String::new());
            continue;
        }

        let lowered_first = trimmed
            .split_whitespace()
            .next()
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if pending_dedent_after_flow
            && !matches!(
                lowered_first.as_str(),
                "elif" | "else" | "except" | "finally"
            )
        {
            if indent_level > 0 {
                indent_level -= 1;
            }
        }
        pending_dedent_after_flow = false;

        if matches!(
            lowered_first.as_str(),
            "elif" | "else" | "except" | "finally"
        ) {
            if indent_level > 0 {
                indent_level -= 1;
            }
        }

        let mut line = String::with_capacity(trimmed.len() + indent_level * 4);
        for _ in 0..indent_level {
            line.push_str("    ");
        }
        line.push_str(trimmed);
        indented.push(line);

        if trimmed.ends_with(':')
            && !matches!(
                lowered_first.as_str(),
                "return" | "break" | "continue" | "pass" | "raise"
            )
        {
            indent_level += 1;
        } else if matches!(
            lowered_first.as_str(),
            "return" | "break" | "continue" | "pass" | "raise"
        ) {
            pending_dedent_after_flow = true;
        }
    }

    indented
}

fn merge_from_import_lines(lines: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::with_capacity(lines.len());
    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx].trim().to_string();
        if line.starts_with("from ")
            && idx + 1 < lines.len()
            && lines[idx + 1].trim_start().starts_with("import ")
        {
            let combined = format!(
                "{} {}",
                line.trim_end(),
                lines[idx + 1].trim_start()
            );
            merged.push(combined);
            idx += 2;
        } else {
            merged.push(line);
            idx += 1;
        }
    }
    merged
}

fn is_assignment_operator(token: &str) -> bool {
    matches!(
        token,
        "="
            | "+="
            | "-="
            | "*="
            | "/="
            | "//="
            | "%="
            | "^="
            | "|="
            | "&="
            | "**="
            | "<<="
            | ">>="
    )
}

fn is_shell_executable(token: &str) -> bool {
    let trimmed = token.trim_matches(|c| c == '\'' || c == '"');
    let lowered = Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    matches!(
        lowered.as_str(),
        "bash"
            | "bash.exe"
            | "sh"
            | "sh.exe"
            | "dash"
            | "dash.exe"
            | "zsh"
            | "zsh.exe"
            | "ksh"
            | "ksh.exe"
            | "busybox"
    )
}


fn is_node_invocation_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|c| c == '\'' || c == '"');
    let base = Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    matches!(base.as_str(), "node" | "node.exe" | "nodejs" | "nodejs.exe")
}

fn format_node_script(tokens: &[String], script_idx: usize, script: &str) -> Option<String> {
    let block = build_js_script_block(script)?;
    let mut parts: Vec<String> = Vec::with_capacity(tokens.len());
    for (idx, token) in tokens.iter().enumerate() {
        if idx == script_idx {
            parts.push(block.clone());
        } else {
            parts.push(escape_token_for_display(token));
        }
    }
    Some(parts.join(" "))
}

fn build_js_script_block(script: &str) -> Option<String> {
    let normalized = script.replace("\r\n", "\n");
    let lines: Vec<String> = if normalized.contains('\n') {
        normalized
            .lines()
            .map(|line| line.trim_end().to_string())
            .collect()
    } else {
        split_js_statements(&normalized)
    };

    let meaningful: Vec<String> = lines
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if meaningful.len() <= 1 {
        return None;
    }

    let indented = indent_js_lines(meaningful);
    let mut block = String::from("'\n");
    for line in indented {
        block.push_str("    ");
        block.push_str(line.as_str());
        block.push('\n');
    }
    block.push('\'');
    Some(block)
}

fn split_js_statements(script: &str) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut escape = false;
    let mut paren_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut bracket_depth = 0i32;

    for ch in script.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' if in_single || in_double || in_backtick => {
                escape = true;
                current.push(ch);
                continue;
            }
            '\'' if !in_double && !in_backtick => {
                in_single = !in_single;
                current.push(ch);
                continue;
            }
            '"' if !in_single && !in_backtick => {
                in_double = !in_double;
                current.push(ch);
                continue;
            }
            '`' if !in_single && !in_double => {
                in_backtick = !in_backtick;
                current.push(ch);
                continue;
            }
            _ => {}
        }

        if !(in_single || in_double || in_backtick) {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                }
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                '[' => bracket_depth += 1,
                ']' => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                ';' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                    current.push(ch);
                    let seg = current.trim().to_string();
                    if !seg.is_empty() {
                        segments.push(seg);
                    }
                    current.clear();
                    continue;
                }
                '\n' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                    let seg = current.trim().to_string();
                    if !seg.is_empty() {
                        segments.push(seg);
                    }
                    current.clear();
                    continue;
                }
                _ => {}
            }
        }

        current.push(ch);
    }

    let seg = current.trim().to_string();
    if !seg.is_empty() {
        segments.push(seg);
    }
    segments
}

fn indent_js_lines(lines: Vec<String>) -> Vec<String> {
    let mut indented: Vec<String> = Vec::with_capacity(lines.len());
    let mut indent_level: usize = 0;

    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            indented.push(String::new());
            continue;
        }

        let mut leading_closers = 0usize;
        let mut cut = trimmed.len();
        for (idx, ch) in trimmed.char_indices() {
            match ch {
                '}' | ']' => {
                    leading_closers += 1;
                    cut = idx + ch.len_utf8();
                    continue;
                }
                _ => {
                    cut = idx;
                    break;
                }
            }
        }

        if leading_closers > 0 && cut >= trimmed.len() {
            cut = trimmed.len();
        }

        if leading_closers > 0 {
            indent_level = indent_level.saturating_sub(leading_closers);
        }

        let remainder = trimmed[cut..].trim_start();
        let mut line = String::with_capacity(remainder.len() + indent_level * 4);
        for _ in 0..indent_level {
            line.push_str("    ");
        }
        if remainder.is_empty() && cut < trimmed.len() {
            line.push_str(trimmed);
        } else {
            line.push_str(remainder);
        }
        indented.push(line);

        let (opens, closes) = js_brace_deltas(trimmed);
        indent_level = indent_level + opens;
        indent_level = indent_level.saturating_sub(closes);
    }

    indented
}

fn js_brace_deltas(line: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut escape = false;

    for ch in line.chars() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double || in_backtick => {
                escape = true;
            }
            '\'' if !in_double && !in_backtick => in_single = !in_single,
            '"' if !in_single && !in_backtick => in_double = !in_double,
            '`' if !in_single && !in_double => in_backtick = !in_backtick,
            '{' if !(in_single || in_double || in_backtick) => opens += 1,
            '}' if !(in_single || in_double || in_backtick) => closes += 1,
            _ => {}
        }
    }

    (opens, closes)
}

fn is_shell_invocation_token(token: &str) -> bool {
    is_shell_executable(token)
}

fn format_shell_script(tokens: &[String], script_idx: usize, script: &str) -> Option<String> {
    let block = build_shell_script_block(script)?;
    let mut parts: Vec<String> = Vec::with_capacity(tokens.len());
    for (idx, token) in tokens.iter().enumerate() {
        if idx == script_idx {
            parts.push(block.clone());
        } else {
            parts.push(escape_token_for_display(token));
        }
    }
    Some(parts.join(" "))
}

fn build_shell_script_block(script: &str) -> Option<String> {
    let normalized = script.replace("\r\n", "\n");
    let segments = split_shell_statements(&normalized);
    let meaningful: Vec<String> = segments
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    if meaningful.len() <= 1 {
        return None;
    }
    let indented = indent_shell_lines(meaningful);
    let mut block = String::from("'\n");
    for line in indented {
        block.push_str("    ");
        block.push_str(line.as_str());
        block.push('\n');
    }
    block.push('\'');
    Some(block)
}

fn split_shell_statements(script: &str) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    let chars: Vec<char> = script.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let ch = chars[idx];
        if escape {
            current.push(ch);
            escape = false;
            idx += 1;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => {
                escape = true;
                current.push(ch);
                idx += 1;
                continue;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
                idx += 1;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
                idx += 1;
                continue;
            }
            ';' if !(in_single || in_double) => {
                current.push(ch);
                segments.push(current.trim().to_string());
                current.clear();
                idx += 1;
                continue;
            }
            '&' | '|' if !(in_single || in_double) => {
                let current_op = ch;
                if idx + 1 < chars.len() && chars[idx + 1] == current_op {
                    if !current.trim().is_empty() {
                        segments.push(current.trim().to_string());
                    }
                    segments.push(format!("{}{}", current_op, current_op));
                    current.clear();
                    idx += 2;
                    continue;
                }
            }
            '\n' if !(in_single || in_double) => {
                segments.push(current.trim().to_string());
                current.clear();
                idx += 1;
                continue;
            }
            _ => {}
        }
        current.push(ch);
        idx += 1;
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}

fn indent_shell_lines(lines: Vec<String>) -> Vec<String> {
    let mut indented: Vec<String> = Vec::with_capacity(lines.len());
    let mut indent_level: usize = 0;

    for raw in lines {
        if raw == "&&" || raw == "||" {
            let mut line = String::new();
            for _ in 0..indent_level {
                line.push_str("    ");
            }
            line.push_str(raw.as_str());
            indented.push(line);
            continue;
        }

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            indented.push(String::new());
            continue;
        }

        if trimmed.starts_with("fi") || trimmed.starts_with("done") || trimmed.starts_with("esac") {
            indent_level = indent_level.saturating_sub(1);
        }

        let mut line = String::new();
        for _ in 0..indent_level {
            line.push_str("    ");
        }
        line.push_str(trimmed);
        indented.push(line);

        if trimmed.ends_with("do")
            || trimmed.ends_with("then")
            || trimmed.ends_with("{")
            || trimmed.starts_with("case ")
        {
            indent_level += 1;
        }
    }

    indented
}

fn adjust_bracket_depth(token: &str, paren: &mut i32, bracket: &mut i32, brace: &mut i32) {
    for ch in token.chars() {
        match ch {
            '(' => *paren += 1,
            ')' => *paren -= 1,
            '[' => *bracket += 1,
            ']' => *bracket -= 1,
            '{' => *brace += 1,
            '}' => *brace -= 1,
            _ => {}
        }
    }
    *paren = (*paren).max(0);
    *bracket = (*bracket).max(0);
    *brace = (*brace).max(0);
}

fn is_python_invocation_token(token: &str) -> bool {
    if token.is_empty() || token.contains('=') {
        return false;
    }

    let trimmed = token.trim_matches(|c| c == '\'' || c == '"');
    let base = Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(trimmed)
        .to_ascii_lowercase();

    if !base.starts_with("python") {
        return false;
    }

    let suffix = &base["python".len()..];
    suffix.is_empty()
        || suffix
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == 'w')
}

fn escape_token_for_display(token: &str) -> String {
    if is_shell_word(token) {
        token.to_string()
    } else {
        let mut escaped = String::from("'");
        for ch in token.chars() {
            if ch == '\'' {
                escaped.push_str("'\\''");
            } else {
                escaped.push(ch);
            }
        }
        escaped.push('\'');
        escaped
    }
}

fn is_shell_word(token: &str) -> bool {
    token.chars().all(|ch| matches!(
        ch,
        'a'..='z'
            | 'A'..='Z'
            | '0'..='9'
            | '_'
            | '-'
            | '.'
            | '/'
            | ':'
            | ','
            | '@'
            | '%'
            | '+'
            | '='
            | '['
            | ']'
    ))
}

fn script_has_semicolon_outside_quotes(script: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in script.chars() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => {
                escape = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ';' if !in_single && !in_double => return true,
            _ => {}
        }
    }

    false
}

fn split_semicolon_statements(script: &str) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in script.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' if in_single || in_double => {
                escape = true;
                current.push(ch);
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            ';' if !in_single && !in_double => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    segments.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    segments
}

fn running_status_line(message: String) -> Line<'static> {
    Line::from(vec![
        Span::styled("└ ", Style::default().fg(crate::colors::border_dim())),
        Span::styled(message, Style::default().fg(crate::colors::text_dim())),
    ])
}

fn new_parsed_command(
    parsed_commands: &[ParsedCommand],
    output: Option<&CommandOutput>,
    stream_preview: Option<&CommandOutput>,
    start_time: Option<Instant>,
) -> Vec<Line<'static>> {
    let meta = ParsedExecMetadata::from_commands(parsed_commands);
    let action = meta.action;
    let ctx_path = meta.ctx_path.as_deref();
    let suppress_run_header = matches!(action, ExecAction::Run) && output.is_some();
    let mut lines: Vec<Line> = Vec::new();
    let mut running_status: Option<Line<'static>> = None;
    if !suppress_run_header {
        match output {
            None => {
                if matches!(action, ExecAction::Run) {
                    let mut message = match &ctx_path {
                        Some(p) => format!("Running... in {p}"),
                        None => "Running...".to_string(),
                    };
                    if let Some(start) = start_time {
                        let elapsed = start.elapsed();
                        message = format!("{message} ({})", format_duration(elapsed));
                    }
                    running_status = Some(running_status_line(message));
                } else {
                    let duration_suffix = if let Some(start) = start_time {
                        let elapsed = start.elapsed();
                        format!(" ({})", format_duration(elapsed))
                    } else {
                        String::new()
                    };
                    let header = match action {
                        ExecAction::Read => "Read",
                        ExecAction::Search => "Search",
                        ExecAction::List => "List",
                        ExecAction::Run => unreachable!(),
                    };
                    lines.push(Line::styled(
                        format!("{header}{duration_suffix}"),
                        Style::default().fg(crate::colors::text_dim()),
                    ));
                }
            }
            Some(o) if o.exit_code == 0 => {
                if matches!(
                    action,
                    ExecAction::Read | ExecAction::Search | ExecAction::List
                ) {
                    lines.push(Line::styled(
                        match action {
                            ExecAction::Read => "Read",
                            ExecAction::Search => "Search",
                            ExecAction::List => "List",
                            ExecAction::Run => unreachable!(),
                        },
                        Style::default().fg(crate::colors::text()),
                    ));
                } else {
                    let done = match ctx_path {
                        Some(p) => format!("Ran in {p}"),
                        None => "Ran".to_string(),
                    };
                    lines.push(Line::styled(
                        done,
                        Style::default()
                            .fg(crate::colors::text_bright())
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
            Some(_o) => {
                if matches!(
                    action,
                    ExecAction::Read | ExecAction::Search | ExecAction::List
                ) {
                    lines.push(Line::styled(
                        match action {
                            ExecAction::Read => "Read",
                            ExecAction::Search => "Search",
                            ExecAction::List => "List",
                            ExecAction::Run => unreachable!(),
                        },
                        Style::default().fg(crate::colors::text()),
                    ));
                } else {
                    let done = match ctx_path {
                        Some(p) => format!("Ran in {p}"),
                        None => "Ran".to_string(),
                    };
                    lines.push(Line::styled(
                        done,
                        Style::default()
                            .fg(crate::colors::text_bright())
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }
    }

    // Collect any paths referenced by search commands to suppress redundant directory lines
    let search_paths = &meta.search_paths;

    // We'll emit only content lines here; the header above already communicates the action.
    // Use a single leading "└ " for the very first content line, then indent subsequent ones,
    // except when we're showing an inline running status for ExecAction::Run.
    let mut any_content_emitted = false;
    let use_content_connectors = !(matches!(action, ExecAction::Run) && output.is_none());

    // Restrict displayed entries to the primary action for this cell.
    // For the generic "run" header, allow Run/Test/Lint/Format entries.
    let expected_label: Option<&'static str> = match action {
        ExecAction::Read => Some("Read"),
        ExecAction::Search => Some("Search"),
        ExecAction::List => Some("List"),
        ExecAction::Run => None,
    };

    for parsed in parsed_commands.iter() {
        // Produce a logical label and content string without icons
        let (label, content) = match parsed {
            ParsedCommand::Read { name, cmd, .. } => {
                let mut c = name.clone();
                if let Some(ann) = parse_read_line_annotation(cmd) {
                    c = format!("{c} {ann}");
                }
                ("Read".to_string(), c)
            }
            ParsedCommand::ListFiles { cmd: _, path } => match path {
                Some(p) => {
                    if search_paths.contains(p) {
                        (String::new(), String::new()) // suppressed
                    } else {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{p}/")
                        };
                        ("List".to_string(), format!("{display_p}"))
                    }
                }
                None => ("List".to_string(), "./".to_string()),
            },
            ParsedCommand::Search { query, path, cmd } => {
                // Format query for display: unescape backslash-escapes and close common unbalanced delimiters
                let prettify_term = |s: &str| -> String {
                    // General unescape: turn "\X" into "X" for any X
                    let mut out = String::with_capacity(s.len());
                    let mut iter = s.chars();
                    while let Some(ch) = iter.next() {
                        if ch == '\\' {
                            if let Some(next) = iter.next() {
                                out.push(next);
                            } else {
                                out.push('\\');
                            }
                        } else {
                            out.push(ch);
                        }
                    }
                    // Balance parentheses
                    let opens_paren = out.matches("(").count();
                    let closes_paren = out.matches(")").count();
                    for _ in 0..opens_paren.saturating_sub(closes_paren) {
                        out.push(')');
                    }
                    // Balance curly braces
                    let opens_curly = out.matches("{").count();
                    let closes_curly = out.matches("}").count();
                    for _ in 0..opens_curly.saturating_sub(closes_curly) {
                        out.push('}');
                    }
                    out
                };
                let fmt_query = |q: &str| -> String {
                    let mut parts: Vec<String> = q
                        .split('|')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(prettify_term)
                        .collect();
                    match parts.len() {
                        0 => String::new(),
                        1 => parts.remove(0),
                        2 => format!("{} and {}", parts[0], parts[1]),
                        _ => {
                            let last = parts.last().cloned().unwrap_or_default();
                            let head = &parts[..parts.len() - 1];
                            format!("{} and {}", head.join(", "), last)
                        }
                    }
                };
                match (query, path) {
                    (Some(q), Some(p)) => {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{p}/")
                        };
                        (
                            "Search".to_string(),
                            format!("{} in {}", fmt_query(q), display_p),
                        )
                    }
                    (Some(q), None) => ("Search".to_string(), format!("{}", fmt_query(q))),
                    (None, Some(p)) => {
                        let display_p = if p.ends_with('/') {
                            p.to_string()
                        } else {
                            format!("{p}/")
                        };
                        ("Search".to_string(), format!(" in {}", display_p))
                    }
                    (None, None) => ("Search".to_string(), cmd.clone()),
                }
            }
            ParsedCommand::ReadCommand { cmd } => ("Run".to_string(), cmd.clone()),
            // Upstream-only variants handled as generic runs in this fork
            ParsedCommand::Unknown { cmd } => {
                let t = cmd.trim();
                let lower = t.to_lowercase();
                if lower.starts_with("echo") && lower.contains("---") {
                    (String::new(), String::new())
                } else {
                    ("Run".to_string(), format_inline_script_for_display(cmd))
                }
            } // ParsedCommand::Noop { .. } => continue,
        };

        // Keep only entries that match the primary action grouping.
        if let Some(exp) = expected_label {
            if label != exp {
                continue;
            }
        } else if !(label == "Run" || label == "Search") {
            continue;
        }

        // Skip suppressed entries
        if label.is_empty() && content.is_empty() {
            continue;
        }

        // Split content into lines and push without repeating the action label
        for line_text in content.lines() {
            if line_text.is_empty() {
                continue;
            }
            let prefix = if !any_content_emitted {
                if suppress_run_header || !use_content_connectors {
                    ""
                } else {
                    "└ "
                }
            } else if suppress_run_header || !use_content_connectors {
                ""
            } else {
                "  "
            };
            let mut spans: Vec<Span<'static>> = Vec::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(
                    prefix,
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }

            match label.as_str() {
                // Highlight searched terms in normal text color; keep connectors/path dim
                "Search" => {
                    let remaining = line_text.to_string();
                    // Split off optional path suffix. Support both " (in ...)" and " in <dir>/" forms.
                    let (terms_part, path_part) = if let Some(idx) = remaining.rfind(" (in ") {
                        (
                            remaining[..idx].to_string(),
                            Some(remaining[idx..].to_string()),
                        )
                    } else if let Some(idx) = remaining.rfind(" in ") {
                        let suffix = &remaining[idx + 1..]; // keep leading space for styling
                        // Heuristic: treat as path if it ends with '/'
                        if suffix.trim_end().ends_with('/') {
                            (
                                remaining[..idx].to_string(),
                                Some(remaining[idx..].to_string()),
                            )
                        } else {
                            (remaining.clone(), None)
                        }
                    } else {
                        (remaining.clone(), None)
                    };
                    // Tokenize terms by ", " and " and " while preserving separators
                    let tmp = terms_part.clone();
                    // First, split by ", "
                    let chunks: Vec<String> = if tmp.contains(", ") {
                        tmp.split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![tmp.clone()]
                    };
                    for (i, chunk) in chunks.iter().enumerate() {
                        if i > 0 {
                            // Add comma separator between items (dim)
                            spans.push(Span::styled(
                                ", ",
                                Style::default().fg(crate::colors::text_dim()),
                            ));
                        }
                        // Within each chunk, if it contains " and ", split into left and right with dimmed " and "
                        if let Some((left, right)) = chunk.rsplit_once(" and ") {
                            if !left.is_empty() {
                                spans.push(Span::styled(
                                    left.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                                spans.push(Span::styled(
                                    " and ",
                                    Style::default().fg(crate::colors::text_dim()),
                                ));
                                spans.push(Span::styled(
                                    right.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                            } else {
                                spans.push(Span::styled(
                                    chunk.to_string(),
                                    Style::default().fg(crate::colors::text()),
                                ));
                            }
                        } else {
                            spans.push(Span::styled(
                                chunk.to_string(),
                                Style::default().fg(crate::colors::text()),
                            ));
                        }
                    }
                    if let Some(p) = path_part {
                        // Dim the entire path portion including the " in " or " (in " prefix
                        spans.push(Span::styled(
                            p,
                            Style::default().fg(crate::colors::text_dim()),
                        ));
                    }
                }
                // Highlight filenames in Read; keep line ranges dim
                "Read" => {
                    if let Some(idx) = line_text.find(" (") {
                        let (fname, rest) = line_text.split_at(idx);
                        spans.push(Span::styled(
                            fname.to_string(),
                            Style::default().fg(crate::colors::text()),
                        ));
                        spans.push(Span::styled(
                            rest.to_string(),
                            Style::default().fg(crate::colors::text_dim()),
                        ));
                    } else {
                        spans.push(Span::styled(
                            line_text.to_string(),
                            Style::default().fg(crate::colors::text()),
                        ));
                    }
                }
                // List: highlight directory names
                "List" => {
                    spans.push(Span::styled(
                        line_text.to_string(),
                        Style::default().fg(crate::colors::text()),
                    ));
                }
                _ => {
                    // For executed commands (Run/Test/Lint/etc.), use shell syntax highlighting.
                    let normalized = normalize_shell_command_display(line_text);
                    let display_line = insert_line_breaks_after_double_ampersand(&normalized);
                    let mut hl =
                        crate::syntax_highlight::highlight_code_block(&display_line, Some("bash"));
                    if let Some(mut first_line) = hl.pop() {
                        emphasize_shell_command_name(&mut first_line);
                        spans.extend(first_line.spans.into_iter());
                    } else {
                        spans.push(Span::styled(
                            display_line,
                            Style::default().fg(crate::colors::text()),
                        ));
                    }
                }
            }

            lines.push(Line::from(spans));
            any_content_emitted = true;
        }
    }

    // If this is a List cell and the loop above produced no content (e.g.,
    // the list path was suppressed because a Search referenced the same path),
    // emit a single contextual line so the location is always visible.
    if matches!(action, ExecAction::List) && !any_content_emitted {
        let display_p = match ctx_path {
            Some(p) if !p.is_empty() => {
                if p.ends_with('/') {
                    p.to_string()
                } else {
                    format!("{p}/")
                }
            }
            _ => "./".to_string(),
        };
        lines.push(Line::from(vec![
            Span::styled("└ ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                format!("{display_p}"),
                Style::default().fg(crate::colors::text()),
            ),
        ]));
        // no-op: avoid unused assignment warning; the variable's value is not consumed later
    }

    // Show stdout for real run commands; keep read/search/list concise unless error
    let show_stdout = matches!(action, ExecAction::Run);
    let use_angle_pipe = show_stdout; // add "> " prefix for run output
    let display_output = output.or(stream_preview);
    let mut preview_lines = output_lines(display_output, !show_stdout, use_angle_pipe);
    if let Some(status_line) = running_status {
        if let Some(last) = preview_lines.last() {
            let is_blank = last
                .spans
                .iter()
                .all(|sp| sp.content.as_ref().trim().is_empty());
            if is_blank {
                preview_lines.pop();
            }
        }
        preview_lines.push(status_line);
    }
    lines.extend(preview_lines);
    lines.push(Line::from(""));
    lines
}

fn new_exec_command_generic(
    command: &[String],
    output: Option<&CommandOutput>,
    stream_preview: Option<&CommandOutput>,
    start_time: Option<Instant>,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let command_escaped = strip_bash_lc_and_escape(command);
    let normalized = normalize_shell_command_display(&command_escaped);
    let command_display = insert_line_breaks_after_double_ampersand(&normalized);
    // Highlight the command as bash and then append a dimmed duration to the
    // first visual line while running.
    let mut highlighted_cmd =
        crate::syntax_highlight::highlight_code_block(&command_display, Some("bash"));

    for (idx, line) in highlighted_cmd.iter_mut().enumerate() {
        emphasize_shell_command_name(line);
        if idx > 0 {
            line.spans.insert(
                0,
                Span::styled("  ", Style::default().fg(crate::colors::text())),
            );
        }
    }

    let render_running_header = output.is_none();
    let display_output = output.or(stream_preview);
    let mut running_status = None;
    if render_running_header {
        let mut message = "Running...".to_string();
        if let Some(start) = start_time {
            let elapsed = start.elapsed();
            message = format!("{message} ({})", format_duration(elapsed));
        }
        running_status = Some(running_status_line(message));
    }

    if output.is_some() {
        for line in highlighted_cmd.iter_mut() {
            for span in line.spans.iter_mut() {
                span.style = span.style.fg(crate::colors::text_bright());
            }
        }
    }

    lines.extend(highlighted_cmd);

    let mut preview_lines = output_lines(display_output, false, true);
    if let Some(status_line) = running_status {
        if let Some(last) = preview_lines.last() {
            let is_blank = last
                .spans
                .iter()
                .all(|sp| sp.content.as_ref().trim().is_empty());
            if is_blank {
                preview_lines.pop();
            }
        }
        preview_lines.push(status_line);
    }

    lines.extend(preview_lines);
    lines
}

#[allow(dead_code)]
pub(crate) fn new_active_mcp_tool_call(invocation: McpInvocation) -> ToolCallCell {
    let invocation_line = format_mcp_invocation(invocation);
    let invocation_text = line_to_plain_text(&invocation_line);
    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status: HistoryToolStatus::Running,
        title: "Working".to_string(),
        duration: None,
        arguments: vec![ToolArgument {
            name: "invocation".to_string(),
            value: ArgumentValue::Text(invocation_text),
        }],
        result_preview: None,
        error_message: None,
    };
    ToolCallCell::new(state)
}

#[allow(dead_code)]
pub(crate) fn new_active_custom_tool_call(tool_name: String, args: Option<String>) -> ToolCallCell {
    let invocation_str = if let Some(args) = args {
        format!("{}({})", tool_name, args)
    } else {
        format!("{}()", tool_name)
    };
    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status: HistoryToolStatus::Running,
        title: "Working".to_string(),
        duration: None,
        arguments: vec![ToolArgument {
            name: "invocation".to_string(),
            value: ArgumentValue::Text(invocation_str),
        }],
        result_preview: None,
        error_message: None,
    };
    ToolCallCell::new(state)
}

// Friendly present-participle titles for running browser tools
fn browser_running_title(tool_name: &str) -> &'static str {
    match tool_name {
        "browser_click" => "Clicking...",
        "browser_type" => "Typing...",
        "browser_key" => "Sending key...",
        "browser_javascript" => "Running JavaScript...",
        "browser_scroll" => "Scrolling...",
        "browser_open" => "Opening...",
        "browser_close" => "Closing...",
        "browser_status" => "Checking status...",
        "browser_history" => "Navigating...",
        "browser_inspect" => "Inspecting...",
        "browser_console" => "Reading console...",
        "browser_move" => "Moving...",
        _ => "Working...",
    }
}

fn argument_value_from_json(value: &serde_json::Value) -> ArgumentValue {
    match value {
        serde_json::Value::String(s) => ArgumentValue::Text(s.clone()),
        serde_json::Value::Number(n) => ArgumentValue::Text(n.to_string()),
        serde_json::Value::Bool(b) => ArgumentValue::Text(b.to_string()),
        serde_json::Value::Null => ArgumentValue::Text("null".to_string()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            ArgumentValue::Json(value.clone())
        }
    }
}

fn arguments_from_json(value: &serde_json::Value) -> Vec<ToolArgument> {
    arguments_from_json_excluding(value, &[])
}

fn arguments_from_json_excluding(
    value: &serde_json::Value,
    exclude: &[&str],
) -> Vec<ToolArgument> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .filter(|(key, _)| !exclude.contains(&key.as_str()))
            .map(|(key, val)| ToolArgument {
                name: key.clone(),
                value: argument_value_from_json(val),
            })
            .collect(),
        serde_json::Value::Array(items) => vec![ToolArgument {
            name: "items".to_string(),
            value: ArgumentValue::Json(serde_json::Value::Array(items.clone())),
        }],
        other => vec![ToolArgument {
            name: "args".to_string(),
            value: argument_value_from_json(other),
        }],
    }
}

pub(crate) fn new_running_browser_tool_call(
    tool_name: String,
    args: Option<String>,
) -> RunningToolCallCell {
    // Parse args JSON and use compact humanized form when possible
    let mut arguments: Vec<ToolArgument> = Vec::new();
    if let Some(args_str) = args {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&args_str) {
            if let Some(lines) = format_browser_args_humanized(&tool_name, &json) {
                let summary = lines_to_plain_text(&lines);
                if !summary.is_empty() {
                    arguments.push(ToolArgument {
                        name: "summary".to_string(),
                        value: ArgumentValue::Text(summary),
                    });
                }
            }
            let mut kv_args = arguments_from_json(&json);
            arguments.append(&mut kv_args);
        }
    }
    let state = RunningToolState {
        id: HistoryId::ZERO,
        call_id: None,
        title: browser_running_title(&tool_name).to_string(),
        started_at: SystemTime::now(),
        arguments,
        wait_has_target: false,
        wait_has_call_id: false,
        wait_cap_ms: None,
    };
    RunningToolCallCell::new(state)
}

fn custom_tool_running_title(tool_name: &str) -> String {
    if tool_name == "wait" {
        return "Waiting".to_string();
    }
    if tool_name.starts_with("agent_") || tool_name == "agent" {
        // Reuse agent title and append ellipsis
        format!("{}...", agent_tool_title(tool_name, None))
    } else if tool_name.starts_with("browser_") {
        browser_running_title(tool_name).to_string()
    } else {
        // TitleCase from snake_case and append ellipsis
        let pretty = tool_name
            .split('_')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    Some(f) => format!("{}{}", f.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("{}...", pretty)
    }
}

pub(crate) fn new_running_custom_tool_call(
    tool_name: String,
    args: Option<String>,
) -> RunningToolCallCell {
    // Parse args JSON and format as structured key/value arguments
    let mut arguments: Vec<ToolArgument> = Vec::new();
    let mut wait_has_target = false;
    let mut wait_has_call_id = false;
    let mut wait_cap_ms = None;
    if let Some(args_str) = args {
        match serde_json::from_str::<serde_json::Value>(&args_str) {
            Ok(json) => {
                if tool_name == "wait" {
                    wait_cap_ms = json.get("timeout_ms").and_then(|v| v.as_u64());
                    if let Some(for_what) = json.get("for").and_then(|v| v.as_str()) {
                        let cleaned = clean_wait_command(for_what);
                        arguments.push(ToolArgument {
                            name: "for".to_string(),
                            value: ArgumentValue::Text(cleaned),
                        });
                        wait_has_target = true;
                    }
                    if let Some(cid) = json.get("call_id").and_then(|v| v.as_str()) {
                        arguments.push(ToolArgument {
                            name: "call_id".to_string(),
                            value: ArgumentValue::Text(cid.to_string()),
                        });
                        wait_has_call_id = true;
                    }
                    let mut remaining = json.clone();
                    if let serde_json::Value::Object(ref mut map) = remaining {
                        map.remove("for");
                        map.remove("call_id");
                        map.remove("timeout_ms");
                    }
                    let mut others = arguments_from_json(&remaining);
                    arguments.append(&mut others);
                } else {
                    let mut kv_args = arguments_from_json(&json);
                    arguments.append(&mut kv_args);
                }
            }
            Err(_) => {
                arguments.push(ToolArgument {
                    name: "args".to_string(),
                    value: ArgumentValue::Text(args_str.clone()),
                });
            }
        }
    }
    let state = RunningToolState {
        id: HistoryId::ZERO,
        call_id: None,
        title: custom_tool_running_title(&tool_name),
        started_at: SystemTime::now(),
        arguments,
        wait_has_target,
        wait_has_call_id,
        wait_cap_ms,
    };
    RunningToolCallCell::new(state)
}

pub(crate) fn new_running_mcp_tool_call(invocation: McpInvocation) -> RunningToolCallCell {
    // Represent as provider.tool(...) on one dim line beneath a generic running header with timer
    let line = format_mcp_invocation(invocation);
    let invocation_text = line_to_plain_text(&line);
    let state = RunningToolState {
        id: HistoryId::ZERO,
        call_id: None,
        title: "Working...".to_string(),
        started_at: SystemTime::now(),
        arguments: vec![ToolArgument {
            name: "invocation".to_string(),
            value: ArgumentValue::Text(invocation_text),
        }],
        wait_has_target: false,
        wait_has_call_id: false,
        wait_cap_ms: None,
    };
    RunningToolCallCell::new(state)
}

pub(crate) fn new_completed_custom_tool_call(
    tool_name: String,
    args: Option<String>,
    duration: Duration,
    success: bool,
    result: String,
) -> ToolCallCell {
    // Special rendering for browser_* tools
    if tool_name.starts_with("browser_") {
        return new_completed_browser_tool_call(tool_name, args, duration, success, result);
    }
    // Special rendering for agent tools
    if tool_name.starts_with("agent_") || tool_name == "agent" {
        return new_completed_agent_tool_call(tool_name, args, duration, success, result);
    }
    let status = if success {
        HistoryToolStatus::Success
    } else {
        HistoryToolStatus::Failed
    };
    let status_title = if success { "Complete" } else { "Error" };
    let invocation_str = if let Some(args) = args.clone() {
        format!("{}({})", tool_name, args)
    } else {
        format!("{}()", tool_name)
    };

    let mut arguments = vec![ToolArgument {
        name: "invocation".to_string(),
        value: ArgumentValue::Text(invocation_str),
    }];

    if let Some(args_str) = args {
        match serde_json::from_str::<serde_json::Value>(&args_str) {
            Ok(json) => {
                let mut parsed = arguments_from_json(&json);
                arguments.append(&mut parsed);
            }
            Err(_) => {
                if !args_str.is_empty() {
                    arguments.push(ToolArgument {
                        name: "args".to_string(),
                        value: ArgumentValue::Text(args_str),
                    });
                }
            }
        }
    }

    let result_preview = if result.is_empty() {
        None
    } else {
        let preview_lines = build_preview_lines(&result, true);
        let preview_strings = preview_lines
            .iter()
            .map(line_to_plain_text)
            .collect::<Vec<_>>();
        Some(ToolResultPreview {
            lines: preview_strings,
            truncated: false,
        })
    };

    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status,
        title: status_title.to_string(),
        duration: Some(duration),
        arguments,
        result_preview,
        error_message: None,
    };
    ToolCallCell::new(state)
}

/// Completed web_fetch tool call with markdown rendering of the `markdown` field.
// Web fetch preview sizing: show 10 lines at the start and 5 at the end.
const WEB_FETCH_HEAD_LINES: usize = 10;
const WEB_FETCH_TAIL_LINES: usize = 5;

pub(crate) fn new_completed_web_fetch_tool_call(
    cfg: &Config,
    args: Option<String>,
    duration: Duration,
    success: bool,
    result: String,
) -> WebFetchToolCell {
    let duration = format_duration(duration);
    let status_str = if success { "Complete" } else { "Error" };
    let title_line = if success {
        Line::from(vec![
            Span::styled(status_str, Style::default().fg(crate::colors::success())),
            format!(", duration: {duration}").dim(),
        ])
    } else {
        Line::from(vec![
            Span::styled(status_str, Style::default().fg(crate::colors::error())),
            format!(", duration: {duration}").dim(),
        ])
    };

    let invocation_str = if let Some(args) = args {
        format!("{}({})", "web_fetch", args)
    } else {
        format!("{}()", "web_fetch")
    };

    // Header/preamble (no border)
    let mut pre_lines: Vec<Line<'static>> = Vec::new();
    pre_lines.push(title_line);
    pre_lines.push(Line::styled(
        invocation_str,
        Style::default()
            .fg(crate::colors::text_dim())
            .add_modifier(Modifier::ITALIC),
    ));

    // Try to parse JSON and extract the markdown field
    let mut appended_markdown = false;
    let mut body_lines: Vec<Line<'static>> = Vec::new();
    if !result.is_empty() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&result) {
            if let Some(md) = value.get("markdown").and_then(|v| v.as_str()) {
                // Build a smarter sectioned preview from the raw markdown.
                let mut sect = build_web_fetch_sectioned_preview(md, cfg);
                dim_webfetch_emphasis_and_links(&mut sect);
                body_lines.extend(sect);
                appended_markdown = true;
            }
        }
    }

    // Fallback: compact preview if JSON parse failed or no markdown present
    if !appended_markdown && !result.is_empty() {
        // Fallback to plain text/JSON preview with ANSI preserved.
        let mut pv =
            select_preview_from_plain_text(&result, WEB_FETCH_HEAD_LINES, WEB_FETCH_TAIL_LINES);
        dim_webfetch_emphasis_and_links(&mut pv);
        body_lines.extend(pv);
    }

    // Spacer below header and below body to match exec styling
    pre_lines.push(Line::from(""));
    if !body_lines.is_empty() {
        body_lines.push(Line::from(""));
    }

    WebFetchToolCell {
        pre_lines,
        body_lines,
        state: if success {
            ToolCellStatus::Success
        } else {
            ToolCellStatus::Failed
        },
    }
}

// Helper: choose first `head` and last `tail` non-empty lines from a styled line list
fn select_preview_from_lines(
    lines: &[Line<'static>],
    head: usize,
    tail: usize,
) -> Vec<Line<'static>> {
    fn is_non_empty(l: &Line<'_>) -> bool {
        let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
        !s.trim().is_empty()
    }
    let non_empty_idx: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| if is_non_empty(l) { Some(i) } else { None })
        .collect();
    if non_empty_idx.len() <= head + tail {
        return lines.to_vec();
    }
    let mut out: Vec<Line<'static>> = Vec::new();
    for &i in non_empty_idx.iter().take(head) {
        out.push(lines[i].clone());
    }
    out.push(Line::from("⋮".dim()));
    for &i in non_empty_idx
        .iter()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .iter()
        .rev()
    {
        out.push(lines[*i].clone());
    }
    out
}

// Helper: like build_preview_lines but parameterized and preserving ANSI
fn select_preview_from_plain_text(text: &str, head: usize, tail: usize) -> Vec<Line<'static>> {
    let processed = format_json_compact(text).unwrap_or_else(|| text.to_string());
    let processed = normalize_overwrite_sequences(&processed);
    let processed = sanitize_for_tui(
        &processed,
        SanitizeMode::AnsiPreserving,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    let non_empty: Vec<&str> = processed.lines().filter(|line| !line.is_empty()).collect();
    fn ansi_line_with_theme_bg(s: &str) -> Line<'static> {
        let mut ln = ansi_escape_line(s);
        for sp in ln.spans.iter_mut() {
            sp.style.bg = None;
        }
        ln
    }
    let mut out: Vec<Line<'static>> = Vec::new();
    if non_empty.len() <= head + tail {
        for s in non_empty {
            out.push(ansi_line_with_theme_bg(s));
        }
        return out;
    }
    for s in non_empty.iter().take(head) {
        out.push(ansi_line_with_theme_bg(s));
    }
    out.push(Line::from("⋮".dim()));
    let start = non_empty.len().saturating_sub(tail);
    for s in &non_empty[start..] {
        out.push(ansi_line_with_theme_bg(s));
    }
    out
}

// ==================== WebFetchToolCell ====================

pub(crate) struct WebFetchToolCell {
    pre_lines: Vec<Line<'static>>,  // header/invocation
    body_lines: Vec<Line<'static>>, // bordered, dim preview
    state: ToolCellStatus,
}

impl HistoryCell for WebFetchToolCell {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn kind(&self) -> HistoryCellType {
        HistoryCellType::Tool { status: self.state }
    }
    fn display_lines(&self) -> Vec<Line<'static>> {
        // Fallback textual representation used only for measurement outside custom render
        let mut v = Vec::new();
        v.extend(self.pre_lines.clone());
        v.extend(self.body_lines.clone());
        v
    }
    fn has_custom_render(&self) -> bool {
        true
    }
    fn desired_height(&self, width: u16) -> u16 {
        let pre_text = Text::from(trim_empty_lines(self.pre_lines.clone()));
        let body_text = Text::from(trim_empty_lines(self.body_lines.clone()));
        let pre_total: u16 = Paragraph::new(pre_text)
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0);
        let body_total: u16 = Paragraph::new(body_text)
            .wrap(Wrap { trim: false })
            .line_count(width.saturating_sub(2))
            .try_into()
            .unwrap_or(0);
        pre_total.saturating_add(body_total)
    }
    fn custom_render_with_skip(&self, area: Rect, buf: &mut Buffer, skip_rows: u16) {
        // Measure with the same widths we will render with.
        let pre_text = Text::from(trim_empty_lines(self.pre_lines.clone()));
        let body_text = Text::from(trim_empty_lines(self.body_lines.clone()));
        let pre_wrap_width = area.width;
        let body_wrap_width = area.width.saturating_sub(2);
        let pre_total: u16 = Paragraph::new(pre_text.clone())
            .wrap(Wrap { trim: false })
            .line_count(pre_wrap_width)
            .try_into()
            .unwrap_or(0);
        let body_total: u16 = Paragraph::new(body_text.clone())
            .wrap(Wrap { trim: false })
            .line_count(body_wrap_width)
            .try_into()
            .unwrap_or(0);

        let pre_skip = skip_rows.min(pre_total);
        let body_skip = skip_rows.saturating_sub(pre_total).min(body_total);

        let pre_remaining = pre_total.saturating_sub(pre_skip);
        let pre_height = pre_remaining.min(area.height);
        let body_available = area.height.saturating_sub(pre_height);
        let body_remaining = body_total.saturating_sub(body_skip);
        let body_height = body_available.min(body_remaining);

        // Render preamble
        if pre_height > 0 {
            let pre_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: pre_height,
            };
            let bg_style = Style::default()
                .bg(crate::colors::background())
                .fg(crate::colors::text());
            fill_rect(buf, pre_area, Some(' '), bg_style);
            let pre_block =
                Block::default().style(Style::default().bg(crate::colors::background()));
            Paragraph::new(pre_text)
                .block(pre_block)
                .wrap(Wrap { trim: false })
                .scroll((pre_skip, 0))
                .style(Style::default().bg(crate::colors::background()))
                .render(pre_area, buf);
        }

        // Render body with left border + dim text
        if body_height > 0 {
            let body_area = Rect {
                x: area.x,
                y: area.y.saturating_add(pre_height),
                width: area.width,
                height: body_height,
            };
            let bg_style = Style::default()
                .bg(crate::colors::background())
                .fg(crate::colors::text_dim());
            fill_rect(buf, body_area, Some(' '), bg_style);
            let block = Block::default()
                .borders(Borders::LEFT)
                .border_style(
                    Style::default()
                        .fg(crate::colors::border_dim())
                        .bg(crate::colors::background()),
                )
                .style(Style::default().bg(crate::colors::background()))
                .padding(Padding {
                    left: 1,
                    right: 0,
                    top: 0,
                    bottom: 0,
                });
            Paragraph::new(body_text)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((body_skip, 0))
                .style(
                    Style::default()
                        .bg(crate::colors::background())
                        .fg(crate::colors::text_dim()),
                )
                .render(body_area, buf);
        }
    }
}

// Build sectioned preview for web_fetch markdown:
// - First 2 non-empty lines
// - Up to 5 sections: a heading line (starts with #) plus the next 4 lines
// - Last 2 non-empty lines
// Ellipses (⋮) are inserted between groups. All content is rendered as markdown.
fn build_web_fetch_sectioned_preview(md: &str, cfg: &Config) -> Vec<Line<'static>> {
    let lines: Vec<&str> = md.lines().collect();

    // Collect first 1 and last 1 non-empty lines (by raw markdown lines)
    let first_non_empty: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| if l.trim().is_empty() { None } else { Some(i) })
        .take(1)
        .collect();
    let last_non_empty_rev: Vec<usize> = lines
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, l)| if l.trim().is_empty() { None } else { Some(i) })
        .take(1)
        .collect();
    let mut last_non_empty = last_non_empty_rev.clone();
    last_non_empty.reverse();

    // Find up to 5 heading indices outside code fences
    let mut in_code = false;
    let mut section_heads: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < lines.len() && section_heads.len() < 5 {
        let l = lines[i];
        let trimmed = l.trim_start();
        // Toggle code fence state
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if !in_code {
            // Heading: 1-6 leading # followed by a space
            let mut level = 0usize;
            for ch in trimmed.chars() {
                if ch == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            if level >= 1 && level <= 6 {
                if trimmed.chars().nth(level).map_or(false, |c| c == ' ') {
                    section_heads.push(i);
                }
            }
        }
        i += 1;
    }

    // Helper to render a slice of raw markdown lines
    let render_slice = |start: usize, end_excl: usize, out: &mut Vec<Line<'static>>| {
        if start >= end_excl || start >= lines.len() {
            return;
        }
        let end = end_excl.min(lines.len());
        let segment = lines[start..end].join("\n");
        let mut seg_lines: Vec<Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&segment, &mut seg_lines, cfg);
        // Trim leading/trailing empties per segment to keep things tight
        out.extend(trim_empty_lines(seg_lines));
    };

    let mut out: Vec<Line<'static>> = Vec::new();

    // First 2 lines
    if !first_non_empty.is_empty() {
        let start = first_non_empty[0];
        let end = first_non_empty
            .last()
            .copied()
            .unwrap_or(start)
            .saturating_add(1);
        render_slice(start, end, &mut out);
    }

    // Sections
    if !section_heads.is_empty() {
        if !out.is_empty() {
            out.push(Line::from("⋮".dim()));
        }
        for (idx, &h) in section_heads.iter().enumerate() {
            // heading + next 4 lines (total up to 5)
            let end = (h + 5).min(lines.len());
            render_slice(h, end, &mut out);
            if idx + 1 < section_heads.len() {
                out.push(Line::from("⋮".dim()));
            }
        }
    }

    // Last 2 lines
    if !last_non_empty.is_empty() {
        // Avoid duplicating lines if they overlap with earlier content
        let last_start = *last_non_empty.first().unwrap_or(&0);
        if !out.is_empty() {
            out.push(Line::from("⋮".dim()));
        }
        let last_end = last_non_empty
            .last()
            .copied()
            .unwrap_or(last_start)
            .saturating_add(1);
        render_slice(last_start, last_end, &mut out);
    }

    if out.is_empty() {
        // Fallback: if nothing matched, show head/tail preview
        let mut all_md_lines: Vec<Line<'static>> = Vec::new();
        crate::markdown::append_markdown(md, &mut all_md_lines, cfg);
        return select_preview_from_lines(
            &all_md_lines,
            WEB_FETCH_HEAD_LINES,
            WEB_FETCH_TAIL_LINES,
        );
    }

    out
}

// Post-process rendered markdown lines to dim emphasis, lists, and links for web_fetch only.
fn dim_webfetch_emphasis_and_links(lines: &mut Vec<Line<'static>>) {
    use ratatui::style::Modifier;
    let text_dim = crate::colors::text_dim();
    let code_bg = crate::colors::code_block_bg();
    // Recompute the link color logic used by the markdown renderer to detect link spans
    let link_fg = crate::colors::mix_toward(crate::colors::text(), crate::colors::primary(), 0.35);
    for line in lines.iter_mut() {
        // Heuristic list detection on the plain text form
        let s: String = line.spans.iter().map(|sp| sp.content.as_ref()).collect();
        let t = s.trim_start();
        let is_list = t.starts_with('-')
            || t.starts_with('*')
            || t.starts_with('+')
            || t.starts_with('•')
            || t.starts_with('·')
            || t.starts_with('⋅')
            || t.chars().take_while(|c| c.is_ascii_digit()).count() > 0
                && (t.chars().skip_while(|c| c.is_ascii_digit()).next() == Some('.')
                    || t.chars().skip_while(|c| c.is_ascii_digit()).next() == Some(')'));

        for sp in line.spans.iter_mut() {
            // Skip code block spans (have a solid code background)
            if sp.style.bg == Some(code_bg) {
                continue;
            }
            let style = &mut sp.style;
            let is_bold = style.add_modifier.contains(Modifier::BOLD);
            let is_under = style.add_modifier.contains(Modifier::UNDERLINED);
            let is_link_colored = style.fg == Some(link_fg);
            if is_list || is_bold || is_under || is_link_colored {
                style.fg = Some(text_dim);
            }
        }
    }
}

// Map `browser_*` tool names to friendly titles
fn browser_tool_title(tool_name: &str) -> &'static str {
    match tool_name {
        "browser_click" => "Browser Click",
        "browser_type" => "Browser Type",
        "browser_key" => "Browser Key",
        "browser_javascript" => "Browser JavaScript",
        "browser_scroll" => "Browser Scroll",
        "browser_open" => "Browser Open",
        "browser_close" => "Browser Close",
        "browser_fetch" => "Browser Fetch",
        "browser_status" => "Browser Status",
        "browser_history" => "Browser History",
        "browser_inspect" => "Browser Inspect",
        "browser_console" => "Browser Console",
        "browser_cdp" => "Browser CDP",
        "browser_move" => "Browser Move",
        _ => "Browser Tool",
    }
}

fn lines_to_plain_text(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(line_to_plain_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_to_plain_text(line: &Line<'_>) -> String {
    line
        .spans
        .iter()
        .map(|sp| sp.content.as_ref())
        .collect::<String>()
}

// Attempt a compact, humanized one-line summary for browser tools.
// Returns Some(lines) when a concise form is available for the given tool, else None.
fn format_browser_args_humanized(
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<Vec<Line<'static>>> {
    use serde_json::Value;
    let text = |s: String| Span::styled(s, Style::default().fg(crate::colors::text()));

    // Helper: format coordinate pair as integers (pixels)
    let fmt_xy = |x: f64, y: f64| -> String {
        let xi = x.round() as i64;
        let yi = y.round() as i64;
        format!("({xi}, {yi})")
    };

    match (tool_name, args) {
        ("browser_click", Value::Object(map)) => {
            // Expect optional `type`, and x/y for absolute. Only compact when both x and y provided.
            let ty = map
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("click")
                .to_lowercase();
            let (x, y) = match (
                map.get("x").and_then(|v| v.as_f64()),
                map.get("y").and_then(|v| v.as_f64()),
            ) {
                (Some(x), Some(y)) => (x, y),
                _ => return None,
            };
            let msg = format!("└ {ty} at {}", fmt_xy(x, y));
            Some(vec![Line::from(text(msg))])
        }
        ("browser_fetch", Value::Object(map)) => {
            if let Some(url) = map.get("url").and_then(|v| v.as_str()) {
                let msg = format!("└ fetch {}", url);
                Some(vec![Line::from(text(msg))])
            } else {
                None
            }
        }
        ("browser_move", Value::Object(map)) => {
            // Prefer absolute x/y → "to (x, y)"; otherwise relative dx/dy → "by (dx, dy)".
            if let (Some(x), Some(y)) = (
                map.get("x").and_then(|v| v.as_f64()),
                map.get("y").and_then(|v| v.as_f64()),
            ) {
                let msg = format!("└ to {}", fmt_xy(x, y));
                return Some(vec![Line::from(text(msg))]);
            }
            if let (Some(dx), Some(dy)) = (
                map.get("dx").and_then(|v| v.as_f64()),
                map.get("dy").and_then(|v| v.as_f64()),
            ) {
                let msg = format!("└ by {}", fmt_xy(dx, dy));
                return Some(vec![Line::from(text(msg))]);
            }
            None
        }
        _ => None,
    }
}

fn new_completed_browser_tool_call(
    tool_name: String,
    args: Option<String>,
    duration: Duration,
    success: bool,
    result: String,
) -> ToolCallCell {
    let status = if success {
        HistoryToolStatus::Success
    } else {
        HistoryToolStatus::Failed
    };
    let mut arguments: Vec<ToolArgument> = Vec::new();
    if let Some(args_str) = args {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&args_str) {
            if let Some(lines) = format_browser_args_humanized(&tool_name, &json) {
                let summary = lines_to_plain_text(&lines);
                if !summary.is_empty() {
                    arguments.push(ToolArgument {
                        name: "summary".to_string(),
                        value: ArgumentValue::Text(summary),
                    });
                }
            }
            let mut kv = arguments_from_json(&json);
            arguments.append(&mut kv);
        } else if !args_str.is_empty() {
            arguments.push(ToolArgument {
                name: "args".to_string(),
                value: ArgumentValue::Text(args_str),
            });
        }
    }

    let result_preview = if result.is_empty() {
        None
    } else {
        let preview_lines = build_preview_lines(&result, true);
        let preview_strings = preview_lines
            .iter()
            .map(line_to_plain_text)
            .collect::<Vec<_>>();
        Some(ToolResultPreview {
            lines: preview_strings,
            truncated: false,
        })
    };

    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status,
        title: browser_tool_title(&tool_name).to_string(),
        duration: Some(duration),
        arguments,
        result_preview,
        error_message: None,
    };
    ToolCallCell::new(state)
}

// Map `agent_*` tool names to friendly titles
fn agent_tool_title(tool_name: &str, action: Option<&str>) -> String {
    let key = action.unwrap_or_else(|| match tool_name {
        "agent_run" => "create",
        "agent_wait" => "wait",
        "agent_result" => "result",
        "agent_cancel" => "cancel",
        "agent_check" => "status",
        "agent_list" => "list",
        "agent" => "create",
        other => other,
    });

    match key {
        "create" | "agent" => "Agent Run".to_string(),
        "wait" | "agent_wait" => "Agent Wait".to_string(),
        "result" | "agent_result" => "Agent Result".to_string(),
        "cancel" | "agent_cancel" => "Agent Cancel".to_string(),
        "status" | "agent_check" | "agent_status" => "Agent Status".to_string(),
        "list" | "agent_list" => "Agent List".to_string(),
        other => {
            if let Some(rest) = other.strip_prefix("agent_") {
                let title = rest
                    .split('_')
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        let mut chars = s.chars();
                        match chars.next() {
                            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("Agent {}", title)
            } else {
                "Agent Tool".to_string()
            }
        }
    }
}

fn new_completed_agent_tool_call(
    tool_name: String,
    args: Option<String>,
    duration: Duration,
    success: bool,
    result: String,
) -> ToolCallCell {
    let status = if success {
        HistoryToolStatus::Success
    } else {
        HistoryToolStatus::Failed
    };
    let mut arguments: Vec<ToolArgument> = Vec::new();
    let mut action: Option<String> = None;
    if let Some(args_str) = args {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&args_str) {
            if let Some(act) = json.get("action").and_then(|v| v.as_str()) {
                action = Some(act.to_string());
            }
            let mut kv = arguments_from_json(&json);
            arguments.append(&mut kv);
        } else if !args_str.is_empty() {
            arguments.push(ToolArgument {
                name: "args".to_string(),
                value: ArgumentValue::Text(args_str),
            });
        }
    }

    let result_preview = if result.is_empty() {
        None
    } else {
        let preview_lines = build_preview_lines(&result, true);
        let preview_strings = preview_lines
            .iter()
            .map(line_to_plain_text)
            .collect::<Vec<_>>();
        Some(ToolResultPreview {
            lines: preview_strings,
            truncated: false,
        })
    };

    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status,
        title: agent_tool_title(&tool_name, action.as_deref()),
        duration: Some(duration),
        arguments,
        result_preview,
        error_message: None,
    };
    ToolCallCell::new(state)
}

// Try to create an image cell if the MCP result contains an image
fn try_new_completed_mcp_tool_call_with_image_output(
    result: &Result<mcp_types::CallToolResult, String>,
) -> Option<ImageOutputCell> {
    match result {
        Ok(mcp_types::CallToolResult { content, .. }) => {
            if let Some(mcp_types::ContentBlock::ImageContent(image_block)) = content.first() {
                let raw_data = match base64::engine::general_purpose::STANDARD
                    .decode(&image_block.data)
                {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to decode image data: {e}");
                        return None;
                    }
                };
                let reader = match ImageReader::new(Cursor::new(&raw_data)).with_guessed_format() {
                    Ok(reader) => reader,
                    Err(e) => {
                        error!("Failed to guess image format: {e}");
                        return None;
                    }
                };

                let decoded = match reader.decode() {
                    Ok(image) => image,
                    Err(e) => {
                        error!("Image decoding failed: {e}");
                        return None;
                    }
                };

                let width = decoded.width().min(u16::MAX as u32) as u16;
                let height = decoded.height().min(u16::MAX as u32) as u16;
                let sha_hex = format!("{:x}", Sha256::digest(&raw_data));
                let byte_len = raw_data.len().min(u32::MAX as usize) as u32;

                let record = ImageRecord {
                    id: HistoryId::ZERO,
                    source_path: None,
                    alt_text: None,
                    width,
                    height,
                    sha256: Some(sha_hex),
                    mime_type: Some(image_block.mime_type.clone()),
                    byte_len: Some(byte_len),
                };

                Some(ImageOutputCell::from_record(record))
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn new_completed_mcp_tool_call(
    _num_cols: usize,
    invocation: McpInvocation,
    duration: Duration,
    success: bool,
    result: Result<mcp_types::CallToolResult, String>,
) -> Box<dyn HistoryCell> {
    if let Some(cell) = try_new_completed_mcp_tool_call_with_image_output(&result) {
        return Box::new(cell);
    }

    let status = if success {
        HistoryToolStatus::Success
    } else {
        HistoryToolStatus::Failed
    };

    let invocation_line = format_mcp_invocation(invocation);
    let invocation_text = line_to_plain_text(&invocation_line);
    let arguments = vec![ToolArgument {
        name: "invocation".to_string(),
        value: ArgumentValue::Text(invocation_text),
    }];

    let mut preview_lines: Vec<String> = Vec::new();
    let mut error_message: Option<String> = None;

    match result {
        Ok(mcp_types::CallToolResult { content, .. }) => {
            for tool_call_result in content {
                match tool_call_result {
                    mcp_types::ContentBlock::TextContent(text) => {
                        let preview = build_preview_lines(&text.text, true);
                        for line in preview {
                            preview_lines.push(line_to_plain_text(&line));
                        }
                        preview_lines.push(String::new());
                    }
                    mcp_types::ContentBlock::ImageContent(_) => {
                        preview_lines.push("<image content>".to_string());
                    }
                    mcp_types::ContentBlock::AudioContent(_) => {
                        preview_lines.push("<audio content>".to_string());
                    }
                    mcp_types::ContentBlock::EmbeddedResource(resource) => {
                        let uri = match resource.resource {
                            EmbeddedResourceResource::TextResourceContents(text) => text.uri,
                            EmbeddedResourceResource::BlobResourceContents(blob) => blob.uri,
                        };
                        preview_lines.push(format!("embedded resource: {uri}"));
                    }
                    mcp_types::ContentBlock::ResourceLink(ResourceLink { uri, .. }) => {
                        preview_lines.push(format!("link: {uri}"));
                    }
                }
            }
            if preview_lines.last().map(|s| !s.is_empty()).unwrap_or(false) {
                preview_lines.push(String::new());
            }
        }
        Err(e) => {
            error_message = Some(format!("Error: {e}"));
        }
    }

    let result_preview = if preview_lines.is_empty() {
        None
    } else {
        Some(ToolResultPreview {
            lines: preview_lines,
            truncated: false,
        })
    };

    let state = ToolCallState {
        id: HistoryId::ZERO,
        call_id: None,
        status,
        title: if success { "Complete" } else { "Error" }.to_string(),
        duration: Some(duration),
        arguments,
        result_preview,
        error_message,
    };

    Box::new(ToolCallCell::new(state))
}

pub(crate) fn new_error_event(message: String) -> PlainMessageState {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::styled(
        "error",
        Style::default()
            .fg(crate::colors::error())
            .add_modifier(Modifier::BOLD),
    ));
    let msg_norm = normalize_overwrite_sequences(&message);
    lines.extend(
        msg_norm
            .lines()
            .map(|line| ansi_escape_line(line).style(Style::default().fg(crate::colors::error()))),
    );
    // No empty line at end - trimming and spacing handled by renderer
    plain_message_state_from_lines(lines, HistoryCellType::Error)
}

#[allow(dead_code)]
pub(crate) fn new_diff_output(diff_output: String) -> DiffCell {
    new_diff_cell_from_string(diff_output)
}

pub(crate) fn new_reasoning_output(reasoning_effort: &ReasoningEffort) -> PlainMessageState {
    let lines = vec![
        Line::from(""),
        Line::from("Reasoning Effort")
            .fg(crate::colors::keyword())
            .bold(),
        Line::from(format!("Value: {}", reasoning_effort)),
    ];
    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

pub(crate) fn new_model_output(model: &str, effort: ReasoningEffort) -> PlainMessageState {
    let lines = vec![
        Line::from(""),
        Line::from("Model Selection")
            .fg(crate::colors::keyword())
            .bold(),
        Line::from(format!("Model: {}", model)),
        Line::from(format!("Reasoning Effort: {}", effort)),
    ];
    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

// Continue with more factory functions...
// I'll add the rest in the next part to keep this manageable
pub(crate) fn new_status_output(
    config: &Config,
    total_usage: &TokenUsage,
    last_usage: &TokenUsage,
) -> PlainMessageState {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from("/status").fg(crate::colors::keyword()));
    lines.push(Line::from(""));

    // 🔧 Configuration
    lines.push(Line::from(vec!["🔧 ".into(), "Configuration".bold()]));

    // Prepare config summary with custom prettification
    let summary_entries = create_config_summary_entries(config);
    let summary_map: HashMap<String, String> = summary_entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();

    let lookup = |key: &str| -> String { summary_map.get(key).unwrap_or(&String::new()).clone() };
    let title_case = |s: &str| -> String {
        s.split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    // Format model name with proper capitalization
    let formatted_model = if config.model.to_lowercase().starts_with("gpt-") {
        format!("GPT{}", &config.model[3..])
    } else {
        config.model.clone()
    };
    lines.push(Line::from(vec![
        "  • Name: ".into(),
        formatted_model.into(),
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

    // 🔐 Authentication
    lines.push(Line::from(vec!["🔐 ".into(), "Authentication".bold()]));
    {
        use code_login::AuthMode;
        use code_login::CodexAuth;
        use code_login::OPENAI_API_KEY_ENV_VAR;
        use code_login::try_read_auth_json;

        // Determine effective auth mode the core would choose
        let auth_result = CodexAuth::from_code_home(
            &config.code_home,
            AuthMode::ChatGPT,
            &config.responses_originator_header,
        );

        match auth_result {
            Ok(Some(auth)) => match auth.mode {
                AuthMode::ApiKey => {
                    // Prefer suffix from auth.json; fall back to env var if needed
                    let suffix =
                        try_read_auth_json(&code_login::get_auth_file(&config.code_home))
                            .ok()
                            .and_then(|a| a.openai_api_key)
                            .or_else(|| std::env::var(OPENAI_API_KEY_ENV_VAR).ok())
                            .map(|k| key_suffix(&k))
                            .unwrap_or_else(|| "????".to_string());
                    lines.push(Line::from(format!("  • Method: API key (…{suffix})")));
                }
                AuthMode::ChatGPT => {
                    let account_id = auth
                        .get_account_id()
                        .unwrap_or_else(|| "unknown".to_string());
                    lines.push(Line::from(format!(
                        "  • Method: ChatGPT account (account_id: {account_id})"
                    )));
                }
            },
            _ => {
                lines.push(Line::from("  • Method: unauthenticated"));
            }
        }
    }

    lines.push(Line::from(""));

    // 📊 Token Usage
    lines.push(Line::from(vec!["📊 ".into(), "Token Usage".bold()]));
    // Input: <input> [+ <cached> cached]
    let mut input_line_spans: Vec<Span<'static>> = vec![
        "  • Input: ".into(),
        format_with_separators(last_usage.non_cached_input()).into(),
    ];
    if last_usage.cached_input_tokens > 0 {
        input_line_spans.push(
            format!(
                " (+ {} cached)",
                format_with_separators(last_usage.cached_input_tokens)
            )
            .into(),
        );
    }
    lines.push(Line::from(input_line_spans));
    // Output: <output>
    lines.push(Line::from(vec![
        "  • Output: ".into(),
        format_with_separators(last_usage.output_tokens).into(),
    ]));
    // Total: <total>
    lines.push(Line::from(vec![
        "  • Total: ".into(),
        format_with_separators(last_usage.blended_total()).into(),
    ]));
    lines.push(Line::from(vec![
        "  • Session total: ".into(),
        format_with_separators(total_usage.blended_total()).into(),
    ]));

    // 📐 Model Limits
    let context_window = config.model_context_window;
    let max_output_tokens = config.model_max_output_tokens;
    let auto_compact_limit = config.model_auto_compact_token_limit;

    if context_window.is_some() || max_output_tokens.is_some() || auto_compact_limit.is_some() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec!["📐 ".into(), "Model Limits".bold()]));

        if let Some(context_window) = context_window {
            let used = last_usage.tokens_in_context_window().min(context_window);
            let percent_full = if context_window > 0 {
                ((used as f64 / context_window as f64) * 100.0).min(100.0)
            } else {
                0.0
            };
            lines.push(Line::from(format!(
                "  • Context window: {} used of {} ({:.0}% full)",
                format_with_separators(used),
                format_with_separators(context_window),
                percent_full
            )));
        }

        if let Some(max_output_tokens) = max_output_tokens {
            lines.push(Line::from(format!(
                "  • Max output tokens: {}",
                format_with_separators(max_output_tokens)
            )));
        }

        match auto_compact_limit {
            Some(limit) if limit > 0 => {
                let limit_u64 = limit as u64;
                let remaining = limit_u64.saturating_sub(total_usage.total_tokens);
                lines.push(Line::from(format!(
                    "  • Auto-compact threshold: {} ({} remaining)",
                    format_with_separators(limit_u64),
                    format_with_separators(remaining)
                )));
                if total_usage.total_tokens > limit_u64 {
                    lines.push(Line::from("    • Compacting will trigger on the next turn".dim()));
                }
            }
            _ => {
                if let Some(window) = context_window {
                    if window > 0 {
                        let used = last_usage.tokens_in_context_window();
                        let remaining = window.saturating_sub(used);
                        let percent_left = if window == 0 {
                            0.0
                        } else {
                            (remaining as f64 / window as f64) * 100.0
                        };
                        lines.push(Line::from(format!(
                            "  • Context window: {} used of {} ({:.0}% left)",
                            format_with_separators(used),
                            format_with_separators(window),
                            percent_left
                        )));
                        lines.push(Line::from(format!(
                            "  • {} tokens before overflow",
                            format_with_separators(remaining)
                        )));
                        lines.push(Line::from("  • Auto-compaction runs after overflow errors".to_string()));
                    } else {
                        lines.push(Line::from("  • Auto-compaction runs after overflow errors".to_string()));
                    }
                } else {
                    lines.push(Line::from("  • Auto-compaction runs after overflow errors".to_string()));
                }
            }
        }
    }

    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

pub(crate) fn new_warning_event(message: String) -> PlainMessageState {
    let warn_style = Style::default().fg(crate::colors::warning());
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(2);
    lines.push(Line::from("notice"));
    lines.push(Line::from(vec![Span::styled(format!("⚠ {message}"), warn_style)]));
    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

pub(crate) fn new_prompts_output() -> PlainMessageState {
    let lines: Vec<Line<'static>> = vec![
        Line::from("/prompts").fg(crate::colors::keyword()),
        Line::from(""),
        Line::from(" 1. Explain this codebase"),
        Line::from(" 2. Summarize recent commits"),
        Line::from(" 3. Implement {feature}"),
        Line::from(" 4. Find and fix a bug in @filename"),
        Line::from(" 5. Write tests for @filename"),
        Line::from(" 6. Improve documentation in @filename"),
        Line::from(""),
    ];
    plain_message_state_from_lines(lines, HistoryCellType::Notice)
}

fn plan_progress_icon(total: usize, completed: usize) -> PlanIcon {
    if total == 0 || completed == 0 {
        PlanIcon::Custom("progress-empty".to_string())
    } else if completed >= total {
        PlanIcon::Custom("progress-complete".to_string())
    } else if completed.saturating_mul(3) <= total {
        PlanIcon::Custom("progress-start".to_string())
    } else if completed.saturating_mul(3) < total.saturating_mul(2) {
        PlanIcon::Custom("progress-mid".to_string())
    } else {
        PlanIcon::Custom("progress-late".to_string())
    }
}

pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> PlanUpdateCell {
    let UpdatePlanArgs { name, plan } = update;

    let total = plan.len();
    let completed = plan
        .iter()
        .filter(|p| matches!(p.status, StepStatus::Completed))
        .count();
    let icon = plan_progress_icon(total, completed);
    let progress = PlanProgress { completed, total };

    let name = name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Plan")
        .to_string();

    let steps: Vec<PlanStep> = plan
        .into_iter()
        .map(|PlanItemArg { step, status }| PlanStep {
            description: step,
            status,
        })
        .collect();

    let state = PlanUpdateState {
        id: HistoryId::ZERO,
        name,
        icon,
        progress,
        steps,
    };

    PlanUpdateCell::new(state)
}

pub(crate) fn new_patch_event(
    event_type: PatchEventType,
    changes: HashMap<PathBuf, FileChange>,
) -> PatchSummaryCell {
    let record = PatchRecord {
        id: HistoryId::ZERO,
        patch_type: match event_type {
            PatchEventType::ApprovalRequest => HistoryPatchEventType::ApprovalRequest,
            PatchEventType::ApplyBegin { auto_approved } => {
                HistoryPatchEventType::ApplyBegin { auto_approved }
            }
            PatchEventType::ApplySuccess => HistoryPatchEventType::ApplySuccess,
            PatchEventType::ApplyFailure => HistoryPatchEventType::ApplyFailure,
        },
        changes,
        failure: None,
    };
    PatchSummaryCell::from_record(record)
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PlainMessageState {
    let mut lines: Vec<Line<'static>> = vec![
        Line::from("❌ Patch application failed")
            .fg(crate::colors::error())
            .bold(),
        Line::from(""),
    ];

    let norm = normalize_overwrite_sequences(&stderr);
    let norm = sanitize_for_tui(
        &norm,
        SanitizeMode::AnsiPreserving,
        SanitizeOptions {
            expand_tabs: true,
            tabstop: 4,
            debug_markers: false,
        },
    );
    for line in norm.lines() {
        if !line.is_empty() {
            lines.push(ansi_escape_line(line).fg(crate::colors::error()));
        }
    }

    lines.push(Line::from(""));
    plain_message_state_from_lines(
        lines,
        HistoryCellType::Patch {
            kind: PatchKind::ApplyFailure,
        },
    )
}

// ==================== PatchSummaryCell ====================
// Renders patch summary + details with width-aware hanging indents so wrapped
// diff lines align under their code indentation.

pub(crate) struct PatchSummaryCell {
    pub(crate) title: String,
    pub(crate) kind: PatchKind,
    pub(crate) record: PatchRecord,
}

fn patch_changes_are_rename_only(changes: &HashMap<PathBuf, FileChange>) -> bool {
    !changes.is_empty() && changes.values().all(file_change_is_rename_only)
}

fn patch_changes_are_noop(changes: &HashMap<PathBuf, FileChange>) -> bool {
    !changes.is_empty()
        && changes.values().all(|change| match change {
            FileChange::Update {
                move_path: None,
                unified_diff,
                ..
            } => {
                !diff_contains_line_edits(unified_diff)
                    && !diff_contains_binary_markers(unified_diff)
                    && !diff_contains_metadata_markers(unified_diff)
            }
            _ => false,
        })
}

fn file_change_is_rename_only(change: &FileChange) -> bool {
    match change {
        FileChange::Update {
            move_path: Some(_),
            unified_diff,
            ..
        } => {
            !diff_contains_line_edits(unified_diff)
                && !diff_contains_binary_markers(unified_diff)
                && !diff_contains_metadata_markers_excluding_rename(unified_diff)
        }
        _ => false,
    }
}

fn diff_contains_line_edits(diff: &str) -> bool {
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        match line.as_bytes().first() {
            Some(b'+') | Some(b'-') => return true,
            _ => {}
        }
    }
    false
}

fn diff_contains_binary_markers(diff: &str) -> bool {
    diff.contains("Binary files") || diff.contains("GIT binary patch") || diff.as_bytes().contains(&0)
}

fn diff_contains_metadata_markers(diff: &str) -> bool {
    diff.contains("new file mode")
        || diff.contains("deleted file mode")
        || diff.contains("old mode")
        || diff.contains("new mode")
        || diff.contains("similarity index")
        || diff.contains("rename from")
        || diff.contains("rename to")
}

fn diff_contains_metadata_markers_excluding_rename(diff: &str) -> bool {
    diff.contains("new file mode")
        || diff.contains("deleted file mode")
        || diff.contains("old mode")
        || diff.contains("new mode")
}

impl PatchSummaryCell {
    pub(crate) fn from_record(record: PatchRecord) -> Self {
        let kind = match record.patch_type {
            HistoryPatchEventType::ApprovalRequest => PatchKind::Proposed,
            HistoryPatchEventType::ApplyBegin { .. } => PatchKind::ApplyBegin,
            HistoryPatchEventType::ApplySuccess => PatchKind::ApplySuccess,
            HistoryPatchEventType::ApplyFailure => PatchKind::ApplyFailure,
        };
        let rename_only = patch_changes_are_rename_only(&record.changes);
        let noop_only = patch_changes_are_noop(&record.changes);
        let title = match record.patch_type {
            HistoryPatchEventType::ApprovalRequest => "proposed patch".to_string(),
            HistoryPatchEventType::ApplyFailure => "Patch failed".to_string(),
            HistoryPatchEventType::ApplyBegin { .. }
            | HistoryPatchEventType::ApplySuccess => {
                if rename_only {
                    "Renamed".to_string()
                } else if noop_only {
                    "No changes".to_string()
                } else {
                    "Updated".to_string()
                }
            }
        };
        Self {
            title,
            kind,
            record,
        }
    }

    fn ui_event_type(&self) -> PatchEventType {
        match self.record.patch_type {
            HistoryPatchEventType::ApprovalRequest => PatchEventType::ApprovalRequest,
            HistoryPatchEventType::ApplyBegin { auto_approved } => {
                PatchEventType::ApplyBegin { auto_approved }
            }
            HistoryPatchEventType::ApplySuccess => PatchEventType::ApplySuccess,
            HistoryPatchEventType::ApplyFailure => PatchEventType::ApplyFailure,
        }
    }

    pub(crate) fn record(&self) -> &PatchRecord {
        &self.record
    }

    pub(crate) fn record_mut(&mut self) -> &mut PatchRecord {
        &mut self.record
    }

    fn build_lines(&self, width: u16) -> Vec<Line<'static>> {
        let effective_width = width.max(1);
        let mut lines: Vec<Line<'static>> = create_diff_summary_with_width(
            &self.title,
            &self.record.changes,
            self.ui_event_type(),
            Some(effective_width as usize),
        )
        .into_iter()
        .collect();

        if matches!(
            self.record.patch_type,
            HistoryPatchEventType::ApplyFailure
        ) {
            if let Some(metadata) = &self.record.failure {
                if !lines.is_empty() {
                    lines.push(Line::default());
                }
                lines.push(
                    Line::from("Patch application failed")
                        .fg(crate::colors::error())
                        .bold(),
                );
                if !metadata.message.is_empty() {
                    lines.push(Line::from(metadata.message.clone()).fg(crate::colors::error()));
                }
                if let Some(stdout) = &metadata.stdout_excerpt {
                    if !stdout.is_empty() {
                        lines.push(Line::default());
                        lines.push(Line::from("stdout excerpt:").fg(crate::colors::info()));
                        for line in stdout.lines() {
                            lines.push(Line::from(line.to_string()).fg(crate::colors::text()));
                        }
                    }
                }
                if let Some(stderr) = &metadata.stderr_excerpt {
                    if !stderr.is_empty() {
                        lines.push(Line::default());
                        lines.push(Line::from("stderr excerpt:").fg(crate::colors::error()));
                        for line in stderr.lines() {
                            lines.push(Line::from(line.to_string()).fg(crate::colors::error()));
                        }
                    }
                }
            }
        }
        lines
    }
}

impl HistoryCell for PatchSummaryCell {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn kind(&self) -> HistoryCellType {
        HistoryCellType::Patch { kind: self.kind }
    }

    // We compute lines based on width at render time; provide a conservative
    // default for non-width callers (not normally used in our pipeline).
    fn display_lines(&self) -> Vec<Line<'static>> {
        self.build_lines(80)
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn desired_height(&self, width: u16) -> u16 {
        // Trim leading/trailing empty lines to keep height in sync with render.
        let lines = trim_empty_lines(self.build_lines(width));
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    fn custom_render_with_skip(&self, area: Rect, buf: &mut Buffer, skip_rows: u16) {
        // Render with trimmed lines and pre-clear the area to avoid residual glyphs
        // when content shrinks (e.g., after width changes or trimming).
        let lines = trim_empty_lines(self.build_lines(area.width));
        let text = Text::from(lines);

        let cell_bg = crate::colors::background();
        let bg_block = Block::default().style(Style::default().bg(cell_bg));

        // Proactively fill the full draw area with the background.
        // This mirrors other cells that ensure a clean slate before drawing.
        crate::util::buffer::fill_rect(
            buf,
            area,
            Some(' '),
            Style::default().bg(cell_bg).fg(crate::colors::text()),
        );

        Paragraph::new(text)
            .block(bg_block)
            .wrap(Wrap { trim: false })
            .scroll((skip_rows, 0))
            .style(Style::default().bg(cell_bg))
            .render(area, buf);
    }
}

// new_patch_apply_success was removed in favor of in-place header mutation and type update in chatwidget

// ==================== Spacing Helper ====================

/// Check if a line appears to be a title/header (like "codex", "user", "thinking", etc.)
fn is_title_line(line: &Line) -> bool {
    // Check if the line has special formatting that indicates it's a title
    if line.spans.is_empty() {
        return false;
    }

    // Get the text content of the line
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
        .trim()
        .to_lowercase();

    // Check for common title patterns (fallback heuristic only; primary logic uses explicit cell types)
    matches!(
        text.as_str(),
        "codex"
            | "user"
            | "thinking"
            | "event"
            | "tool"
            | "/diff"
            | "/status"
            | "/prompts"
            | "reasoning effort"
            | "error"
    ) || text.starts_with("⚡")
        || text.starts_with("⚙")
        || text.starts_with("✓")
        || text.starts_with("✗")
        || text.starts_with("↯")
        || text.starts_with("proposed patch")
        || text.starts_with("applying patch")
        || text.starts_with("updating")
        || text.starts_with("updated")
}

/// Check if a line is empty (no content or just whitespace)
fn is_empty_line(line: &Line) -> bool {
    if line.spans.is_empty() {
        return true;
    }
    // Consider a line empty when all spans have only whitespace
    line.spans
        .iter()
        .all(|s| s.content.as_ref().trim().is_empty())
}

/// Trim empty lines from the beginning and end of a Vec<Line>.
/// Also normalizes internal spacing - no more than 1 empty line between content.
/// This ensures consistent spacing when cells are rendered together.
pub(crate) fn trim_empty_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    // Remove ALL leading empty lines
    while lines.first().map_or(false, is_empty_line) {
        lines.remove(0);
    }

    // Remove ALL trailing empty lines
    while lines.last().map_or(false, is_empty_line) {
        lines.pop();
    }

    // Normalize internal spacing - no more than 1 empty line in a row
    let mut result = Vec::new();
    let mut prev_was_empty = false;

    for line in lines {
        let is_empty = is_empty_line(&line);

        // Skip consecutive empty lines
        if is_empty && prev_was_empty {
            continue;
        }

        // Special case: If this is an empty line right after a title, skip it
        if is_empty && result.len() == 1 && result.first().map_or(false, is_title_line) {
            continue;
        }

        result.push(line);
        prev_was_empty = is_empty;
    }

    result
}

pub(crate) fn cell_from_record(record: &crate::history::state::HistoryRecord, cfg: &Config) -> Box<dyn HistoryCell> {
    match record {
        HistoryRecord::PlainMessage(state) => Box::new(PlainHistoryCell::from_state(state.clone())),
        HistoryRecord::WaitStatus(state) => Box::new(wait_status::WaitStatusCell::from_state(state.clone())),
        HistoryRecord::Loading(state) => Box::new(loading::LoadingCell::from_state(state.clone())),
        HistoryRecord::RunningTool(state) => {
            Box::new(tool::RunningToolCallCell::from_state(state.clone()))
        }
        HistoryRecord::ToolCall(state) => Box::new(tool::ToolCallCell::from_state(state.clone())),
        HistoryRecord::PlanUpdate(state) => Box::new(plan_update::PlanUpdateCell::from_state(state.clone())),
        HistoryRecord::UpgradeNotice(state) => Box::new(upgrade::UpgradeNoticeCell::from_state(state.clone())),
        HistoryRecord::Reasoning(state) => Box::new(reasoning::CollapsibleReasoningCell::from_state(state.clone())),
        HistoryRecord::Exec(state) => Box::new(exec::ExecCell::from_record(state.clone())),
        HistoryRecord::MergedExec(state) => Box::new(MergedExecCell::from_state(state.clone())),
        HistoryRecord::AssistantStream(state) => {
            Box::new(stream::StreamingContentCell::from_state(
                state.clone(),
                cfg.file_opener,
                cfg.cwd.clone(),
            ))
        }
        HistoryRecord::AssistantMessage(state) => {
            Box::new(assistant::AssistantMarkdownCell::from_state(state.clone(), cfg))
        }
        HistoryRecord::Diff(state) => Box::new(diff::DiffCell::from_record(state.clone())),
        HistoryRecord::Image(state) => Box::new(image::ImageOutputCell::from_record(state.clone())),
        HistoryRecord::Explore(state) => {
            Box::new(explore::ExploreAggregationCell::from_record(state.clone()))
        }
        HistoryRecord::RateLimits(state) => Box::new(rate_limits::RateLimitsCell::from_record(state.clone())),
        HistoryRecord::Patch(state) => Box::new(PatchSummaryCell::from_record(state.clone())),
        HistoryRecord::BackgroundEvent(state) => Box::new(background::BackgroundEventCell::new(state.clone())),
        HistoryRecord::Notice(state) => Box::new(PlainHistoryCell::from_notice_record(state.clone())),
    }
}

pub(crate) fn lines_from_record(record: &crate::history::state::HistoryRecord, cfg: &Config) -> Vec<Line<'static>> {
    match record {
        HistoryRecord::Explore(state) => return explore_lines_from_record(state),
        _ => {}
    }
    cell_from_record(record, cfg).display_lines_trimmed()
}

pub(crate) fn merged_exec_lines_from_record(record: &MergedExecRecord) -> Vec<Line<'static>> {
    MergedExecCell::from_state(record.clone()).display_lines()
}

pub(crate) fn record_from_cell(cell: &dyn HistoryCell) -> Option<HistoryRecord> {
    if let Some(plain) = cell.as_any().downcast_ref::<PlainHistoryCell>() {
        return Some(HistoryRecord::PlainMessage(plain.state().clone()));
    }
    if let Some(wait) = cell.as_any().downcast_ref::<wait_status::WaitStatusCell>() {
        return Some(HistoryRecord::WaitStatus(wait.state().clone()));
    }
    if let Some(loading) = cell.as_any().downcast_ref::<loading::LoadingCell>() {
        return Some(HistoryRecord::Loading(loading.state().clone()));
    }
    if let Some(background) = cell
        .as_any()
        .downcast_ref::<background::BackgroundEventCell>()
    {
        return Some(HistoryRecord::BackgroundEvent(background.state().clone()));
    }
    if let Some(merged) = cell.as_any().downcast_ref::<MergedExecCell>() {
        return Some(HistoryRecord::MergedExec(merged.to_record()));
    }
    if let Some(explore) = cell
        .as_any()
        .downcast_ref::<explore::ExploreAggregationCell>()
    {
        return Some(HistoryRecord::Explore(explore.record().clone()));
    }
    if let Some(tool_call) = cell.as_any().downcast_ref::<tool::ToolCallCell>() {
        return Some(HistoryRecord::ToolCall(tool_call.state().clone()));
    }
    if let Some(running_tool) = cell
        .as_any()
        .downcast_ref::<tool::RunningToolCallCell>()
    {
        return Some(HistoryRecord::RunningTool(running_tool.state().clone()));
    }
    if let Some(plan) = cell.as_any().downcast_ref::<plan_update::PlanUpdateCell>() {
        return Some(HistoryRecord::PlanUpdate(plan.state().clone()));
    }
    if let Some(upgrade) = cell.as_any().downcast_ref::<upgrade::UpgradeNoticeCell>() {
        return Some(HistoryRecord::UpgradeNotice(upgrade.state().clone()));
    }
    if let Some(reasoning) = cell
        .as_any()
        .downcast_ref::<reasoning::CollapsibleReasoningCell>()
    {
        return Some(HistoryRecord::Reasoning(reasoning.reasoning_state()));
    }
    if let Some(exec) = cell.as_any().downcast_ref::<exec::ExecCell>() {
        return Some(HistoryRecord::Exec(exec.record.clone()));
    }
    if let Some(stream) = cell
        .as_any()
        .downcast_ref::<stream::StreamingContentCell>()
    {
        return Some(HistoryRecord::AssistantStream(stream.state().clone()));
    }
    if let Some(assistant) = cell
        .as_any()
        .downcast_ref::<assistant::AssistantMarkdownCell>()
    {
        return Some(HistoryRecord::AssistantMessage(assistant.state().clone()));
    }
    if let Some(diff) = cell.as_any().downcast_ref::<diff::DiffCell>() {
        return Some(HistoryRecord::Diff(diff.record().clone()));
    }
    if let Some(image) = cell.as_any().downcast_ref::<image::ImageOutputCell>() {
        return Some(HistoryRecord::Image(image.record().clone()));
    }
    if let Some(patch) = cell.as_any().downcast_ref::<PatchSummaryCell>() {
        return Some(HistoryRecord::Patch(patch.record().clone()));
    }
    if let Some(rate_limits) = cell
        .as_any()
        .downcast_ref::<rate_limits::RateLimitsCell>()
    {
        return Some(HistoryRecord::RateLimits(rate_limits.record().clone()));
    }
    None
}

fn format_inline_node_for_display(command_escaped: &str) -> Option<String> {
    let tokens: Vec<String> = Shlex::new(command_escaped).collect();
    if tokens.len() < 2 {
        return None;
    }

    let node_idx = tokens
        .iter()
        .position(|token| is_node_invocation_token(token))?;

    let mut idx = node_idx + 1;
    while idx < tokens.len() {
        match tokens[idx].as_str() {
            "-e" | "--eval" | "-p" | "--print" => {
                let script_idx = idx + 1;
                if script_idx >= tokens.len() {
                    return None;
                }
                return format_node_script(&tokens, script_idx, tokens[script_idx].as_str());
            }
            "--" => break,
            _ => idx += 1,
        }
    }

    None
}

fn format_inline_shell_for_display(command_escaped: &str) -> Option<String> {
    let tokens: Vec<String> = Shlex::new(command_escaped).collect();
    if tokens.len() < 3 {
        return None;
    }

    let shell_idx = tokens
        .iter()
        .position(|t| is_shell_invocation_token(t))?;

    let flag_idx = shell_idx + 1;
    if flag_idx >= tokens.len() {
        return None;
    }

    let flag = tokens[flag_idx].as_str();
    if flag != "-c" && flag != "-lc" {
        return None;
    }

    let script_idx = flag_idx + 1;
    if script_idx >= tokens.len() {
        return None;
    }

    format_shell_script(&tokens, script_idx, tokens[script_idx].as_str())
}
