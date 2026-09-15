//! Maximized sections and a responsive overview share the same panels and scrolling behavior.
//! Zoom, section changes, and reflow preserve report selections and inline details.

use super::AnalyticsView;
use super::data;
use super::styles::secondary_style;
use crate::analytics::models::AccountAnalyticsHistory;
use crate::analytics::models::AccountAnalyticsValue;
use crate::analytics::sections::Section;
use crate::keymap::ListAction;
use crate::style::accent_style;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::collections::BTreeMap;

pub(super) fn columns(left: Line<'static>, right: Line<'static>, width: usize) -> Line<'static> {
    let padding = width
        .saturating_sub(left.width() + right.width())
        .max(/*other*/ 1);
    let mut spans = left.spans;
    spans.push(" ".repeat(padding).into());
    spans.extend(right.spans);
    Line::from(spans)
}

/// Join independently stacked panels without leaving holes below shorter sections.
pub(super) fn join_columns(
    left: &[Line<'static>],
    right: &[Line<'static>],
    column_width: usize,
) -> Vec<Line<'static>> {
    (0..left.len().max(right.len()))
        .map(|row| {
            let left = left.get(row).cloned().unwrap_or_default();
            let mut line = columns(left, Line::default(), column_width + 4);
            if let Some(right) = right.get(row) {
                line.spans.extend(right.spans.iter().cloned());
            }
            line
        })
        .collect()
}

pub(super) fn categories(history: &AccountAnalyticsHistory) -> Vec<AccountAnalyticsValue> {
    let mut totals = BTreeMap::<String, AccountAnalyticsValue>::new();
    for value in history.data.iter().flat_map(|day| &day.values) {
        totals
            .entry(value.key.clone())
            .or_insert_with(|| AccountAnalyticsValue {
                value: 0.0,
                ..value.clone()
            })
            .value += value.value;
    }
    let mut values = totals.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| {
        b.value
            .abs()
            .total_cmp(&a.value.abs())
            .then_with(|| a.key.cmp(&b.key))
    });
    values
}

/// The last plotted category collects all remaining values without renormalizing them.
pub(super) fn parts(
    values: &[AccountAnalyticsValue],
    categories: &[AccountAnalyticsValue],
) -> [f64; 4] {
    let mut parts = [0.0; 4];
    for value in values {
        let index = categories
            .iter()
            .take(/*n*/ 3)
            .position(|category| category.key == value.key)
            .unwrap_or(/*default*/ 3);
        parts[index] += value.value;
    }
    parts
}

