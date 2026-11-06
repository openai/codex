//! Draw sidebar tabs and hit-test the same visible cells. When space runs out,
//! keep the selected tab visible and reserve the add button at the right edge.

use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
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
    Add,
}

impl<'a, Id: Copy + Eq> TabStrip<'a, Id> {
    pub(crate) fn selected_index(self) -> Option<usize> {
        self.ids.iter().position(|id| Some(*id) == self.selected)
    }

    pub(crate) fn strip_targets(
        self,
        area: Rect,
    ) -> (impl Iterator<Item = (usize, Id, Rect)> + 'a, Rect) {
        let width = area.width.saturating_sub(1);
        let plus = width.min(3);
        let available = width - plus;
        let height = area.height.min(1);
        let tab_width = available.min(self.tab_width);
        let count = usize::from(available.checked_div(tab_width).unwrap_or_default());
        let first = self
            .selected_index()
            .map_or(0, |index| (index + 1).saturating_sub(count));
        let tabs = self.ids.iter().copied().enumerate().skip(first).take(count);
        let targets = tabs.map(move |(index, id)| {
            let x = area.x + (index - first) as u16 * tab_width;
            (index, id, Rect::new(x, area.y, tab_width, height))
        });
        (targets, Rect::new(area.x + available, area.y, plus, height))
    }

    pub(crate) fn hit(self, area: Rect, point: Position) -> Option<TabTarget<Id>> {
        let (mut tabs, add) = self.strip_targets(area);
        if add.contains(point) {
            Some(TabTarget::Add)
        } else {
            tabs.find_map(|(_, id, rect)| rect.contains(point).then_some(TabTarget::Tab(id)))
        }
    }

    pub(crate) fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        add_state: TabState,
        selected_state: TabState,
        hovered_tab: Option<Id>,
        label: impl Fn(usize, Id) -> Line<'a>,
    ) {
        let (tabs, add) = self.strip_targets(area);
        let style = |state| match state {
            TabState::Active => crate::style::selection_style().not_underlined(),
            TabState::Inactive => crate::bottom_pane::active_tab_style(),
            TabState::Absent => crate::style::secondary_text_style()
                .bg(Color::Reset)
                .not_underlined()
                .not_reversed(),
        };
        for (index, id, rect) in tabs {
            let tab_style = if selected_state != TabState::Absent && Some(id) == self.selected {
                style(selected_state)
            } else if hovered_tab == Some(id) {
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
            label(index, id).style(tab_style).render(rect, buf);
        }
        Line::from(" + ").style(style(add_state)).render(add, buf);
    }
}
