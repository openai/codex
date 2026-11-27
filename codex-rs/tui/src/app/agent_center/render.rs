//! Status tabs, task rows, the rename editor and details share a wide/narrow layout.

use super::*;
use crate::bottom_pane::render_filled_tab_bar;
use crate::footer_hint::Hint;
use crate::footer_hint::shortcut;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::motion::LOADING_FRAME_INTERVAL;
use crate::motion::loading_glyph_with_delay;
use crate::style::accent_style;
use crate::style::secondary_text_style;
use crossterm::cursor::SetCursorStyle;
use ratatui::widgets::WidgetRef;
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
        let state = self.state();
        let footer_height = u16::from(area.height >= 2);
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
        let search = row(list, /*offset*/ 0, u16::from(state.editing_metadata()));
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

    fn cursor_style(&self, _area: Rect) -> SetCursorStyle {
        let state = self.state();
        if state.rename_target.is_some() && state.input.uses_vim_insert_cursor() {
            SetCursorStyle::SteadyBar
        } else {
            SetCursorStyle::DefaultUserShape
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let layout = self.center_layout(area);
        let state = self.state();
        if state.editing_metadata() && !layout.search.is_empty() {
            if state.rename_target.is_some() {
                let mut area = layout.search;
                let prefix_width = 9.min(area.width);
                area.x += prefix_width;
                area.width -= prefix_width;
                return state.input.cursor_pos(area);
            }
            let input = &state.search;
            let prefix = "Search › ";
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
        let footer_hints = self.center_footer_hints();
        let mut state = self.state();
        state.row_hits.clear();
        let header_active = self.hint_focus.is_active() && !state.help;
        let filter_keys = if header_active && !state.editing_metadata() {
            self.center_filter_hint()
        } else {
            String::new()
        };
        const DIRECTORY: &str = "Directory";
        let grouping = match state.grouping {
            AgentsOverviewGrouping::Project => "Project",
            AgentsOverviewGrouping::Directory => DIRECTORY,
            AgentsOverviewGrouping::Status => "Status",
            AgentsOverviewGrouping::Model => "Model",
        };
        let hint = self
            .agents_keymap
            .primary_hint("toggle_grouping", &self.agents_keymap.toggle_grouping);
        let key = hint
            .map(|hint| format!("  {}", hint.display_label()))
            .unwrap_or_default();
        let width = usize::from(inset(layout.header).width);
        // Reserve the longest group and the configured hint even when parked. Cycling
        // groups or moving focus should keep the title in place.
        let label = |key_width| {
            [
                ("Agent command center", "  Group: "),
                ("Command Center", "  by "),
                ("Command Center", "  "),
                ("Group:", " "),
            ]
            .into_iter()
            .find(|(title, prefix)| {
                title.width() + prefix.width() + DIRECTORY.width() + key_width <= width
            })
        };
        let (title, mut group_label) = match label(key.width()).or_else(|| label(0)) {
            Some((title, prefix)) => (title, format!("{prefix}{grouping}")),
            None => (grouping, String::new()),
        };
        if header_active
            && !state.task_actions_blocked()
            && hint.is_some_and(|hint| self.editing_allows_hint(&state, hint))
            && title.width() + group_label.width() + key.width() <= width
        {
            group_label.push_str(&key);
        }
        line(
            vec![title.bold(), group_label.dim()],
            inset(layout.header),
            buf,
        );
        let history = state.history.as_ref();
        let labels = TASK_FILTERS
            .iter()
            .map(|(label, group)| {
                let count = self
                    .rows
                    .iter()
                    .filter(|row| state.includes_row(row))
                    .filter(|row| group.is_none_or(|group| group == row.group))
                    .count();
                format!("{label} {count}")
            })
            .collect::<Vec<_>>();
        let mut tabs = inset(row(layout.header, /*offset*/ 1, /*height*/ 1));
        if tabs.width >= 80 && !filter_keys.is_empty() {
            let filter_hint = shortcut(&filter_keys, "filter");
            let hint_width = filter_hint.width() as u16;
            filter_hint.render(
                Rect::new(tabs.right() - hint_width, tabs.y, hint_width, tabs.height),
                buf,
            );
            tabs.width = tabs.width.saturating_sub(hint_width + 2);
        }
        let targets = render_filled_tab_bar(
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            state.status_filter,
            tabs,
            buf,
        );
        if self.dock_focused == Some(false)
            && let Some((_, tab)) = targets.iter().find(|(id, _)| *id == state.status_filter)
        {
            buf.set_style(*tab, crate::style::quiet_workspace_style());
        }
        line(
            "─"
                .repeat(usize::from(area.width.saturating_sub(/*rhs*/ 4)))
                .dim(),
            inset(row(layout.header, /*offset*/ 2, /*height*/ 1)),
            buf,
        );
        if state.editing_metadata() {
            buf.set_style(layout.search, crate::bottom_pane::active_tab_style());
            if state.rename_target.is_some() {
                line("Rename › ".cyan().bold(), layout.search, buf);
                let mut area = layout.search;
                let prefix_width = 9.min(area.width);
                area.x += prefix_width;
                area.width -= prefix_width;
                WidgetRef::render_ref(&&state.input, area, buf);
            } else {
                let label = "Search › ";
                let available = usize::from(layout.search.width).saturating_sub(label.width() + 1);
                line(
                    vec![
                        label.cyan().bold(),
                        visible_suffix(&state.search, available).to_owned().into(),
                    ],
                    layout.search,
                    buf,
                );
            }
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
                state.removal.map(|action| match action {
                    AgentsOverviewAction::Archive => {
                        "Archiving task…  You can still browse.".to_owned()
                    }
                    AgentsOverviewAction::Delete => {
                        "Deleting task…  You can still browse.".to_owned()
                    }
                })
            })
            .or_else(|| {
                let failed = state.refresh_failed || history.is_some_and(|s| s.failed);
                (failed && !state.loading).then(|| "Error loading tasks".to_owned())
            })
            .or_else(|| state.server_version_notice.clone());
        let footer = inset(layout.footer);
        let progress = if state.help || state.connection_notice.is_some() || footer.is_empty() {
            None
        } else if let Some(opening) = state.opening_task {
            Some(("Opening task…", opening.started_at))
        } else if self.dock_motion.is_some() && history.is_none() && notice.is_none() {
            let full_label = "Loading additional tasks…";
            // Reserve a column each for the spinner and its trailing space.
            let label = if full_label.width() + 2 <= usize::from(footer.width) {
                full_label
            } else {
                "Loading tasks…"
            };
            state
                .initial_discovery_started_at
                .map(|started_at| (label, started_at))
        } else {
            None
        };
        if let Some((label, started_at)) = progress {
            let glyph = self.dock_motion.as_ref().map_or("", |(motion, frames)| {
                loading_glyph_with_delay(started_at, LOADING_FRAME_INTERVAL, *motion, frames)
            });
            let glyph = if glyph.is_empty() { " " } else { glyph };
            line(
                vec![
                    Span::styled(glyph, accent_style()),
                    " ".into(),
                    Span::styled(label, secondary_text_style()),
                ],
                footer,
                buf,
            );
        } else if let Some(notice) = notice.filter(|_| !state.help) {
            line(notice.dim(), footer, buf);
        } else {
            let hints = footer_hints
                .into_iter()
                .map(|(key, label)| Hint::key(key, label));
            let row = self.hint_focus.render_row(hints, footer.width);
            line(row.line, footer, buf);
        }
        let help = state.help;
        drop(state);
        if help {
            let body = row(
                area,
                layout.header.height,
                layout.footer.y - layout.header.bottom(),
            );
            Clear.render(inset(body), buf);
            let body = inset(body);
            let mut lines = self.center_help_lines(body.width);
            if lines.len() > usize::from(body.height) {
                lines.truncate(usize::from(body.height.saturating_sub(/*rhs*/ 1)));
                lines.push("… resize to see all".dim().into());
            }
            Paragraph::new(lines).render(body, buf);
            return;
        }
        for y in layout.gap.y..layout.gap.bottom() {
            if layout.gap.width > 1 {
                buf[(layout.gap.x + 1, y)]
                    .set_symbol("│")
                    .set_style(Style::default().dim());
            }
        }
        self.render_center_rows(
            layout.list,
            buf,
            /*progress_in_footer*/ progress.is_some(),
        );
        if !layout.details.is_empty() {
            self.render_details(layout.details, buf);
        }
    }
}
