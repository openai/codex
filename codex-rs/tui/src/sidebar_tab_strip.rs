//! Draw sidebar tabs and hit-test the same visible cells. When space runs out,
//! keep the selected tab visible and reserve the add button at the right edge.
//! A one-column gap separates tabs and is excluded from their click targets.

use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Widget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SidebarTab {
    Chat(codex_protocol::ThreadId),
    Document(uuid::Uuid),
    Subagents,
}

/// Adjacent tabs have one unpainted, noninteractive terminal column between them.
const TAB_GAP: u16 = 1;
const MIN_TAB_WIDTH: u16 = 12;
const MAX_TAB_WIDTH: u16 = 22;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabState {
    Absent,
    Active,
    Inactive,
}

#[derive(Clone, Copy)]
pub(crate) struct TabStrip<'a, Id> {
    pub(crate) ids: &'a [Id],
    pub(crate) selected: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabTarget<Id> {
    Tab(Id),
    Close(Id),
    Overflow { tab: Id, glyph: &'static str },
    Add,
}

impl<'a, Id: Copy + Eq> TabStrip<'a, Id> {
    pub(crate) fn selected_index(self) -> Option<usize> {
        self.ids.iter().position(|id| Some(*id) == self.selected)
    }

    pub(crate) fn strip_targets(self, area: Rect) -> Vec<(TabTarget<Id>, Rect)> {
        let width = area.width.saturating_sub(1);
        let plus = width.min(3);
        let available = width - plus;
        let height = area.height.min(1);
        let rect = |offset, width| Rect::new(area.x + offset, area.y, width, height);
        let mut targets = Vec::new();
        if available > 0 && !self.ids.is_empty() {
            let preferred = (area.width / 4).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
            let stride = preferred + TAB_GAP;
            let overflow = self.ids.len() > usize::from(((available + TAB_GAP) / stride).max(1));
            // Tiny strips spend their space on the title.
            let gutter = if overflow && available >= 7 { 2 } else { 0 };
            let space = available - 2 * gutter;
            let width = space.min(preferred);
            let count = usize::from((space + TAB_GAP) / (width + TAB_GAP)).min(self.ids.len());
            let start = self
                .selected_index()
                .map_or(0, |index| (index + 1).saturating_sub(count));
            let end = start + count;

            if gutter > 0 && start > 0 {
                targets.push((
                    TabTarget::Overflow {
                        tab: self.ids[start - 1],
                        glyph: "‹",
                    },
                    rect(0, 1),
                ));
            }
            let mut x = gutter;
            for &id in &self.ids[start..end] {
                let close = u16::from(width >= 3);
                let label = width - close;
                targets.push((TabTarget::Tab(id), rect(x, label)));
                if close > 0 {
                    targets.push((TabTarget::Close(id), rect(x + label, close)));
                }
                x += width + TAB_GAP;
            }
            if gutter > 0 && end < self.ids.len() {
                targets.push((
                    TabTarget::Overflow {
                        tab: self.ids[end],
                        glyph: "›",
                    },
                    rect(available - 1, 1),
                ));
            }
        }
        targets.push((TabTarget::Add, rect(available, plus)));
        targets
    }

    pub(crate) fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        add_state: TabState,
        selected_state: TabState,
        pointer: Option<Position>,
        paint: impl Fn(Id, Rect, Style, &mut Buffer),
    ) -> Vec<(TabTarget<Id>, Rect)> {
        let style = |state| match state {
            TabState::Active => crate::style::selection_style().not_underlined(),
            TabState::Inactive => crate::style::quiet_workspace_style(),
            TabState::Absent => crate::style::secondary_text_style()
                .bg(Color::Reset)
                .not_underlined()
                .not_reversed(),
        };
        let rest = style(TabState::Absent).patch(crate::style::user_message_style());
        let rest = rest.fg(crate::style::readable_color_on(
            rest.fg.unwrap_or(Color::Reset),
            rest.bg,
        ));
        let hover = crate::style::hover_style(style(TabState::Absent).fg(Color::Reset));
        let selected_style = match selected_state {
            TabState::Active => style(selected_state),
            TabState::Inactive => hover.bold(),
            TabState::Absent => rest,
        };
        let targets = self.strip_targets(area);
        for &(target, rect) in &targets {
            let hovered = pointer.is_some_and(|point| rect.contains(point));
            let quiet = if hovered { hover } else { rest };
            match target {
                TabTarget::Tab(id) => {
                    let tab_style =
                        if Some(id) == self.selected && selected_state != TabState::Absent {
                            if hovered {
                                crate::style::hover_style(selected_style)
                            } else {
                                selected_style
                            }
                        } else {
                            quiet
                        };
                    paint(id, rect, tab_style, buf);
                }
                TabTarget::Close(id) => {
                    let close = if Some(id) == self.selected && selected_state != TabState::Absent {
                        if hovered {
                            crate::style::hover_style(selected_style)
                        } else {
                            selected_style
                        }
                    } else {
                        quiet
                    };
                    Line::from("×").style(close).render(rect, buf);
                }
                TabTarget::Overflow { glyph, .. } => {
                    let arrow = if hovered {
                        hover
                    } else {
                        style(TabState::Absent)
                    };
                    Line::from(glyph).style(arrow).render(rect, buf)
                }
                TabTarget::Add => {
                    let add = match (hovered, add_state) {
                        (true, TabState::Active) => crate::style::hover_style(style(add_state)),
                        (true, TabState::Inactive | TabState::Absent) => hover,
                        (false, TabState::Active | TabState::Inactive | TabState::Absent) => {
                            style(add_state)
                        }
                    };
                    Line::from(" + ").style(add).render(rect, buf);
                }
            }
        }
        targets
    }
}