impl AnalyticsView {
    pub(super) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.poll_reports();
        let width = usize::from(area.width.saturating_sub(/*rhs*/ 4)).max(/*other*/ 1);
        let show_range = !matches!(self.section, Section::Chats | Section::Plan)
            && !self.visible_sections().is_empty();
        let hint = |action| {
            self.keymap
                .primary_hint(action)
                .map(crate::key_hint::ShortcutHint::display_label)
                .unwrap_or_default()
        };
        let controls = if self.visible_sections().is_empty() {
            format!("R refresh · {} back · q close", hint(ListAction::Cancel))
        } else if self.group_picker.is_some() {
            format!(
                "{}/{} group · {} select · {} cancel",
                hint(ListAction::MoveUp),
                hint(ListAction::MoveDown),
                hint(ListAction::Accept),
                hint(ListAction::Cancel)
            )
        } else {
            let navigation = if !self.zoomed {
                format!("{}/z maximize", hint(ListAction::Accept))
            } else if self.section == Section::Plan {
                format!(
                    "{}/{} window · {}/{} period · {} details",
                    hint(ListAction::MoveLeft),
                    hint(ListAction::MoveRight),
                    hint(ListAction::MoveUp),
                    hint(ListAction::MoveDown),
                    hint(ListAction::Accept)
                )
            } else if self.section != Section::Chats {
                format!(
                    "{}/{} day · {} details",
                    hint(ListAction::MoveLeft),
                    hint(ListAction::MoveRight),
                    hint(ListAction::Accept)
                )
            } else {
                format!(
                    "{}/{} row · {} details",
                    hint(ListAction::MoveUp),
                    hint(ListAction::MoveDown),
                    hint(ListAction::Accept)
                )
            };
            format!(
                "tab/1–{} section · {}{navigation}\n{}{}R refresh · {}/{} scroll · {} back · q close",
                self.visible_sections().len(),
                if self.zoomed { "z dashboard · " } else { "" },
                if show_range { "r 7/30d · " } else { "" },
                if self.group_options().len() < 2 {
                    ""
                } else {
                    "g group · "
                },
                hint(ListAction::PageUp),
                hint(ListAction::PageDown),
                hint(ListAction::Cancel)
            )
        };
        let control_line = |line: &str| {
            let mut spans = Vec::new();
            for (index, control) in line.split(" · ").enumerate() {
                if index > 0 {
                    spans.push(" · ".into());
                }
                let (keys, description) = control.rsplit_once(' ').unwrap_or((control, ""));
                #[expect(
                    clippy::disallowed_methods,
                    reason = "Analytics shortcut keys intentionally use white to distinguish them from descriptions."
                )]
                spans.push(keys.to_owned().white().bold().not_dim());
                if !description.is_empty() {
                    spans.push(format!(" {description}").into());
                }
            }
            Line::from(spans)
        };
        let mut footer = word_wrap_lines(controls.lines().map(control_line), RtOptions::new(width));
        if self.group_picker.is_none()
            && matches!(
                self.section,
                Section::Plugins | Section::Activity | Section::Skills
            )
            && !self.day_has_details(self.section, self.sections[self.section].cursor)
        {
            let height = footer.len();
            let controls =
                controls.replace(&format!(" · {} details", hint(ListAction::Accept)), "");
            footer = word_wrap_lines(controls.lines().map(control_line), RtOptions::new(width));
            // Removing a contextual hint must not resize the chart on narrow terminals.
            footer.resize(height, Line::default());
        }
        let sections = if self.zoomed {
            let line = Line::from(
                self.visible_sections()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, section)| {
                        let label = self.section_title(section);
                        let label = if width >= 64 {
                            format!("{} {label}", index + 1)
                        } else {
                            label.to_string()
                        };
                        if section == self.section {
                            format!("[{label}]  ").set_style(accent_style())
                        } else {
                            format!("{label}  ").set_style(secondary_style())
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            word_wrap_lines([line], RtOptions::new(width))
        } else {
            Vec::new()
        };
        let [title, range, tabs, body, hints] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(if show_range { 2 } else { 1 }),
            Constraint::Length(if sections.is_empty() {
                0
            } else {
                sections.len() as u16 + 1
            }),
            Constraint::Min(0),
            Constraint::Length(footer.len() as u16),
        ])
        .areas(Rect {
            x: area.x + area.width.min(/*other*/ 2),
            width: area.width.saturating_sub(/*rhs*/ 4),
            ..area
        });
        Line::from(vec![
            "Analytics".bold(),
            self.live
                .as_ref()
                .and_then(|live| live.account_label())
                .map(|account| format!("  ·  Live account · {account}"))
                .unwrap_or_else(|| "  ·  Live account".into())
                .set_style(secondary_style()),
        ])
        .render(title, buf);
        if show_range {
            let dates = self.date_range();
            let (start, end) = (dates.start(), dates.end());
            Line::from(vec![
                if self.ranges[self.range_group(self.section) as usize] == 0 {
                    "[7d]".set_style(accent_style())
                } else {
                    "7d".into()
                },
                "  ".into(),
                if self.ranges[self.range_group(self.section) as usize] == 1 {
                    "[30d]".set_style(accent_style())
                } else {
                    "30d".into()
                },
                format!(
                    "  ·  {}–{}",
                    data::date(&start.to_string()),
                    end.format("%b %-d, %Y")
                )
                .set_style(secondary_style()),
            ])
            .render(range, buf);
        }
        Paragraph::new(sections).render(tabs, buf);
        self.viewport_height = usize::from(body.height).max(/*other*/ 1);
        let (lines, selection) = if self.visible_sections().is_empty() {
            let message = self
                .account
                .message()
                .unwrap_or("Analytics is not available for this account type.");
            (vec![Line::from(message.to_string())], 0..1)
        } else if self.zoomed {
            let chart_height =
                (usize::from(body.height) / 5).clamp(/*min*/ 4, /*max*/ 10);
            let mut panel = self.panel(self.section, width, chart_height);
            if panel.chart_bands > 0 {
                let overhead = panel.lines.len() - chart_height * panel.chart_bands;
                let height =
                    usize::from(body.height).saturating_sub(overhead + 1) / panel.chart_bands;
                let height = height.max(/*other*/ 2);
                if height != chart_height {
                    panel = self.panel(self.section, width, height);
                }
            }
            let selection = if panel.lines.len() <= usize::from(body.height) {
                0..panel.lines.len()
            } else {
                panel.selection
            };
            panel.lines.push(Line::default());
            (panel.lines, selection)
        } else {
            self.dashboard_lines(width, usize::from(body.height))
        };
        if self.follow_selection {
            if selection.start < self.scroll_offset {
                self.scroll_offset = selection.start;
            } else {
                let end = selection.start + selection.len().min(self.viewport_height);
                if end > self.scroll_offset + self.viewport_height {
                    self.scroll_offset = end - self.viewport_height;
                }
            }
        }
        self.scroll_offset = self
            .scroll_offset
            .min(lines.len().saturating_sub(self.viewport_height));
        self.follow_selection = false;
        Paragraph::new(lines)
            .scroll(/*offset*/ (self.scroll_offset as u16, 0))
            .render(body, buf);
        Paragraph::new(footer)
            .set_style(secondary_style())
            .render(hints, buf);
    }
}
