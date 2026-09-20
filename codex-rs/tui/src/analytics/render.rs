//! Fixed analytics navigation and keyboard help around existing report panels.
//! Tabs and hints never wrap or move when a report loads, changes range, or becomes empty.

use super::AnalyticsView;
use super::styles::secondary_style;
use crate::analytics::models::AccountAnalyticsHistory;
use crate::analytics::models::AccountAnalyticsValue;
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
        let inset = u16::from(area.width > 2);
        let inner = Rect {
            x: area.x + inset,
            width: area.width.saturating_sub(inset * 2),
            ..area
        };
        let width = usize::from(inner.width).max(/*other*/ 1);
        let [title, tabs, rule, controls, body, hints] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(if width >= 100 { 2 } else { 3 }),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .areas(inner);
        let account = self.live.as_ref().and_then(|live| live.account_label());
        let plan = self.account.ready().map(|plan| {
            use codex_protocol::account::PlanType;
            match plan {
                PlanType::Free => "Free",
                PlanType::Go => "Go",
                PlanType::Plus => "Plus",
                PlanType::Pro => "Pro",
                PlanType::ProLite => "Pro Lite",
                PlanType::Team
                | PlanType::Business
                | PlanType::SelfServeBusinessProLite
                | PlanType::SelfServeBusinessUsageBased => "Business",
                PlanType::Enterprise
                | PlanType::Ent26
                | PlanType::EnterpriseCbpAutomation
                | PlanType::EnterpriseCbpUsageBased => "Enterprise",
                PlanType::Edu | PlanType::EduPlus | PlanType::EduPro => "Education",
                PlanType::Unknown => "Account",
            }
        });
        let heading = Line::from(vec![
            plan.map(|plan| format!("Usage · {plan}"))
                .unwrap_or_else(|| "Usage".into())
                .bold(),
            account
                .map(|account| format!(" · {account}"))
                .unwrap_or_default()
                .set_style(secondary_style()),
        ]);
        crate::line_truncation::truncate_line_with_ellipsis_if_overflow(heading, width)
            .render(title, buf);
        let visible = self.visible_sections();
        let labels = visible
            .iter()
            .enumerate()
            .map(|(index, section)| {
                if width >= 80 {
                    format!("{} {}", index + 1, self.tab_label(*section))
                } else {
                    self.tab_label(*section).to_owned()
                }
            })
            .collect::<Vec<_>>();
        let selected = visible
            .iter()
            .position(|section| *section == self.section)
            .unwrap_or_default();
        crate::bottom_pane::render_filled_tab_bar(
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            selected,
            tabs,
            buf,
        );
        Line::from("─".repeat(width)).dim().render(rule, buf);
        self.render_controls(controls, buf);
        self.viewport_height = usize::from(body.height).max(/*other*/ 1);
        let (lines, selection) = if self.show_help {
            (self.help_lines(width), 0..1)
        } else if visible.is_empty() {
            let message = self
                .account
                .message()
                .unwrap_or("Analytics is not available for this account type.");
            (
                textwrap::wrap(message, width)
                    .into_iter()
                    .map(|line| Line::from(line.into_owned()))
                    .collect(),
                0..1,
            )
        } else if !self.zoomed {
            self.dashboard_lines(width, self.viewport_height)
        } else {
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
            let center_summary =
                self.section == super::sections::Section::Summary && self.profile.ready().is_some();
            if center_summary {
                let padding = usize::from(body.height).saturating_sub(panel.lines.len()) / 2;
                panel
                    .lines
                    .splice(0..0, std::iter::repeat_n(Line::default(), padding));
            }
            let selection = if panel.lines.len() <= usize::from(body.height) {
                0..panel.lines.len()
            } else {
                panel.selection
            };
            if !center_summary && panel.lines.len() > usize::from(body.height) {
                panel.lines.push(Line::default());
            }
            (panel.lines, selection)
        };
        let mut scroll_offset = self.scroll_offset();
        if self.follow_selection {
            if selection.start < scroll_offset {
                scroll_offset = selection.start;
            } else {
                let end = selection.start + selection.len().min(self.viewport_height);
                if end > scroll_offset + self.viewport_height {
                    scroll_offset = end - self.viewport_height;
                }
            }
        }
        let scrollable = lines.len() > self.viewport_height;
        scroll_offset = scroll_offset.min(lines.len().saturating_sub(self.viewport_height));
        *self.scroll_offset_mut() = scroll_offset;
        self.follow_selection = false;
        Paragraph::new(lines)
            .scroll(
                /*offset*/ (scroll_offset.min(usize::from(u16::MAX)) as u16, 0),
            )
            .render(body, buf);
        Paragraph::new(self.footer_lines(width, scrollable))
            .style(secondary_style())
            .render(hints, buf);
    }
}
