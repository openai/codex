//! Visible report controls and update times. Mouse targets and keyboard shortcuts invoke the same semantic actions.
//! Their reserved rows depend only on terminal width, so loading never shifts the report body.

use super::AnalyticsView;
use super::sections::Section;
use super::styles::secondary_style;
use crate::key_hint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow as truncate;
use crate::style::accent_style;
use chrono::Datelike;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

/// A mouse action is independent of a user's remapped or disabled arrow bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Range,
    Group,
    Model,
    TaskMetric,
    Dashboard,
    PlanWindow,
    ZeroCreditGroups,
}

impl AnalyticsView {
    /// Share action eligibility between painted controls, keyboard dispatch and help.
    pub(super) fn control_available(&self, control: Control) -> bool {
        if self.visible_sections().is_empty() {
            return false;
        }
        match control {
            Control::Range => self.section.report().is_some(),
            Control::Group => self.group_options().len() > 1,
            Control::Model => self.section == Section::Usage && self.business(),
            Control::TaskMetric => self.section == Section::Chats && !self.business(),
            Control::Dashboard => true,
            Control::PlanWindow => self.section == Section::Plan,
            Control::ZeroCreditGroups => {
                self.section == Section::Chats
                    && self.business()
                    && self.sections[Section::Chats].detail.is_some()
            }
        }
    }

    /// Apply a visible report action without synthesizing a remappable keyboard event.
    pub(super) fn activate_control(&mut self, control: Control) {
        if !self.control_available(control) {
            return;
        }
        // Labels can change width; wait for their new painted positions before another click.
        self.invalidate_mouse_targets();
        match control {
            Control::ZeroCreditGroups => {
                self.show_zero_credit_groups = !self.show_zero_credit_groups
            }
            Control::Range => self.change_range(),
            Control::Dashboard => self.toggle_dashboard(),
            Control::PlanWindow => {
                self.plan_action(Some(crate::keymap::ListAction::MoveRight));
                self.follow_selection = true;
            }
            Control::Model => {
                let models = self
                    .live
                    .as_ref()
                    .map_or_else(Vec::new, |live| live.token_models());
                self.token_model = match self
                    .token_model
                    .as_ref()
                    .and_then(|model| models.iter().position(|candidate| candidate == model))
                {
                    Some(index) => models.get(index + 1).cloned(),
                    None => models.first().cloned(),
                };
                self.load_report(Section::Usage);
            }
            Control::Group => {
                let options = self.group_options();
                let next = options
                    .iter()
                    .position(|group| *group == self.sections[self.section].group)
                    .map_or(/*default*/ 0, |index| (index + 1) % options.len());
                self.sections[self.section].group = options[next];
                self.load_report(self.section);
            }
            Control::TaskMetric => {
                let metrics = self.task_metrics();
                let index = metrics
                    .iter()
                    .position(|metric| *metric == self.task_metric())
                    .unwrap_or_default();
                self.chat_metric = metrics[(index + 1) % metrics.len()];
                self.sections[Section::Chats].detail = None;
            }
        }
    }

