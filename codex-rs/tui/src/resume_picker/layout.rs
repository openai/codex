//! Session-picker chrome shares its geometry with paging and rendering.
//! Decorative gaps collapse before the list loses room for two comfortable rows.

use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;

use super::PickerState;
use super::list_viewport_width;
use super::render_list;
use super::render_picker_footer;
use super::render_transcript_loading_overlay;
use super::search_line;
use super::toolbar_for_width;

pub(super) struct PickerAreas {
    header: Rect,
    toolbar: Rect,
    search: Rect,
    pub(super) list: Rect,
    footer: Rect,
}

pub(super) fn areas(area: Rect) -> PickerAreas {
    const FIXED_CHROME_ROWS: u16 = 7;
    const MIN_LIST_ROWS: u16 = 6;
    let gaps = area
        .height
        .saturating_sub(FIXED_CHROME_ROWS + MIN_LIST_ROWS)
        .min(/*other*/ 3);
    let [header, _, toolbar, _, search, _, list, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(u16::from(gaps >= 3)),
        Constraint::Length(1),
        Constraint::Length(u16::from(gaps >= 2)),
        Constraint::Length(1),
        Constraint::Length(u16::from(gaps >= 1)),
        Constraint::Min(area.height.saturating_sub(FIXED_CHROME_ROWS + gaps)),
        Constraint::Length(4),
    ])
    .areas(area);
    PickerAreas {
        header,
        toolbar,
        search,
        list,
        footer,
    }
}

pub(super) fn render(frame: &mut crate::custom_terminal::Frame, state: &PickerState) {
    let PickerAreas {
        header,
        toolbar,
        search,
        list,
        footer,
    } = areas(frame.area());
    let chrome = |area: Rect| {
        Rect::new(
            area.x.saturating_add(/*rhs*/ 1),
            area.y,
            area.width.saturating_sub(/*rhs*/ 2),
            area.height,
        )
    };
    frame.render_widget_ref(&Line::from(state.action.title().bold()), chrome(header));
    let toolbar = chrome(toolbar);
    frame.render_widget_ref(&toolbar_for_width(state, toolbar.width), toolbar);
    let search = chrome(search);
    frame.render_widget_ref(&search_line(state, search.width), search);
    let list = Rect::new(
        list.x.saturating_add(/*rhs*/ 2),
        list.y,
        list_viewport_width(list.width),
        list.height,
    );
    render_list(frame, list, state);
    if state.is_transcript_loading() {
        render_transcript_loading_overlay(frame, list);
    }
    render_picker_footer(frame, footer, state, list.height);
}
