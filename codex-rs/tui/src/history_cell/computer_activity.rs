//! Compact adjacent computer calls without discarding their chronological transcript details.
//!
//! CUA calls and intervening transcript-only reasoning share this cell. Visible history items
//! and turn boundaries end the group.
//! Preview selection favors failures and images, but never changes the order of retained rows.

use super::*;

#[derive(Debug)]
pub(crate) struct ComputerActivityCell {
    pub(crate) group: ActivityGroup<McpToolCallCell>,
}

impl Default for ComputerActivityCell {
    fn default() -> Self {
        Self {
            group: ActivityGroup::new(Vec::new()),
        }
    }
}

impl ComputerActivityCell {
    fn detailed_hyperlink_lines(&self, width: u16, mode: HistoryRenderMode) -> Vec<HyperlinkLine> {
        let mut lines = Vec::new();
        for (index, call) in self.group.calls.iter().enumerate() {
            lines.extend(call.transcript_hyperlink_lines(width));
            lines.extend(self.group.details.lines_after(index + 1, width, mode));
        }
        lines
    }

    pub(crate) fn call_ids(&self) -> impl Iterator<Item = &str> {
        self.group.calls.iter().map(McpToolCallCell::call_id)
    }

    /// Prepend validated historical calls without recreating pending live calls or their clocks.
    pub(crate) fn prepend(&mut self, older: Self) {
        self.group.prepend(older.group);
    }

    pub(crate) fn start(&mut self, call: McpToolCallCell) {
        if !self
            .group
            .calls
            .iter()
            .any(|existing| existing.call_id == call.call_id)
        {
            self.group.calls.push(call);
        }
    }

    /// Completion-only replay follows the same path as a live start followed by completion.
    pub(crate) fn complete(
        &mut self,
        call: McpToolCallCell,
        duration: Duration,
        result: Result<codex_protocol::mcp::CallToolResult, String>,
    ) {
        let id = call.call_id.clone();
        self.start(call);
        if let Some(call) = self.group.calls.iter_mut().find(|call| call.call_id == id) {
            call.complete(duration, result);
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.group.calls.iter().any(|call| call.result.is_none())
    }

    pub(crate) fn mark_failed(&mut self) {
        for call in &mut self.group.calls {
            if call.result.is_none() {
                call.mark_failed();
            }
        }
    }
}

fn has_image(call: &McpToolCallCell) -> bool {
    matches!(&call.result, Some(Ok(result)) if result.has_image)
}

/// Keep previews on one physical row, including grapheme clusters and narrow terminals.
fn preview(text: &str, width: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let (prefix, rest, _) = take_prefix_by_width(&normalized, width);
    if rest.is_empty() {
        prefix
    } else if width == 0 {
        String::new()
    } else {
        let (prefix, _, _) = take_prefix_by_width(&normalized, width.saturating_sub(1));
        format!("{prefix}…")
    }
}

fn error_preview(call: &McpToolCallCell) -> Option<&str> {
    let text = match call.result.as_ref()? {
        Err(error) => error.as_str(),
        Ok(result) => result
            .content
            .iter()
            .filter_map(result::McpContentBlock::text)
            .find(|text| text.starts_with("Script error:"))
            .or_else(|| {
                result
                    .content
                    .iter()
                    .find_map(|block| block.text().filter(|text| !text.trim().is_empty()))
            })?,
    };
    // CUA errors can append entire API manuals. The first nonempty line is the diagnostic;
    // preserve subsequent lines only in the full transcript, without inventing a paraphrase.
    text.strip_prefix("Script error:")
        .unwrap_or(text)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
}

impl HistoryCell for ComputerActivityCell {
    fn append_reasoning(&mut self, cell: Box<dyn HistoryCell>) -> Result<(), Box<dyn HistoryCell>> {
        if self.group.calls.is_empty() {
            Err(cell)
        } else {
            self.group.push_detail(std::sync::Arc::from(cell));
            Ok(())
        }
    }