    pub(super) fn render_controls(&mut self, area: Rect, buf: &mut Buffer) {
        self.control_hits.clear();
        if area.is_empty() {
            return;
        }
        if self.show_help {
            "Usage shortcuts".bold().render(area, buf);
            return;
        }
        if self.visible_sections().is_empty() {
            return;
        }
        let width = usize::from(area.width);
        let mut rows = vec![Vec::new(), Vec::new()];
        let report = self.control_available(Control::Range);
        if report {
            let range = self.ranges[self.range_group(self.section) as usize];
            let dates = self.section_date_range(self.section);
            let interval = if dates.start().year() != dates.end().year() && width < 50 {
                format!(
                    "{}–{}",
                    dates.start().format("%m/%d/%Y"),
                    dates.end().format("%m/%d/%Y")
                )
            } else if dates.start().year() == dates.end().year() {
                format!(
                    "{}–{}",
                    dates.start().format("%b %-d"),
                    dates.end().format("%b %-d, %Y")
                )
            } else {
                format!(
                    "{}–{}",
                    dates.start().format("%b %-d, %Y"),
                    dates.end().format("%b %-d, %Y")
                )
            };
            rows[0].push((
                Control::Range,
                Line::from(vec![
                    key_hint::plain(KeyCode::Char('r')).into(),
                    " ".dim(),
                    if range == 0 {
                        "7d".set_style(accent_style()).bold()
                    } else {
                        "7d".dim()
                    },
                    "/".dim(),
                    if range == 1 {
                        "30d".set_style(accent_style()).bold()
                    } else {
                        "30d".dim()
                    },
                    format!(" · {interval}").into(),
                ]),
            ));
        }
        let option_row = usize::from(width < 100 && report);
        if self.control_available(Control::Group) {
            rows[option_row].push((
                Control::Group,
                Line::from(vec![
                    key_hint::plain(KeyCode::Char('g')).into(),
                    " ".dim(),
                    self.group_label(self.section, self.sections[self.section].group)
                        .to_owned()
                        .into(),
                ]),
            ));
        }
        if self.control_available(Control::Model) {
            rows[option_row].push((
                Control::Model,
                Line::from(vec![
                    key_hint::plain(KeyCode::Char('m')).into(),
                    " ".dim(),
                    self.token_model
                        .clone()
                        .unwrap_or_else(|| "All models".into())
                        .into(),
                ]),
            ));
        }
        if self.control_available(Control::TaskMetric) {
            rows[0].push((
                Control::TaskMetric,
                Line::from(vec![
                    key_hint::plain(KeyCode::Char('s')).into(),
                    " sort: ".dim(),
                    super::task_panel::METRICS[self.task_metric()].into(),
                ]),
            ));
        }
        if self.control_available(Control::PlanWindow) {
            let row = usize::from(width < 100);
            let hints = [
                crate::keymap::ListAction::MoveLeft,
                crate::keymap::ListAction::MoveRight,
            ]
            .into_iter()
            .filter_map(|action| self.keymap.primary_hint(action))
            .map(key_hint::ShortcutHint::display_label)
            .collect::<Vec<_>>()
            .join("/");
            let mut plan_window = Line::default();
            if !hints.is_empty() {
                plan_window.spans = key_hint::key_label_spans(&format!("{hints} "));
            }
            plan_window.spans.extend([
                if self.plan.window == 0 {
                    "5-hour".set_style(accent_style()).bold()
                } else {
                    "5-hour".dim()
                },
                " / ".dim(),
                if self.plan.window == 1 {
                    "Weekly".set_style(accent_style()).bold()
                } else {
                    "Weekly".dim()
                },
            ]);
            rows[row].push((Control::PlanWindow, plan_window));
        }
        if !self.zoomed {
            let mut dashboard = Line::from("Dashboard".bold());
            if let Some(hint) = self.keymap.primary_hint(crate::keymap::ListAction::Accept) {
                dashboard.spans.push(" · ".dim());
                dashboard.spans.extend(hint.spans());
                dashboard.spans.push(" focus".dim());
            }
            rows[0].insert(/*index*/ 0, (Control::Dashboard, dashboard));
        }
        for (row, controls) in rows
            .into_iter()
            .enumerate()
            .take(usize::from(area.height.saturating_sub(/*rhs*/ 1)))
        {
            let mut x = area.x;
            for (key, line) in controls {
                if x > area.x {
                    " · ".dim().render(
                        Rect::new(
                            x,
                            area.y + row as u16,
                            area.right().saturating_sub(x).min(/*other*/ 3),
                            /*height*/ 1,
                        ),
                        buf,
                    );
                    x = x.saturating_add(/*rhs*/ 3);
                }
                let remaining = area.right().saturating_sub(x);
                if remaining == 0 {
                    break;
                }
                let line = truncate(line, usize::from(remaining));
                let hit = Rect::new(
                    x,
                    area.y + row as u16,
                    line.width() as u16,
                    /*height*/ 1,
                );
                line.render(hit, buf);
                if !hit.is_empty() {
                    self.control_hits.push((key, hit));
                }
                x = hit.right();
            }
        }
        let updated = self.sections[self.section]
            .history
            .ready()
            .and_then(|report| report.updated_at)
            .and_then(|value| chrono::DateTime::from_timestamp(value, /*nsecs*/ 0));
        let metadata = updated
            .map(|updated| format!("Updated · {} UTC", updated.format("%b %-d %H:%M")))
            .unwrap_or_default();
        truncate(metadata.set_style(secondary_style()).into(), width).render(
            Rect::new(area.x, area.bottom() - 1, area.width, /*height*/ 1),
            buf,
        );
    }
}
