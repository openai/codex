use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::any::Any;
use unicode_width::UnicodeWidthStr;
mod agent;
mod approval_decision;
mod composite;
mod deprecation;
mod exec;
mod final_separator;
mod mcp;
mod notice;
mod patch;
mod patch_apply_failure;
mod plain;
mod plan;
mod prefixed_wrapped;
mod reasoning_summary;
mod review_status;
mod session;
mod update_available;
mod user;
mod view_image;
mod web_search;

pub(crate) use agent::AgentMessageCell;
#[expect(unused_imports)]
pub(crate) use approval_decision::ApprovalDecisionCell;
pub(crate) use approval_decision::new_approval_decision_cell;
pub(crate) use composite::CompositeHistoryCell;
pub(crate) use deprecation::new_deprecation_notice;
pub(crate) use final_separator::FinalMessageSeparator;
pub(crate) use mcp::McpToolCallCell;
#[expect(unused_imports)]
pub(crate) use mcp::McpToolsOutputCell;
pub(crate) use mcp::empty_mcp_output;
pub(crate) use mcp::new_active_mcp_tool_call;
pub(crate) use mcp::new_mcp_tools_output;
pub(crate) use notice::new_error_event;
pub(crate) use notice::new_info_event;
pub(crate) use notice::new_warning_event;
#[expect(unused_imports)]
pub(crate) use patch::PatchHistoryCell;
pub(crate) use patch::new_patch_event;
#[expect(unused_imports)]
pub(crate) use patch_apply_failure::PatchApplyFailureCell;
pub(crate) use patch_apply_failure::new_patch_apply_failure;
pub(crate) use plain::PlainHistoryCell;
#[expect(unused_imports)]
pub(crate) use plan::PlanUpdateCell;
pub(crate) use plan::new_plan_update;
pub(crate) use prefixed_wrapped::PrefixedWrappedHistoryCell;
#[expect(unused_imports)]
pub(crate) use reasoning_summary::ReasoningSummaryCell;
pub(crate) use reasoning_summary::new_reasoning_summary_block;
#[expect(unused_imports)]
pub(crate) use review_status::ReviewStatusCell;
pub(crate) use review_status::new_review_status_line;
pub(crate) use session::SessionInfoCell;
pub(crate) use session::new_session_info;
#[expect(unused_imports)]
pub(crate) use update_available::UpdateAvailableHistoryCell;
pub(crate) use user::UserHistoryCell;
pub(crate) use user::new_user_prompt;
#[expect(unused_imports)]
pub(crate) use view_image::ViewImageToolCallCell;
pub(crate) use view_image::new_view_image_tool_call;
#[expect(unused_imports)]
pub(crate) use web_search::WebSearchCallCell;
pub(crate) use web_search::new_web_search_call;

/// Represents an event to display in the conversation history. Returns its
/// `Vec<Line<'static>>` representation to make it easier to display in a
/// scrollable list.
pub(crate) trait HistoryCell: std::fmt::Debug + Send + Sync + Any {
    /// Render this cell into a set of display lines for the on-screen history panel.
    ///
    /// The width is the available area in the history list. Implementations should wrap or
    /// truncate as needed so callers can drop the result directly into a `Paragraph`.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    /// Render this cell to a newline-separated string for display-oriented assertions.
    ///
    /// This flattens the `display_lines` spans while preserving line breaks so tests can compare
    /// the rendered shape without manually joining.
    #[cfg(test)]
    fn display_string(&self, width: u16) -> String {
        lines_to_string(&self.display_lines(width))
    }

    /// Compute the preferred height for the on-screen history panel.
    ///
    /// The default implementation measures `display_lines` wrapped to the provided width so the
    /// caller can reserve the exact number of rows needed.
    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(self.display_lines(width)))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }

    /// Render this cell into a set of lines suitable for transcript export.
    ///
    /// The default implementation matches `display_lines`, but cells can opt in to returning
    /// additional context or omit styling-only padding when exporting.
    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    /// Render this cell to a newline-separated string for transcript assertions.
    ///
    /// This flattens `transcript_lines` without styling, mirroring `display_string` but using the
    /// transcript view.
    #[cfg(test)]
    fn transcript_string(&self, width: u16) -> String {
        lines_to_string(&self.transcript_lines(width))
    }

    /// Compute the preferred transcript height for export views.
    ///
    /// The default implementation mirrors `desired_height` but operates on `transcript_lines` so
    /// cells that elide content from the transcript can size correctly.
    fn desired_transcript_height(&self, width: u16) -> u16 {
        let lines = self.transcript_lines(width);
        // Workaround for ratatui bug: if there's only one line and it's whitespace-only, ratatui
        // gives 2 lines.
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

    /// Whether this cell is a continuation of the prior stream output.
    ///
    /// Streamed agent responses set this so the history renderer can avoid re-drawing an
    /// intermediate separator.
    fn is_stream_continuation(&self) -> bool {
        false
    }
}

impl Renderable for Box<dyn HistoryCell> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.display_lines(area.width);
        let y = if area.height == 0 {
            0
        } else {
            let overflow = lines.len().saturating_sub(usize::from(area.height));
            u16::try_from(overflow).unwrap_or(u16::MAX)
        };
        Paragraph::new(Text::from(lines))
            .scroll((y, 0))
            .render(area, buf);
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

#[cfg(test)]
fn flatten_line(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[cfg(test)]
fn lines_to_string(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(flatten_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render `lines` inside a border sized to the widest span in the content.
pub(crate) fn with_border(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    with_border_internal(lines, None)
}

/// Render `lines` inside a border whose inner width is at least `inner_width`.
///
/// This is useful when callers have already clamped their content to a
/// specific width and want the border math centralized here instead of
/// duplicating padding logic in the TUI widgets themselves.
pub(crate) fn with_border_with_inner_width(
    lines: Vec<Line<'static>>,
    inner_width: usize,
) -> Vec<Line<'static>> {
    with_border_internal(lines, Some(inner_width))
}

fn with_border_internal(
    lines: Vec<Line<'static>>,
    forced_inner_width: Option<usize>,
) -> Vec<Line<'static>> {
    let max_line_width = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0);
    let content_width = forced_inner_width
        .unwrap_or(max_line_width)
        .max(max_line_width);

    let mut out = Vec::with_capacity(lines.len() + 2);
    let border_inner_width = content_width + 2;
    out.push(vec![format!("╭{}╮", "─".repeat(border_inner_width)).dim()].into());

    for line in lines.into_iter() {
        let used_width: usize = line
            .iter()
            .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
            .sum();
        let span_count = line.spans.len();
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(span_count + 4);
        spans.push(Span::from("│ ").dim());
        spans.extend(line.into_iter());
        if used_width < content_width {
            spans.push(Span::from(" ".repeat(content_width - used_width)).dim());
        }
        spans.push(Span::from(" │").dim());
        out.push(Line::from(spans));
    }

    out.push(vec![format!("╰{}╯", "─".repeat(border_inner_width)).dim()].into());

    out
}

/// Return the emoji followed by a hair space (U+200A) to make a compact prefix.
/// Using only the hair space avoids excessive padding after the emoji while
/// still providing a small visual gap across terminals.
pub(crate) fn padded_emoji(emoji: &str) -> String {
    format!("{emoji}\u{200A}")
}
