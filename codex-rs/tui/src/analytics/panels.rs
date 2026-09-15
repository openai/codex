//! Responsive panels with consistent category colors and signed daily amounts.

use super::AnalyticsView;
use super::render::columns;
use super::styles::secondary_style;
use crate::analytics::sections::Section;
use crate::style::accent_style;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Styled;
use ratatui::style::Stylize;
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
        let focused = section == self.section;
        let title = format!(
            "{} {}",
            if focused { "▎" } else { " " },
            self.section_title(section)
        );
        let heading = if focused {
            title.set_style(accent_style()).into()
        } else {
            title.bold().into()
        };
        let group = self.sections[section].group;
        let heading = if matches!(
            section,
            Section::Usage | Section::Credits | Section::Activity | Section::Plan
        ) {
            columns(
                heading,
                format!("By {}", self.group_label(section, group).to_lowercase())
                    .set_style(secondary_style())
                    .into(),
                width,
            )
        } else {
            heading
        };
        let mut lines = vec![heading, "─".repeat(width).dim().into()];
        if section == Section::Usage && self.business() {
            lines.push(
                format!(
                    "{} · m change model",
                    self.token_model.as_deref().unwrap_or("All models")
                )
                .set_style(secondary_style())
                .into(),
            );
        }
        let mut selection;
        let mut chart_bands = 0;
        if focused && let Some(choice) = self.group_picker {
            lines.extend(
                self.group_options()
                    .iter()
                    .enumerate()
                    .map(|(index, group)| {
                        let label = format!(
                            "{} {}",
                            if index == choice { "›" } else { " " },
                            self.group_label(section, *group)
                        );
                        if index == choice {
                            label.set_style(accent_style()).into()
                        } else {
                            label.into()
                        }
                    }),
            );
        }
        if section == Section::Plan {
            let (plan, detail) = self.plan_lines(width);
            selection = lines.len() + detail.start..lines.len() + detail.end;
            lines.extend(plan);
        } else if section == Section::Chats {
            let (chats, detail) = self.chat_lines(width);
            selection = lines.len() + detail.start..lines.len() + detail.end;
            lines.extend(chats);
        } else {
            let history = self.history_lines(section, width, chart_height);
            chart_bands = history.bands;
            lines.extend(history.lines);
            selection = 0..lines.len();
        }
        if focused && self.group_picker.is_some() {
            selection = 0..2 + self.group_options().len();
        }
        let mut wrapped = Vec::new();
        let mut wrapped_selection = 0..0;
        for (index, line) in lines.into_iter().enumerate() {
            if index == selection.start {
                wrapped_selection.start = wrapped.len();
            }
            wrapped.extend(word_wrap_lines(
                [line],
                RtOptions::new(width.max(/*other*/ 1)),
            ));
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
