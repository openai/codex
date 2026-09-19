//! Compact MCP and code-mode activity previews, independent of retained detail rendering.

use super::McpToolCallCell;
use super::NodeReplExecOutput;
use super::result::McpResultKind;
use crate::history_cell::activity_preview::DETAIL_PREVIEW_LINES;
use crate::history_cell::activity_preview::clipped_line;
use crate::live_wrap::take_prefix_by_width;
use crate::motion::MotionMode;
use crate::motion::ReducedMotionIndicator;
use crate::motion::activity_indicator;
use crate::style::accent_color;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::borrow::Cow;
use std::collections::VecDeque;

impl McpToolCallCell {
    pub(super) fn compact_mcp_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let status = self.success();
        let (marker, verb) = match status {
            Some(true) => ("•".green().bold(), "Called"),
            Some(false) => ("•".red().bold(), "Failed"),
            None => (
                activity_indicator(
                    Some(self.start_time),
                    MotionMode::from_animations_enabled(self.animations_enabled),
                    ReducedMotionIndicator::StaticBullet,
                )
                .unwrap_or_else(|| "•".dim()),
                "Calling",
            ),
        };
        let title = self
            .invocation
            .arguments
            .as_ref()
            .filter(|_| self.result_kind() == McpResultKind::NodeRepl)
            .and_then(|arguments| arguments.get("title"))
            .and_then(serde_json::Value::as_str)
            .map(|title| title.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| format!("{}.{}", self.invocation.server, self.invocation.tool));
        let mut lines = vec![clipped_line(
            Line::from(vec![
                marker,
                " ".into(),
                verb.bold(),
                " ".into(),
                title.fg(accent_color()),
            ]),
            width,
        )];
        let mut details = VecDeque::with_capacity(DETAIL_PREVIEW_LINES);
        let push_tail = |details: &mut VecDeque<String>, text: &str| {
            for line in text
                .lines()
                .rev()
                .take(DETAIL_PREVIEW_LINES - details.len())
            {
                // Bound selected rows before allocating; retained output may be megabytes.
                let bounded = &line[..line.floor_char_boundary(16 * 1024)];
                let (prefix, rest, _) = take_prefix_by_width(bounded, usize::from(width) + 1);
                let mut detail = prefix.to_owned();
                if !rest.is_empty() || bounded.len() < line.len() {
                    detail.push('…');
                }
                details.push_front(detail);
            }
        };
        if let Some(result) = &self.result {
            match result {
                Ok(result) => {
                    for block in result.content.iter().rev() {
                        let output = if self.result_kind() == McpResultKind::NodeRepl
                            && status == Some(true)
                            && let Some(text) = block.text()
                        {
                            if text.starts_with("Script completed\n")
                                && let Some((_, output)) = text.split_once("\nOutput:\n")
                            {
                                Cow::Borrowed(output)
                            } else if let Ok(output) =
                                serde_json::from_str::<NodeReplExecOutput>(text)
                                && output.exit_code == 0
                            {
                                Cow::Owned(output.output)
                            } else {
                                Cow::Borrowed(block.render_full())
                            }
                        } else {
                            Cow::Borrowed(block.render_full())
                        };
                        push_tail(&mut details, &output);
                        // A code-mode image can also carry text; its marker precedes that output.
                        if block.is_image
                            && block.text().is_some()
                            && details.len() < DETAIL_PREVIEW_LINES
                        {
                            details.push_front("Returned image".to_owned());
                        }
                        if details.len() == DETAIL_PREVIEW_LINES {
                            break;
                        }
                    }
                }
                Err(error) => {
                    push_tail(&mut details, error);
                    if error.lines().take(DETAIL_PREVIEW_LINES + 1).count() <= DETAIL_PREVIEW_LINES
                    {
                        if let Some(first) = details.front_mut() {
                            first.insert_str(0, "Error: ");
                        } else {
                            details.push_front("Error: ".to_owned());
                        }
                    }
                }
            }
        }
        for (index, detail) in details.into_iter().enumerate() {
            lines.push(clipped_line(
                Line::from(vec![
                    if index == 0 { "  └ " } else { "    " }.dim(),
                    detail.dim(),
                ]),
                width,
            ));
        }
        lines
    }
}
