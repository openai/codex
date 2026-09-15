//! Bordered, aligned overview cards. Detailed reports remain in maximized mode.
//! Card heights depend on the viewport, never on the selected day or load state.

use super::AnalyticsView;
use super::data;
use super::render::columns;
use super::render::join_columns;
use super::styles::number;
use super::styles::secondary_style;
use crate::analytics::sections::Section;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow as truncate;
use crate::style::accent_style;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::ops::Range;

impl AnalyticsView {
    pub(super) fn dashboard_lines(
        &self,
        width: usize,
        height: usize,
    ) -> (Vec<Line<'static>>, Range<usize>) {
        let count = if width >= 104 { 2 } else { 1 };
        let card_width = if count == 2 { (width - 4) / 2 } else { width };
        let visible = self.visible_sections();
        let rows = visible.len().div_ceil(count).max(/*other*/ 1);
        let card_height =
            (height.saturating_sub(/*rhs*/ 2) / rows).clamp(/*min*/ 16, /*max*/ 24);
        let inner_width = card_width.saturating_sub(/*rhs*/ 4).max(/*other*/ 1);
        let inner_height = card_height - 4;
        let mut lines = Vec::new();
        let mut selected = 0..0;
        for (row, chunk) in visible.chunks(count).enumerate() {
            let mut cards = Vec::new();
            for (column, section) in chunk.iter().copied().enumerate() {
                let focused = self.section == section;
                let border = if focused {
                    accent_style()
                } else {
                    secondary_style().dim()
                };
                let mut content = Vec::new();
                if !matches!(section, Section::Chats | Section::Plan | Section::Summary) {
                    content.push(
                        format!(
                            "{}d{}",
                            if self.ranges[self.range_group(section) as usize] == 0 {
                                7
                            } else {
                                30
                            },
                            if section == Section::Usage && self.business() {
                                format!(
                                    " · {}",
                                    self.token_model.as_deref().unwrap_or("All models")
                                )
                            } else {
                                String::new()
                            }
                        )
                        .set_style(secondary_style())
                        .into(),
                    );
                }
                if matches!(
                    section,
                    Section::Usage | Section::Credits | Section::Activity | Section::Plan
                ) {
                    content.push(
                        format!(
                            "By {}",
                            self.group_label(section, self.sections[section].group)
                                .to_lowercase()
                        )
                        .set_style(secondary_style())
                        .into(),
                    );
                }
                match section {
                    Section::Summary => content.extend(self.summary_lines(inner_width)),
                    Section::Usage
                    | Section::Plugins
                    | Section::Credits
                    | Section::Activity
                    | Section::Skills => content.extend(
                        self.history_lines(
                            section,
                            inner_width,
                            inner_height.saturating_sub(/*rhs*/ 13).max(/*other*/ 1),
                        )
                        .lines,
                    ),
                    Section::Plan => content.extend(self.plan_lines(inner_width).0),
                    Section::Chats if !self.business() => {
                        content.extend(self.task_lines(inner_width).0)
                    }
                    Section::Chats => {
                        content.push(
                            "30d active · lifetime credits"
                                .set_style(secondary_style())
                                .into(),
                        );
                        content.push(Line::default());
                        if let Some(chats) = self.chats.ready() {
                            if chats.rows.is_empty() {
                                content.push(
                                    "No recent local chats.".set_style(secondary_style()).into(),
                                );
                            }
                            for chat in chats.rows.iter().take(/*n*/ 5) {
                                let amount = chat
                                    .usage
                                    .as_ref()
                                    .map(|usage| {
                                        data::credits(usage.estimated_usage_credits_micros)
                                    })
                                    .unwrap_or_else(|| "—".into());
                                content.push(columns(
                                    truncate(
                                        chat.display_title().to_string().into(),
                                        inner_width.saturating_sub(amount.len() + 1),
                                    ),
                                    number(amount).into(),
                                    inner_width,
                                ));
                            }
                            content.push(Line::default());
                            content.push(
                                "Local chats · excludes subagents"
                                    .set_style(secondary_style())
                                    .into(),
                            );
                            if chats.rows.iter().any(|chat| chat.usage.is_none()) {
                                content.push(
                                    "Some estimates unavailable"
                                        .set_style(secondary_style())
                                        .into(),
                                );
                            }
                        } else if let Some(message) = self.chats.message() {
                            content.push(message.to_string().set_style(secondary_style()).into());
                        }
                    }
                }
                let mut content = word_wrap_lines(content, RtOptions::new(inner_width));
                if content.len() > inner_height {
                    content.truncate(inner_height);
                    content[inner_height - 1] =
                        "… z to maximize".set_style(secondary_style()).into();
                }
                content.resize(inner_height, Line::default());
                let label = format!(
                    " {} {} ",
                    row * count + column + 1,
                    self.section_title(section)
                );
                let title = truncate(
                    Line::from(vec![
                        " ".into(),
                        if focused {
                            "▸".set_style(accent_style())
                        } else {
                            " ".into()
                        },
                        if focused {
                            label.bold()
                        } else {
                            label.set_style(secondary_style())
                        },
                    ]),
                    card_width.saturating_sub(/*rhs*/ 3),
                );
                let mut top = vec!["╭─".set_style(border)];
                top.extend(title.spans.iter().cloned());
                top.push(
                    "─"
                        .repeat(card_width.saturating_sub(title.width() + 3))
                        .set_style(border),
                );
                top.push("╮".set_style(border));
                let blank = columns(
                    "│".set_style(border).into(),
                    "│".set_style(border).into(),
                    card_width,
                );
                let mut card = vec![Line::from(top), blank.clone()];
                card.extend(content.into_iter().map(|line| {
                    let line = truncate(line, inner_width);
                    let padding = card_width.saturating_sub(line.width() + 3);
                    let mut padded = vec!["│".set_style(border), " ".into()];
                    padded.extend(line.spans);
                    padded.push(" ".repeat(padding).into());
                    padded.push("│".set_style(border));
                    Line::from(padded)
                }));
                card.push(blank);
                card.push(
                    format!("╰{}╯", "─".repeat(card_width.saturating_sub(/*rhs*/ 2)))
                        .set_style(border)
                        .into(),
                );
                cards.push(
                    card.into_iter()
                        .map(|line| truncate(line, card_width))
                        .collect::<Vec<_>>(),
                );
            }
            if chunk.contains(&self.section) {
                selected = lines.len()..lines.len() + card_height;
            }
            lines.extend(if cards.len() == 2 {
                join_columns(&cards[0], &cards[1], card_width)
            } else {
                cards.remove(/*index*/ 0)
            });
            if (row + 1) * count < visible.len() {
                lines.push(Line::default());
            }
        }
        (lines, selected)
    }
}
