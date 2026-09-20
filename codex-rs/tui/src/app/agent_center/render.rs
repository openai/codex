//! Task rows, metadata editing and read-only details share a wide/narrow layout.

use super::hints::hint_line;
use super::*;
use crate::bottom_pane::render_filled_tab_bar;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn row(area: Rect, offset: u16, height: u16) -> Rect {
    let offset = offset.min(area.height);
    Rect::new(
        area.x,
        area.y + offset,
        area.width,
        height.min(area.height - offset),
    )
}

pub(super) fn line(text: impl Into<Line<'static>>, area: Rect, buf: &mut Buffer) {
    if !area.is_empty() {
        truncate_line_with_ellipsis_if_overflow(text.into(), usize::from(area.width))
            .render(row(area, /*offset*/ 0, /*height*/ 1), buf);
    }
}

fn visible_suffix(input: &str, available: usize) -> &str {
    let mut start = input.len();
    let mut visible_width = 0;
    for (offset, grapheme) in input.grapheme_indices(/*is_extended*/ true).rev() {
        let width = grapheme.width();
        if visible_width + width > available {
            break;
        }
        visible_width += width;
        start = offset;
    }
    &input[start..]
}

struct CenterLayout {
    header: Rect,
    list: Rect,
    gap: Rect,
    details: Rect,
    search: Rect,
    footer: Rect,
}

impl AgentsOverviewView {
    fn center_layout(&self, area: Rect) -> CenterLayout {
        let footer_height = (self.footer_lines(area.width.saturating_sub(4)).len() as u16)
            .min(area.height.saturating_sub(4));
        let state = self.state();
        let header_height = if area.height >= 6 { 3 } else { 0 };
        let footer = row(area, area.height - footer_height, footer_height);
        let header = row(area, /*offset*/ 0, header_height);
        let body = row(area, header_height, footer.y - header.bottom())
            .inner(Margin::new(/*horizontal*/ 2, /*vertical*/ 0));
        let [list, gap, details] = if body.width >= 90 {
            Layout::horizontal([
                Constraint::Min(46),
                Constraint::Length(3),
                Constraint::Length(38),
            ])
            .areas(body)
        } else {
            [body, Rect::default(), Rect::default()]
        };
        let search = row(
            list,
            /*offset*/ 0,
            u16::from(list.height >= 3 || state.editing_metadata()),
        );
        let list = row(
            list,
            search.height,
            list.height.saturating_sub(search.height),
        );
        CenterLayout {
            header,
            list,
            gap,
            details,
            search,
            footer,
        }
    }
}

impl Renderable for AgentsOverviewView {
    fn desired_height(&self, _width: u16) -> u16 {
        24
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let layout = self.center_layout(area);
        let state = self.state();
        if state.editing_metadata() && !layout.search.is_empty() {
            let input = if state.rename_target.is_some() {
                &state.input
            } else {
                &state.search
            };
            let prefix = if state.rename_target.is_some() {
                "Rename › "
            } else {
                "Search › "
            };
            let available = usize::from(layout.search.width).saturating_sub(prefix.width() + 1);
            return Some((
                layout.search.x
                    + (prefix.width() + visible_suffix(input, available).width())
                        .min(usize::from(layout.search.width.saturating_sub(/*rhs*/ 1)))
                        as u16,
                layout.search.y,
            ));
        }
        None
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let layout = self.center_layout(area);
        let inset =
            |area: Rect| area.inner(Margin::new(/*horizontal*/ 2, /*vertical*/ 0));
        let footer_lines = self.footer_lines(area.width.saturating_sub(4));
        let search_key = self
            .agents_keymap
            .primary_hint("search", &self.agents_keymap.search)
            .map(ShortcutHint::display_label)
            .unwrap_or_default();
        let state = self.state();
        let filter_keys = if state.editing_metadata() {
            String::new()
        } else {
            self.center_filter_hint()
        };
        let grouping = match state.grouping {
            AgentsOverviewGrouping::Project => "Project",
            AgentsOverviewGrouping::Status => "Status",
            AgentsOverviewGrouping::Model => "Model",
        };
        let group_key = self
            .agents_keymap
            .primary_hint("toggle_grouping", &self.agents_keymap.toggle_grouping)
            .map(ShortcutHint::display_label)
            .unwrap_or_default();
        line(
            vec![
                "Agent command center".bold(),
                format!("  Group: {grouping}  {group_key}").dim(),
            ],
            inset(layout.header),
            buf,
        );
        let labels = TASK_FILTERS
            .iter()
            .map(|(label, group)| {
                let count = self
                    .rows
                    .iter()
                    .filter(|row| group.is_none_or(|group| group == row.group))
                    .count();
                format!("{label} {count}")
            })
            .collect::<Vec<_>>();
        let mut tabs = inset(row(layout.header, /*offset*/ 1, /*height*/ 1));
        let filter_hint = hint_line(&[(filter_keys, "filter".into())]);
        if tabs.width >= 80 && filter_hint.width() > 0 {
            let hint_width = filter_hint.width() as u16;
            filter_hint.render(
                Rect::new(tabs.right() - hint_width, tabs.y, hint_width, tabs.height),
                buf,
            );
            tabs.width = tabs.width.saturating_sub(hint_width + 2);
        }
        render_filled_tab_bar(
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            state.status_filter,
            tabs,
            buf,
        );
        line(
            "─"
                .repeat(usize::from(area.width.saturating_sub(/*rhs*/ 4)))
                .dim(),
            inset(row(layout.header, /*offset*/ 2, /*height*/ 1)),
            buf,
        );
        if state.editing_metadata() {
            let (label, input) = if state.rename_target.is_some() {
                ("Rename › ", &state.input)
            } else {
                ("Search › ", &state.search)
            };
            let available = usize::from(layout.search.width).saturating_sub(label.width() + 1);
            buf.set_style(layout.search, crate::bottom_pane::active_tab_style());
            line(
                vec![
                    label.cyan().bold(),
                    visible_suffix(input, available).to_owned().into(),
                ],
                layout.search,
                buf,
            );
        } else {
            let search_label = if search_key.is_empty() {
                "Search unavailable".to_owned()
            } else {
                format!("Search tasks · {search_key}")
            };
            line(search_label.dim(), layout.search, buf);
        }
        let notice = state
            .connection_notice
            .map(str::to_owned)
            .or_else(|| {
                state
                    .creating_worktree
                    .then(|| "Creating worktree…".to_owned())
            })
            .or_else(|| {
                (state.refresh_failed && !state.loading).then(|| "Error loading tasks".to_owned())
            })
            .or_else(|| state.server_version_notice.clone());
        if let Some(notice) = notice.filter(|_| state.key_chord_hint.is_none()) {
            line(notice.dim(), inset(layout.footer), buf);
        } else {
            Paragraph::new(footer_lines).render(inset(layout.footer), buf);
        }
        drop(state);
        for y in layout.gap.y..layout.gap.bottom() {
            if layout.gap.width > 1 {
                buf[(layout.gap.x + 1, y)]
                    .set_symbol("│")
                    .set_style(Style::default().dim());
            }
        }
        self.render_center_rows(layout.list, buf);
        if !layout.details.is_empty() {
            self.render_details(layout.details, buf);
        }
    }
}
