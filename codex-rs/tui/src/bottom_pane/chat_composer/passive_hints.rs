//! Fit whole composer hints with navigation and actions on the left and indicators on the right.
//! Request a second row only when a compact input mode (Shell, Vim, Plan) and a
//! current action (queue or active side conversation) fit separately but not together.
//! In two rows, navigation and modes stay above actions and the remaining indicators.
//! Shorten or drop labels in collapse order. If a row still overflows, refill it
//! in interaction priority order, then paint the surviving labels in reading order.

use super::*;
use crate::bottom_pane::footer::goal_status_indicator_line;
use crate::bottom_pane::footer::mode_indicator_line;
use crate::line_truncation::line_width;
use crate::style::secondary_text_style;
use ratatui::style::Styled;

// Declaration order is the reading order within each side.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Agents,
    Shortcuts,
    Action,
    Side,
    SideNav,
    AgentName,
    Shell,
    Vim,
    Plan,
    Goal,
    Ide,
    Context,
    Warning,
}

use Kind::*;

impl Kind {
    fn is_input_mode(self) -> bool {
        matches!(self, Shell | Vim | Plan)
    }

    fn left(self) -> bool {
        matches!(self, Agents | Shortcuts | Action | Side | SideNav)
    }

    fn row(self, two_rows: bool) -> usize {
        usize::from(
            two_rows && !self.is_input_mode() && !matches!(self, Agents | Shortcuts | SideNav),
        )
    }
}

fn joined_width(first: usize, second: usize) -> usize {
    first + second + usize::from(first > 0 && second > 0) * 3
}

struct Label {
    kind: Kind,
    forms: Vec<(usize, Line<'static>)>,
    selected: Option<usize>,
}

impl Label {
    fn width(&self) -> usize {
        self.selected
            .and_then(|i| self.forms.get(i))
            .map_or(0, |(cells, _)| *cells)
    }
}

#[derive(Default)]
pub(super) struct Hints {
    labels: Vec<Label>,
}

#[derive(Default, Clone, Copy)]
struct Row {
    left: usize,
    right: usize,
}

impl Row {
    fn width(self) -> usize {
        self.left + self.right + usize::from(self.left > 0 && self.right > 0)
    }
}

impl Hints {
    fn add(&mut self, kind: Kind, forms: impl IntoIterator<Item = Line<'static>>) {
        let forms: Vec<_> = forms
            .into_iter()
            .map(|line| (line_width(&line), line))
            .filter(|(cells, _)| *cells > 0)
            .collect();
        if !forms.is_empty() {
            self.labels.push(Label {
                kind,
                forms,
                selected: Some(0),
            });
        }
    }

    pub(super) fn desired_rows(&self, width: u16) -> u16 {
        let width = usize::from(width);
        let mut compact = Row::default();
        let mut any_fits = false;
        for label in &self.labels {
            let Some(&(cells, _)) = label.forms.last() else {
                continue;
            };
            any_fits |= cells <= width;
            let group = if label.kind.is_input_mode() {
                &mut compact.right
            } else if matches!(label.kind, Action | Side) {
                &mut compact.left
            } else {
                continue;
            };
            let next = joined_width(*group, cells);
            if next <= width {
                *group = next;
            }
        }
        u16::from(any_fits) + u16::from(compact.width() > width)
    }

    fn placement(&self, two_rows: bool) -> [Row; 2] {
        let mut rows = [Row::default(); 2];
        for label in &self.labels {
            let row = &mut rows[label.kind.row(two_rows)];
            let group = if label.kind.left() {
                &mut row.left
            } else {
                &mut row.right
            };
            *group = joined_width(*group, label.width());
        }
        rows
    }

    fn fit(&mut self, width: usize, two: bool) -> [Row; 2] {
        let mut p = self.placement(two);
        for (kind, form) in [
            (Shortcuts, None),
            (Plan, Some(1)),
            (Warning, Some(1)),
            (Ide, Some(1)),
            (Action, Some(1)),
            (Warning, Some(2)),
            (Agents, None),
        ] {
            if p[kind.row(two)].width() <= width {
                continue;
            }
            let Some(label) = self.labels.iter_mut().find(|label| label.kind == kind) else {
                continue;
            };
            if let Some(i) = form
                && label
                    .forms
                    .get(i)
                    .is_none_or(|(cells, _)| *cells >= label.width())
            {
                continue;
            }
            label.selected = form;
            p = self.placement(two);
        }
        let overflow = p.map(|row| row.width() > width);
        if !overflow.contains(&true) {
            return p;
        }
        // Refill only overflowing rows in interaction priority order.
        for label in &mut self.labels {
            if overflow[label.kind.row(two)] {
                label.selected = None;
            }
        }
        for kind in [
            Action, Side, Shell, Vim, Plan, Goal, Ide, Warning, Context, SideNav, AgentName,
            Agents, Shortcuts,
        ] {
            if !overflow[kind.row(two)] {
                continue;
            }
            if let Some(i) = self.labels.iter().position(|label| label.kind == kind) {
                self.labels[i].selected = Some(self.labels[i].forms.len() - 1);
                if self.placement(two)[kind.row(two)].width() > width {
                    self.labels[i].selected = None;
                }
            }
        }
        self.placement(two)
    }