    fn compact_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = self.display_lines(width);
        if width > 0 && !self.is_active() && self.group.calls.len() > 3 {
            // The owned transcript supplies the shared disclosure control. Legacy history keeps
            // its built-in summary, which is the final row whenever completed calls are hidden.
            lines.pop();
            if let Some(line) = lines.last_mut()
                && let Some(prefix) = line.spans.first_mut()
            {
                *prefix = "  └ ".dim();
            }
        }
        plain_hyperlink_lines(lines)
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let active = self
            .group
            .calls
            .iter()
            .rposition(|call| call.result.is_none());
        let failures = self
            .group
            .calls
            .iter()
            .filter(|call| call.success() == Some(false))
            .count();
        let count = self.group.calls.len();
        let bullet = active
            .and_then(|index| {
                let call = &self.group.calls[index];
                activity_indicator(
                    Some(call.start_time),
                    MotionMode::from_animations_enabled(call.animations_enabled),
                    ReducedMotionIndicator::StaticBullet,
                )
            })
            .unwrap_or_else(|| "•".dim());
        let label = if active.is_some() {
            "Using computer"
        } else {
            "Used computer"
        };
        let unit = if count == 1 { "action" } else { "actions" };
        let mut header = vec![
            bullet,
            " ".into(),
            label.bold(),
            format!(" · {count} {unit}").dim(),
        ];
        if failures > 0 {
            header.push(format!(" · {failures} failed").red());
        }
        let mut lines = adaptive_wrap_line(
            &Line::from(header),
            RtOptions::new(usize::from(width).max(1)).subsequent_indent("  ".into()),
        )
        .iter()
        .map(line_to_static)
        .collect::<Vec<_>>();
        let mut selected = if let Some(index) = active {
            vec![index]
        } else {
            let mut indices = (0..count).collect::<Vec<_>>();
            indices.sort_by_key(|&index| {
                let call = &self.group.calls[index];
                std::cmp::Reverse((call.success() == Some(false), has_image(call), index))
            });
            indices.truncate(if count > 3 { 2 } else { 3 });
            indices
        };
        selected.sort_unstable();
        let hidden = count - selected.len();
        let visible = selected.len();
        for (row_index, index) in selected.into_iter().enumerate() {
            let call = &self.group.calls[index];
            let title = call
                .invocation
                .arguments
                .as_ref()
                .and_then(|args| args.get("title"))
                .and_then(serde_json::Value::as_str)
                .filter(|title| !title.trim().is_empty())
                .unwrap_or("Computer action");
            let failed = call.success() == Some(false);
            let summary = if failed {
                match error_preview(call) {
                    Some(error) if width < 60 => format!("Failed: {error}"),
                    Some(error) => {
                        let title = preview(title, usize::from(width) / 3);
                        format!("Failed: {title} — {error}")
                    }
                    None => format!("Failed: {title}"),
                }
            } else if has_image(call) {
                format!("Captured screenshot · {title}")
            } else {
                title.to_string()
            };
            let summary = preview(&summary, usize::from(width).saturating_sub(4));
            let row = if failed { summary.red() } else { summary.dim() };
            let prefix = if row_index + 1 == visible && (hidden == 0 || active.is_some()) {
                "  └ "
            } else {
                "  ├ "
            };
            lines.push(vec![prefix.dim(), row].into());
        }
        if hidden > 0 && active.is_none() {
            let summary = preview(
                &format!("{hidden} more · ctrl+t"),
                usize::from(width).saturating_sub(4),
            );
            lines.push(vec!["  └ ".dim(), summary.dim()].into());
        }
        lines
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.transcript_hyperlink_lines(width))
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.detailed_hyperlink_lines(width, HistoryRenderMode::Rich)
    }

    fn activity_ids(&self) -> Vec<String> {
        self.call_ids().map(|id| format!("mcp:{id}")).collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(visible_lines(
            self.detailed_hyperlink_lines(u16::MAX, HistoryRenderMode::Raw),
        ))
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        self.group
            .calls
            .iter()
            .filter_map(HistoryCell::transcript_animation_tick)
            .max()
    }
}

#[cfg(test)]
#[path = "computer_activity_tests.rs"]
mod tests;
