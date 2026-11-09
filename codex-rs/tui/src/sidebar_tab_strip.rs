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

/// Adjacent tabs have one unpainted, noninteractive terminal column between them.
const TAB_GAP: u16 = 1;

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
    pub(crate) tab_width: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabTarget<Id> {
    Tab(Id),
    Overflow { tab: Id, glyph: &'static str },
    Add,
}

impl<'a, Id: Copy + Eq> TabStrip<'a, Id> {
    pub(crate) fn selected_index(self) -> Option<usize> {
        self.ids.iter().position(|id| Some(*id) == self.selected)
    }

    pub(crate) fn strip_targets(
        self,
        area: Rect,
    ) -> impl Iterator<Item = (TabTarget<Id>, Rect)> + 'a {
        let width = area.width.saturating_sub(1);
        let plus = width.min(3);
        let available = width - plus;
        let height = area.height.min(1);
        let stride = self.tab_width.saturating_add(TAB_GAP);
        let overflow = self.ids.len() > usize::from(((available + TAB_GAP) / stride).max(1));
        // Reserve both arrow gutters so the tabs stay in place while switching.
        let gutter = if overflow && available >= 7 { 2 } else { 0 };
        let space = available - 2 * gutter;
        let tab_width = space.min(self.tab_width);
        let count = if tab_width == 0 {
            0
        } else {
            usize::from((space + TAB_GAP) / (tab_width + TAB_GAP))
        };
        let first = self
            .selected_index()
            .map_or(0, |index| (index + 1).saturating_sub(count));
        let end = (first + count).min(self.ids.len());
        let rect = move |offset, width| Rect::new(area.x + offset, area.y, width, height);
        let arrow = |chat, glyph, x| (TabTarget::Overflow { tab: chat, glyph }, rect(x, 1));
        let left = (gutter > 0 && first > 0).then(|| arrow(self.ids[first - 1], "‹", 0));
        let right =
            (gutter > 0 && end < self.ids.len()).then(|| arrow(self.ids[end], "›", available - 1));
        let tabs = self.ids[first..end].iter().enumerate().map(move |(i, id)| {
            (
                TabTarget::Tab(*id),
                rect(gutter + i as u16 * (tab_width + TAB_GAP), tab_width),
            )
        });
        left.into_iter()
            .chain(tabs)
            .chain(right)
            .chain([(TabTarget::Add, rect(available, plus))])
    }

    pub(crate) fn hit(self, area: Rect, point: Position) -> Option<(TabTarget<Id>, Rect)> {
        self.strip_targets(area)
            .find(|(_, rect)| rect.contains(point))
    }

    pub(crate) fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        add_state: TabState,
        selected_state: TabState,
        hovered_target: Option<TabTarget<Id>>,
        paint: impl Fn(Id, Rect, Style, &mut Buffer),
    ) {
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
        for (target, rect) in self.strip_targets(area) {
            let hovered = Some(target) == hovered_target;
            let quiet = if hovered { hover } else { rest };
            match target {
                TabTarget::Tab(id) => {
                    let tab_style =
                        if Some(id) == self.selected && selected_state != TabState::Absent {
                            selected_style
                        } else {
                            quiet
                        };
                    paint(id, rect, tab_style, buf);
                }
                TabTarget::Overflow { glyph, .. } => {
                    Line::from(glyph).style(quiet).render(rect, buf)
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
    }
}