    pub(super) fn render(mut self, area: Rect, buf: &mut Buffer) -> Option<Rect> {
        if area.is_empty() {
            return None;
        }
        let two = area.height > 1;
        let rows = self.fit(usize::from(area.width), two);
        self.labels.sort_by_key(|label| label.kind);
        let mut cursors = rows.map(|row| [area.x, area.right() - row.right as u16]);
        let mut warning = None;
        for label in self.labels {
            let Some((cells, line)) = label.selected.map(|i| &label.forms[i]) else {
                continue;
            };
            let row = label.kind.row(two);
            let x = &mut cursors[row][usize::from(!label.kind.left())];
            let y = area.y + row as u16;
            // Keep the meter's extra right inset when it is alone on its side and space permits.
            let inset = label.kind == Context
                && rows[row].right == *cells
                && rows[row].width() < usize::from(area.width);
            let rect = Rect::new(*x - u16::from(inset), y, *cells as u16, 1);
            Widget::render(line, rect, buf);
            if label.kind == Warning {
                warning = Some(rect);
            }
            *x += *cells as u16;
            let end = if label.kind.left() {
                area.x + rows[row].left as u16
            } else {
                area.right()
            };
            if *x < end {
                buf.set_string(*x, y, " · ", secondary_text_style());
                *x += 3;
            }
        }
        warning
    }
}

fn shortcut(key: key_hint::ShortcutHint, action: &'static str) -> Line<'static> {
    let mut line = Line::from(key.spans());
    line.push_span(action.set_style(crate::style::secondary_text_style()));
    line
}

pub(super) fn hint_area(area: Rect) -> Rect {
    let mut area = inset_footer_hint_area(area);
    area.width = area.width.saturating_sub(1);
    area
}

impl ChatComposer {
    pub(super) fn passive_hints(&self, options: ComposerRenderOptions<'_>) -> Option<Hints> {
        let mode = self.footer_mode();
        if options.footer.is_some()
            || self.custom_footer_height().is_some()
            || !matches!(
                mode,
                FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
            )
        {
            return None;
        }
        let mut hints = Hints::default();
        hints.add(Shell, self.shell_mode_footer_line());
        hints.add(Vim, self.vim_mode_indicator_span().map(Line::from));
        let can_navigate = self.has_focus && self.draft.input_enabled && !self.popup_active();
        let can_type = can_navigate && !self.blocks_direct_input;
        let show_cycle = can_type && !self.is_task_running;
        if let Some(plan) = self.footer.collaboration_mode_indicator {
            hints.add(
                Plan,
                [
                    mode_indicator_line(Some(plan), show_cycle),
                    mode_indicator_line(Some(plan), /*show_cycle_hint*/ false),
                ]
                .into_iter()
                .flatten(),
            );
        } else {
            hints.add(
                Goal,
                goal_status_indicator_line(self.footer.goal_status_indicator.as_ref()),
            );
        }
        if self.footer.ide_context_active {
            hints.add(
                Ide,
                [Line::from("IDE context".cyan()), Line::from("IDE".cyan())],
            );
        }
        if mode == FooterMode::ComposerHasDraft
            && can_type
            && self.is_task_running
            && let Some(key) = self.footer.queue_key
        {
            hints.add(
                Action,
                [
                    shortcut(key, " to queue message"),
                    shortcut(key, " to queue"),
                ],
            );
        }
        if let Some(label) = &self.footer.side_conversation_context_label {
            let kind = if self.side_conversation_active {
                Side
            } else {
                SideNav
            };
            hints.add(kind, [side_conversation_context_line(label)]);
        }
        if !self.footer.status_line_enabled
            && let Some(agent) = &self.footer.active_agent_label
        {
            hints.add(
                AgentName,
                [Line::from(agent.clone().set_style(secondary_text_style()))],
            );
        }
        if mode == FooterMode::ComposerEmpty && can_navigate && !self.is_in_paste_burst() {
            if self.agents_navigation_available() {
                hints.add(
                    Agents,
                    [shortcut(
                        key_hint::plain(KeyCode::Left).into(),
                        " for agents",
                    )],
                );
            }
            if let Some(key) = self.footer.toggle_shortcuts_key {
                hints.add(Shortcuts, [shortcut(key, " for shortcuts")]);
            }
        }
        let show_warning = self.show_warning_notice(options);
        if !self.footer.status_line_enabled
            && !self.footer.context_window_hidden
            && self.footer.side_conversation_context_label.is_none()
            && !self.is_bang_shell_command()
        {
            if hints.labels.is_empty() && !show_warning {
                // A lone context meter retains its existing inset and narrow-width clipping.
                return None;
            }
            hints.add(
                Context,
                [context_window_line(
                    self.footer.context_window_percent,
                    self.footer.context_window_used_tokens,
                )],
            );
        }
        if show_warning {
            hints.add(Warning, self.warning_labels(options.warning_count));
        }
        Some(hints)
    }
}
