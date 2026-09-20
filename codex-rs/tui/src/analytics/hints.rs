//! Two fixed hint rows keep report actions visible without changing chart geometry.

use super::AnalyticsView;
use super::sections::Section;
use crate::key_hint::key_label_spans;
use crate::keymap::ListAction;
use ratatui::style::Stylize;
use ratatui::text::Line;

impl AnalyticsView {
    pub(super) fn footer_lines(&self, width: usize, scrollable: bool) -> Vec<Line<'static>> {
        let pair = |a, b| format!("{}/{}", self.hint(a), self.hint(b));
        let mut actions = Vec::new();
        if self.show_help {
            actions.push((self.hint(ListAction::Cancel), "back"));
        } else if !self.zoomed {
            actions.push((self.hint(ListAction::Accept), "focus"));
            actions.push(("tab".into(), "card"));
        } else {
            match self.section {
                Section::Summary => {}
                Section::Plan => {
                    actions.push((pair(ListAction::MoveUp, ListAction::MoveDown), "period"));
                }
                Section::Chats => {
                    actions.push((pair(ListAction::MoveUp, ListAction::MoveDown), "row"));
                }
                Section::Usage
                | Section::Credits
                | Section::Activity
                | Section::Plugins
                | Section::Skills => {
                    actions.push((pair(ListAction::MoveLeft, ListAction::MoveRight), "day"));
                }
            }
            let plan_details = self.section == Section::Plan
                && self
                    .plan
                    .report
                    .ready()
                    .and_then(|report| {
                        report.periods[self.plan.window].get(self.plan.cursor[self.plan.window])
                    })
                    .is_some();
            if plan_details
                || self.day_has_details(self.section, self.sections[self.section].cursor)
                || (self.section == Section::Chats && self.chat_has_details())
            {
                actions.push((self.hint(ListAction::Accept), "details"));
            }
        }
        if scrollable {
            actions.push((pair(ListAction::PageUp, ListAction::PageDown), "scroll"));
        }
        if !self.show_help {
            actions.push((self.hint(ListAction::Cancel), "back"));
        }
        let mut navigation = if self.show_help {
            vec![("?".into(), "close help")]
        } else if self.visible_sections().is_empty() {
            vec![("R".into(), "retry")]
        } else {
            vec![
                (
                    if width >= 80 {
                        format!("tab/1–{}", self.visible_sections().len())
                    } else {
                        "tab".into()
                    },
                    "report",
                ),
                (
                    "z".into(),
                    if !self.zoomed {
                        "focus"
                    } else if width < 50 {
                        "all"
                    } else {
                        "dashboard"
                    },
                ),
                ("?".into(), if width < 50 { "" } else { "help" }),
                ("R".into(), "refresh"),
            ]
        };
        if !self.help_shortcut_available() {
            navigation.retain(|(key, _)| key != "?");
        }
        [actions, navigation]
            .into_iter()
            .map(|hints| {
                let mut line = Line::default();
                for (key, label) in hints {
                    if key.is_empty() {
                        continue;
                    }
                    let separator = if line.spans.is_empty() { "" } else { " · " };
                    let mut part = Line::from(separator.dim());
                    part.spans.extend(key_label_spans(&key));
                    if !label.is_empty() {
                        part.spans.push(format!(" {label}").dim());
                    }
                    if line.width() + part.width() <= width {
                        line.spans.extend(part.spans);
                    }
                }
                line
            })
            .collect()
    }
}
