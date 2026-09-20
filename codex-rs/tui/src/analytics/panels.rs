//! Responsive panels with consistent category colors and signed daily amounts.

use super::AnalyticsView;
use crate::analytics::sections::Section;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::text::Line;
use std::ops::Range;

/// Panel content and measured geometry shared by layout and scrolling.
pub(super) struct Panel {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) selection: Range<usize>,
    pub(super) chart_bands: usize,
}

impl AnalyticsView {
    pub(super) fn panel(&self, section: Section, width: usize, chart_height: usize) -> Panel {
        let mut lines = Vec::new();
        let selection;
        let mut chart_bands = 0;

        if section == Section::Summary {
            selection = 0..1;
            lines.extend(self.summary_lines(width));
        } else if section == Section::Plan {
            let (plan, detail) = self.plan_lines(width);
            selection = lines.len() + detail.start..lines.len() + detail.end;
            lines.extend(plan);
        } else if section == Section::Chats {
            let (chats, detail) = if self.business() {
                self.chat_lines(width)
            } else {
                self.task_lines(width)
            };
            selection = lines.len() + detail.start..lines.len() + detail.end;
            lines.extend(chats);
        } else {
            let history = self.history_lines(section, width, chart_height);
            chart_bands = history.bands;
            lines.extend(history.lines);
            selection = 0..lines.len();
        }

        let mut wrapped = Vec::new();
        let mut wrapped_selection = 0..0;
        for (index, line) in lines.into_iter().enumerate() {
            if index == selection.start {
                wrapped_selection.start = wrapped.len();
            }
            if line.style.bg.is_some() && line.width() <= width {
                // Keep highlighted padding on selected rows that already fit the panel.
                wrapped.push(line);
            } else {
                wrapped.extend(word_wrap_lines(
                    [line],
                    RtOptions::new(width.max(/*other*/ 1)),
                ));
            }
            if index + 1 == selection.end {
                wrapped_selection.end = wrapped.len();
            }
        }
        Panel {
            lines: wrapped,
            selection: wrapped_selection,
            chart_bands,
        }
    }
}
