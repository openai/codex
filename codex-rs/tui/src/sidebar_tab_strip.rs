//! Draw sidebar tabs and hit-test the same visible cells. When space runs out,
//! keep the selected tab visible and reserve the add button at the right edge.

use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Widget;

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

pub(crate) enum TabTarget<Id> {
    Tab(Id),
    Overflow { tab: Id, glyph: &'static str },
    Add,
}

impl<Id: Copy> TabTarget<Id> {
    pub(crate) fn tab_id(&self) -> Option<Id> {
        match self {
            Self::Tab(id) | Self::Overflow { tab: id, .. } => Some(*id),
            Self::Add => None,
        }
    }
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
        let overflow = self.ids.len() > usize::from((available / self.tab_width).max(1));
        // Reserve both arrow gutters so the tabs stay in place while switching.
        let gutter = if overflow && available >= 7 { 2 } else { 0 };
        let space = available - 2 * gutter;
        let tab_width = space.min(self.tab_width);
        let count = usize::from(space.checked_div(tab_width).unwrap_or_default());
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
                rect(gutter + i as u16 * tab_width, tab_width),
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
        hovered_tab: Option<Id>,
        paint: impl Fn(Id, Rect, Style, &mut Buffer),
    ) {
        let style = |state| match state {
            TabState::Active => crate::style::selection_style().not_underlined(),
            TabState::Inactive => crate::bottom_pane::active_tab_style(),
            TabState::Absent => crate::style::secondary_text_style()
                .bg(Color::Reset)
                .not_underlined()
                .not_reversed(),
        };
        for (target, rect) in self.strip_targets(area) {
            let quiet = if hovered_tab.is_some() && target.tab_id() == hovered_tab {
                let hover = crate::style::quiet_workspace_style()
                    .fg(Color::Reset)
                    .not_dim();
                if hover.bg == Some(Color::Reset) {
                    hover.underlined()
                } else {
                    hover
                }
            } else {
                style(TabState::Absent)
            };
            match target {
                TabTarget::Tab(id) => {
                    let tab_style =
                        if Some(id) == self.selected && selected_state != TabState::Absent {
                            style(selected_state)
                        } else {
                            quiet
                        };
                    paint(id, rect, tab_style, buf);
                }
                TabTarget::Overflow { glyph, .. } => {
                    Line::from(glyph).style(quiet).render(rect, buf)
                }
                TabTarget::Add => Line::from(" + ").style(style(add_state)).render(rect, buf),
            }
        }
    }
}
