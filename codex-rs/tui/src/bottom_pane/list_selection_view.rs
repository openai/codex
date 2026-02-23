use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use itertools::Itertools as _;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use super::selection_popup_common::render_menu_surface;
use super::selection_popup_common::wrap_styled_line;
use crate::app_event_sender::AppEventSender;
use crate::key_hint::KeyBinding;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
pub(crate) use super::selection_popup_common::ColumnWidthMode;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::measure_rows_height_stable_col_widths;
use super::selection_popup_common::measure_rows_height_with_col_width_mode;
use super::selection_popup_common::render_rows;
use super::selection_popup_common::render_rows_stable_col_widths;
use super::selection_popup_common::render_rows_with_col_width_mode;
use unicode_width::UnicodeWidthStr;

/// Minimum list width (in content columns) required before the side-by-side
/// layout is activated. Keeps the list usable even when sharing horizontal
/// space with the side content panel.
const MIN_LIST_WIDTH_FOR_SIDE: u16 = 40;

/// Horizontal gap (in columns) between the list area and the side content
/// panel when side-by-side layout is active.
const SIDE_CONTENT_GAP: u16 = 2;

/// Shared menu-surface horizontal inset (2 cells per side) used by selection popups.
const MENU_SURFACE_HORIZONTAL_INSET: u16 = 4;

/// Controls how the side content panel is sized relative to the popup width.
///
/// When the computed side width falls below `side_content_min_width` or the
/// remaining list area would be narrower than [`MIN_LIST_WIDTH_FOR_SIDE`], the
/// side-by-side layout is abandoned and the stacked fallback is used instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SideContentWidth {
    /// Fixed number of columns.  `Fixed(0)` disables side content entirely.
    Fixed(u16),
    /// Exact 50/50 split of the content area (minus the inter-column gap).
    Half,
}

impl Default for SideContentWidth {
    fn default() -> Self {
        Self::Fixed(0)
    }
}

/// Returns the popup content width after subtracting the shared menu-surface
/// horizontal inset (2 columns on each side).
pub(crate) fn popup_content_width(total_width: u16) -> u16 {
    total_width.saturating_sub(MENU_SURFACE_HORIZONTAL_INSET)
}

/// Returns side-by-side layout widths as `(list_width, side_width)` when the
/// layout can fit. Returns `None` when the side panel is disabled/too narrow or
/// when the remaining list width would become unusably small.
pub(crate) fn side_by_side_layout_widths(
    content_width: u16,
    side_content_width: SideContentWidth,
    side_content_min_width: u16,
) -> Option<(u16, u16)> {
    let side_width = match side_content_width {
        SideContentWidth::Fixed(0) => return None,
        SideContentWidth::Fixed(width) => width,
        SideContentWidth::Half => content_width.saturating_sub(SIDE_CONTENT_GAP) / 2,
    };
    if side_width < side_content_min_width {
        return None;
    }
    let list_width = content_width.saturating_sub(SIDE_CONTENT_GAP + side_width);
    (list_width >= MIN_LIST_WIDTH_FOR_SIDE).then_some((list_width, side_width))
}

/// One selectable item in the generic selection list.
pub(crate) type SelectionAction = Box<dyn Fn(&AppEventSender) + Send + Sync>;

/// Callback invoked whenever the highlighted item changes (arrow keys, search
/// filter, number-key jump).  Receives the *actual* index into the unfiltered
/// `items` list and the event sender.  Used by the theme picker for live preview.
pub(crate) type OnSelectionChangedCallback =
    Option<Box<dyn Fn(usize, &AppEventSender) + Send + Sync>>;

/// Callback invoked when the picker is dismissed without accepting (Esc or
/// Ctrl+C).  Used by the theme picker to restore the pre-open theme.
pub(crate) type OnCancelCallback = Option<Box<dyn Fn(&AppEventSender) + Send + Sync>>;

/// One row in a [`ListSelectionView`] selection list.
///
/// This is the source-of-truth model for row state before filtering and
/// formatting into render rows. A row is treated as disabled when either
/// `is_disabled` is true or `disabled_reason` is present; disabled rows cannot
/// be accepted and are skipped by keyboard navigation.
#[derive(Default)]
pub(crate) struct SelectionItem {
    pub name: String,
    pub name_prefix_spans: Vec<Span<'static>>,
    pub display_shortcut: Option<KeyBinding>,
    pub description: Option<String>,
    pub selected_description: Option<String>,
    pub is_current: bool,
    pub is_default: bool,
    pub is_disabled: bool,
    pub actions: Vec<SelectionAction>,
    pub dismiss_on_select: bool,
    pub search_value: Option<String>,
    pub disabled_reason: Option<String>,
}

/// Construction-time configuration for [`ListSelectionView`].
///
/// This config is consumed once by [`ListSelectionView::new`]. After
/// construction, mutable interaction state (filtering, scrolling, and selected
/// row) lives on the view itself.
///
/// `col_width_mode` controls column width mode in selection lists:
/// `AutoVisible` (default) measures only rows visible in the viewport
/// `AutoAllRows` measures all rows to ensure stable column widths as the user scrolls
/// `Fixed` used a fixed 30/70  split between columns
pub(crate) struct SelectionViewParams {
    pub view_id: Option<&'static str>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub footer_note: Option<Line<'static>>,
    pub footer_hint: Option<Line<'static>>,
    pub items: Vec<SelectionItem>,
    pub is_searchable: bool,
    pub search_placeholder: Option<String>,
    pub col_width_mode: ColumnWidthMode,
    pub header: Box<dyn Renderable>,
    pub initial_selected_idx: Option<usize>,

    /// Rich content rendered beside (wide terminals) or below (narrow terminals)
    /// the list items, inside the bordered menu surface. Used by the theme picker
    /// to show a syntax-highlighted preview.
    pub side_content: Box<dyn Renderable>,

    /// Width mode for side content when side-by-side layout is active.
    pub side_content_width: SideContentWidth,

    /// Minimum side panel width required before side-by-side layout activates.
    pub side_content_min_width: u16,

    /// Optional fallback content rendered when side-by-side does not fit.
    /// When absent, `side_content` is reused.
    pub stacked_side_content: Option<Box<dyn Renderable>>,

    /// Keep side-content background colors after rendering in side-by-side mode.
    /// Disabled by default so existing popups preserve their reset-background look.
    pub preserve_side_content_bg: bool,

    /// Called when the highlighted item changes (navigation, filter, number-key).
    /// Receives the *actual* item index, not the filtered/visible index.
    pub on_selection_changed: OnSelectionChangedCallback,

    /// Called when the picker is dismissed via Esc/Ctrl+C without selecting.
    pub on_cancel: OnCancelCallback,
}

impl Default for SelectionViewParams {
    fn default() -> Self {
        Self {
            view_id: None,
            title: None,
            subtitle: None,
            footer_note: None,
            footer_hint: None,
            items: Vec::new(),
            is_searchable: false,
            search_placeholder: None,
            col_width_mode: ColumnWidthMode::AutoVisible,
            header: Box::new(()),
            initial_selected_idx: None,
            side_content: Box::new(()),
            side_content_width: SideContentWidth::default(),
            side_content_min_width: 0,
            stacked_side_content: None,
            preserve_side_content_bg: false,
            on_selection_changed: None,
            on_cancel: None,
        }
    }
}

/// Runtime state for rendering and interacting with a list-based selection popup.
///
/// This type is the single authority for filtered index mapping between
/// visible rows and source items and for preserving selection while filters
/// change.
pub(crate) struct ListSelectionView {
    view_id: Option<&'static str>,
    footer_note: Option<Line<'static>>,
    footer_hint: Option<Line<'static>>,
    items: Vec<SelectionItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    is_searchable: bool,
    search_query: String,
    search_placeholder: Option<String>,
    col_width_mode: ColumnWidthMode,
    filtered_indices: Vec<usize>,
    last_selected_actual_idx: Option<usize>,
    header: Box<dyn Renderable>,
    initial_selected_idx: Option<usize>,
    side_content: Box<dyn Renderable>,
    side_content_width: SideContentWidth,
    side_content_min_width: u16,
    stacked_side_content: Option<Box<dyn Renderable>>,
    preserve_side_content_bg: bool,

    /// Called when the highlighted item changes (navigation, filter, number-key).
    on_selection_changed: OnSelectionChangedCallback,

    /// Called when the picker is dismissed via Esc/Ctrl+C without selecting.
    on_cancel: OnCancelCallback,
}

impl ListSelectionView {
    /// Create a selection popup view with filtering, scrolling, and callbacks wired.
    ///
    /// The constructor normalizes header/title composition and immediately
    /// applies filtering so `ScrollState` starts in a valid visible range.
    /// When search is enabled, rows without `search_value` will disappear as
    /// soon as the query is non-empty, which can look like dropped data unless
    /// callers intentionally populate that field.
    pub fn new(params: SelectionViewParams, app_event_tx: AppEventSender) -> Self {
        let mut header = params.header;
        if params.title.is_some() || params.subtitle.is_some() {
            let title = params.title.map(|title| Line::from(title.bold()));
            let subtitle = params.subtitle.map(|subtitle| Line::from(subtitle.dim()));
            header = Box::new(ColumnRenderable::with([
                header,
                Box::new(title),
                Box::new(subtitle),
            ]));
        }
        let mut s = Self {
            view_id: params.view_id,
            footer_note: params.footer_note,
            footer_hint: params.footer_hint,
            items: params.items,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            is_searchable: params.is_searchable,
            search_query: String::new(),
            search_placeholder: if params.is_searchable {
                params.search_placeholder
            } else {
                None
            },
            col_width_mode: params.col_width_mode,
            filtered_indices: Vec::new(),
            last_selected_actual_idx: None,
            header,
            initial_selected_idx: params.initial_selected_idx,
            side_content: params.side_content,
            side_content_width: params.side_content_width,
            side_content_min_width: params.side_content_min_width,
            stacked_side_content: params.stacked_side_content,
            preserve_side_content_bg: params.preserve_side_content_bg,
            on_selection_changed: params.on_selection_changed,
            on_cancel: params.on_cancel,
        };
        s.apply_filter();
        s
    }

    fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    fn max_visible_rows(len: usize) -> usize {
        MAX_POPUP_ROWS.min(len.max(1))
    }

    fn selected_actual_idx(&self) -> Option<usize> {
        self.state
            .selected_idx
            .and_then(|visible_idx| self.filtered_indices.get(visible_idx).copied())
    }

    fn apply_filter(&mut self) {
        let previously_selected = self
            .selected_actual_idx()
            .or_else(|| {
                (!self.is_searchable)
                    .then(|| self.items.iter().position(|item| item.is_current))
                    .flatten()
            })
            .or_else(|| self.initial_selected_idx.take());

        if self.is_searchable && !self.search_query.is_empty() {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_indices = self
                .items
                .iter()
                .positions(|item| {
                    item.search_value
                        .as_ref()
                        .is_some_and(|v| v.to_lowercase().contains(&query_lower))
                })
                .collect();
        } else {
            self.filtered_indices = (0..self.items.len()).collect();
        }

        let len = self.filtered_indices.len();
        self.state.selected_idx = self
            .state
            .selected_idx
            .and_then(|visible_idx| {
                self.filtered_indices
                    .get(visible_idx)
                    .and_then(|idx| self.filtered_indices.iter().position(|cur| cur == idx))
            })
            .or_else(|| {
                previously_selected.and_then(|actual_idx| {
                    self.filtered_indices
                        .iter()
                        .position(|idx| *idx == actual_idx)
                })
            })
            .or_else(|| (len > 0).then_some(0));

        let visible = Self::max_visible_rows(len);
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, visible);

        // Notify the callback when filtering changes the selected actual item
        // so live preview stays in sync (e.g. typing in the theme picker).
        let new_actual = self.selected_actual_idx();
        if new_actual != previously_selected {
            self.fire_selection_changed();
        }
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(visible_idx, actual_idx)| {
                self.items.get(*actual_idx).map(|item| {
                    let is_selected = self.state.selected_idx == Some(visible_idx);
                    let prefix = if is_selected { '›' } else { ' ' };
                    let name = item.name.as_str();
                    let marker = if item.is_current {
                        " (current)"
                    } else if item.is_default {
                        " (default)"
                    } else {
                        ""
                    };
                    let name_with_marker = format!("{name}{marker}");
                    let n = visible_idx + 1;
                    let wrap_prefix = if self.is_searchable {
                        // The number keys don't work when search is enabled (since we let the
                        // numbers be used for the search query).
                        format!("{prefix} ")
                    } else {
                        format!("{prefix} {n}. ")
                    };
                    let wrap_prefix_width = UnicodeWidthStr::width(wrap_prefix.as_str());
                    let mut name_prefix_spans = Vec::new();
                    name_prefix_spans.push(wrap_prefix.into());
                    name_prefix_spans.extend(item.name_prefix_spans.clone());
                    let description = is_selected
                        .then(|| item.selected_description.clone())
                        .flatten()
                        .or_else(|| item.description.clone());
                    let wrap_indent = description.is_none().then_some(wrap_prefix_width);
                    let is_disabled = item.is_disabled || item.disabled_reason.is_some();
                    GenericDisplayRow {
                        name: name_with_marker,
                        name_prefix_spans,
                        display_shortcut: item.display_shortcut,
                        match_indices: None,
                        description,
                        category_tag: None,
                        wrap_indent,
                        is_disabled,
                        disabled_reason: item.disabled_reason.clone(),
                    }
                })
            })
            .collect()
    }

    fn move_up(&mut self) {
        let before = self.selected_actual_idx();
        let len = self.visible_len();
        self.state.move_up_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
        self.skip_disabled_up();
        if self.selected_actual_idx() != before {
            self.fire_selection_changed();
        }
    }

    fn move_down(&mut self) {
        let before = self.selected_actual_idx();
        let len = self.visible_len();
        self.state.move_down_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
        self.skip_disabled_down();
        if self.selected_actual_idx() != before {
            self.fire_selection_changed();
        }
    }

    fn fire_selection_changed(&self) {
        if let Some(cb) = &self.on_selection_changed
            && let Some(actual) = self.selected_actual_idx()
        {
            cb(actual, &self.app_event_tx);
        }
    }

    fn accept(&mut self) {
        let selected_item = self
            .state
            .selected_idx
            .and_then(|idx| self.filtered_indices.get(idx))
            .and_then(|actual_idx| self.items.get(*actual_idx));
        if let Some(item) = selected_item
            && item.disabled_reason.is_none()
            && !item.is_disabled
        {
            if let Some(idx) = self.state.selected_idx
                && let Some(actual_idx) = self.filtered_indices.get(idx)
            {
                self.last_selected_actual_idx = Some(*actual_idx);
            }
            for act in &item.actions {
                act(&self.app_event_tx);
            }
            if item.dismiss_on_select {
                self.complete = true;
            }
        } else if selected_item.is_none() {
            if let Some(cb) = &self.on_cancel {
                cb(&self.app_event_tx);
            }
            self.complete = true;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.apply_filter();
    }

    pub(crate) fn take_last_selected_index(&mut self) -> Option<usize> {
        self.last_selected_actual_idx.take()
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn clear_to_terminal_bg(buf: &mut Buffer, area: Rect) {
        let buf_area = buf.area();
        let min_x = area.x.max(buf_area.x);
        let min_y = area.y.max(buf_area.y);
        let max_x = area
            .x
            .saturating_add(area.width)
            .min(buf_area.x.saturating_add(buf_area.width));
        let max_y = area
            .y
            .saturating_add(area.height)
            .min(buf_area.y.saturating_add(buf_area.height));
        for y in min_y..max_y {
            for x in min_x..max_x {
                buf[(x, y)]
                    .set_symbol(" ")
                    .set_style(ratatui::style::Style::reset());
            }
        }
    }

    fn force_bg_to_terminal_bg(buf: &mut Buffer, area: Rect) {
        let buf_area = buf.area();
        let min_x = area.x.max(buf_area.x);
        let min_y = area.y.max(buf_area.y);
        let max_x = area
            .x
            .saturating_add(area.width)
            .min(buf_area.x.saturating_add(buf_area.width));
        let max_y = area
            .y
            .saturating_add(area.height)
            .min(buf_area.y.saturating_add(buf_area.height));
        for y in min_y..max_y {
            for x in min_x..max_x {
                buf[(x, y)].set_bg(ratatui::style::Color::Reset);
            }
        }
    }

    fn stacked_side_content(&self) -> &dyn Renderable {
        self.stacked_side_content
            .as_deref()
            .unwrap_or_else(|| self.side_content.as_ref())
    }

    /// Returns `Some(side_width)` when the content area is wide enough for a
    /// side-by-side layout (list + gap + side panel), `None` otherwise.
    fn side_layout_width(&self, content_width: u16) -> Option<u16> {
        side_by_side_layout_widths(
            content_width,
            self.side_content_width,
            self.side_content_min_width,
        )
        .map(|(_, side_width)| side_width)
    }

    fn skip_disabled_down(&mut self) {
        let len = self.visible_len();
        for _ in 0..len {
            if let Some(idx) = self.state.selected_idx
                && let Some(actual_idx) = self.filtered_indices.get(idx)
                && self
                    .items
                    .get(*actual_idx)
                    .is_some_and(|item| item.disabled_reason.is_some() || item.is_disabled)
            {
                self.state.move_down_wrap(len);
            } else {
                break;
            }
        }
    }

    fn skip_disabled_up(&mut self) {
        let len = self.visible_len();
        for _ in 0..len {
            if let Some(idx) = self.state.selected_idx
                && let Some(actual_idx) = self.filtered_indices.get(idx)
                && self
                    .items
                    .get(*actual_idx)
                    .is_some_and(|item| item.disabled_reason.is_some() || item.is_disabled)
            {
                self.state.move_up_wrap(len);
            } else {
                break;
            }
        }
    }
}

impl BottomPaneView for ListSelectionView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            // Some terminals (or configurations) send Control key chords as
            // C0 control characters without reporting the CONTROL modifier.
            // Handle fallbacks for Ctrl-P/N here so navigation works everywhere.
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^P */ => self.move_up(),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } if !self.is_searchable => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{000e}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^N */ => self.move_down(),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } if !self.is_searchable => self.move_down(),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } if self.is_searchable => {
                self.search_query.pop();
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if self.is_searchable
                && !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_query.push(c);
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if !self.is_searchable
                && !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(idx) = c
                    .to_digit(10)
                    .map(|d| d as usize)
                    .and_then(|d| d.checked_sub(1))
                    && idx < self.items.len()
                    && self
                        .items
                        .get(idx)
                        .is_some_and(|item| item.disabled_reason.is_none() && !item.is_disabled)
                {
                    self.state.selected_idx = Some(idx);
                    self.accept();
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.accept(),
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn view_id(&self) -> Option<&'static str> {
        self.view_id
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if let Some(cb) = &self.on_cancel {
            cb(&self.app_event_tx);
        }
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for ListSelectionView {
    fn desired_height(&self, width: u16) -> u16 {
        // Inner content width after menu surface horizontal insets (2 per side).
        let inner_width = popup_content_width(width);

        // When side-by-side is active, measure the list at the reduced width
        // that accounts for the gap and side panel.
        let effective_rows_width = if let Some(side_w) = self.side_layout_width(inner_width) {
            Self::rows_width(width).saturating_sub(SIDE_CONTENT_GAP + side_w)
        } else {
            Self::rows_width(width)
        };

        // Measure wrapped height for up to MAX_POPUP_ROWS items.
        let rows = self.build_rows();
        let rows_height = match self.col_width_mode {
            ColumnWidthMode::AutoVisible => measure_rows_height(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
            ),
            ColumnWidthMode::AutoAllRows => measure_rows_height_stable_col_widths(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
            ),
            ColumnWidthMode::Fixed => measure_rows_height_with_col_width_mode(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
                ColumnWidthMode::Fixed,
            ),
        };

        let mut height = self.header.desired_height(inner_width);
        height = height.saturating_add(rows_height + 3);
        if self.is_searchable {
            height = height.saturating_add(1);
        }

        // Side content: when the terminal is wide enough the panel sits beside
        // the list and shares vertical space; otherwise it stacks below.
        if self.side_layout_width(inner_width).is_some() {
            // Side-by-side — side content shares list rows vertically so it
            // doesn't add to total height.
        } else {
            let side_h = self.stacked_side_content().desired_height(inner_width);
            if side_h > 0 {
                height = height.saturating_add(1 + side_h);
            }
        }

        if let Some(note) = &self.footer_note {
            let note_width = width.saturating_sub(2);
            let note_lines = wrap_styled_line(note, note_width);
            height = height.saturating_add(note_lines.len() as u16);
        }
        if self.footer_hint.is_some() {
            height = height.saturating_add(1);
        }
        height
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let note_width = area.width.saturating_sub(2);
        let note_lines = self
            .footer_note
            .as_ref()
            .map(|note| wrap_styled_line(note, note_width));
        let note_height = note_lines.as_ref().map_or(0, |lines| lines.len() as u16);
        let footer_rows = note_height + u16::from(self.footer_hint.is_some());
        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(footer_rows)]).areas(area);

        let outer_content_area = content_area;
        // Paint the shared menu surface and then layout inside the returned inset.
        let content_area = render_menu_surface(outer_content_area, buf);

        let inner_width = popup_content_width(outer_content_area.width);
        let side_w = self.side_layout_width(inner_width);

        // When side-by-side is active, shrink the list to make room.
        let full_rows_width = Self::rows_width(outer_content_area.width);
        let effective_rows_width = if let Some(sw) = side_w {
            full_rows_width.saturating_sub(SIDE_CONTENT_GAP + sw)
        } else {
            full_rows_width
        };

        let header_height = self.header.desired_height(inner_width);
        let rows = self.build_rows();
        let rows_height = match self.col_width_mode {
            ColumnWidthMode::AutoVisible => measure_rows_height(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
            ),
            ColumnWidthMode::AutoAllRows => measure_rows_height_stable_col_widths(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
            ),
            ColumnWidthMode::Fixed => measure_rows_height_with_col_width_mode(
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                effective_rows_width.saturating_add(1),
                ColumnWidthMode::Fixed,
            ),
        };

        // Stacked (fallback) side content height — only used when not side-by-side.
        let stacked_side_h = if side_w.is_none() {
            self.stacked_side_content().desired_height(inner_width)
        } else {
            0
        };
        let stacked_gap = if stacked_side_h > 0 { 1 } else { 0 };

        let [header_area, _, search_area, list_area, _, stacked_side_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(if self.is_searchable { 1 } else { 0 }),
            Constraint::Length(rows_height),
            Constraint::Length(stacked_gap),
            Constraint::Length(stacked_side_h),
        ])
        .areas(content_area);

        // -- Header --
        if header_area.height < header_height {
            let [header_area, elision_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(header_area);
            self.header.render(header_area, buf);
            Paragraph::new(vec![
                Line::from(format!("[… {header_height} lines] ctrl + a view all")).dim(),
            ])
            .render(elision_area, buf);
        } else {
            self.header.render(header_area, buf);
        }

        // -- Search bar --
        if self.is_searchable {
            Line::from(self.search_query.clone()).render(search_area, buf);
            let query_span: Span<'static> = if self.search_query.is_empty() {
                self.search_placeholder
                    .as_ref()
                    .map(|placeholder| placeholder.clone().dim())
                    .unwrap_or_else(|| "".into())
            } else {
                self.search_query.clone().into()
            };
            Line::from(query_span).render(search_area, buf);
        }

        // -- List rows --
        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: effective_rows_width.max(1),
                height: list_area.height,
            };
            match self.col_width_mode {
                ColumnWidthMode::AutoVisible => render_rows(
                    render_area,
                    buf,
                    &rows,
                    &self.state,
                    render_area.height as usize,
                    "no matches",
                ),
                ColumnWidthMode::AutoAllRows => render_rows_stable_col_widths(
                    render_area,
                    buf,
                    &rows,
                    &self.state,
                    render_area.height as usize,
                    "no matches",
                ),
                ColumnWidthMode::Fixed => render_rows_with_col_width_mode(
                    render_area,
                    buf,
                    &rows,
                    &self.state,
                    render_area.height as usize,
                    "no matches",
                    ColumnWidthMode::Fixed,
                ),
            };
        }

        // -- Side content (preview panel) --
        if let Some(sw) = side_w {
            // Side-by-side: render to the right half of the popup content
            // area so preview content can center vertically in that panel.
            let side_x = content_area.x + content_area.width - sw;
            let side_area = Rect::new(side_x, content_area.y, sw, content_area.height);

            // Clear the menu-surface background behind the side panel so the
            // preview appears on the terminal's own background.
            let clear_x = side_x.saturating_sub(SIDE_CONTENT_GAP);
            let clear_w = outer_content_area
                .x
                .saturating_add(outer_content_area.width)
                .saturating_sub(clear_x);
            Self::clear_to_terminal_bg(
                buf,
                Rect::new(
                    clear_x,
                    outer_content_area.y,
                    clear_w,
                    outer_content_area.height,
                ),
            );
            self.side_content.render(side_area, buf);
            if !self.preserve_side_content_bg {
                Self::force_bg_to_terminal_bg(
                    buf,
                    Rect::new(
                        clear_x,
                        outer_content_area.y,
                        clear_w,
                        outer_content_area.height,
                    ),
                );
            }
        } else if stacked_side_area.height > 0 {
            // Stacked fallback: render below the list (same as old footer_content).
            let clear_height = (outer_content_area.y + outer_content_area.height)
                .saturating_sub(stacked_side_area.y);
            let clear_area = Rect::new(
                outer_content_area.x,
                stacked_side_area.y,
                outer_content_area.width,
                clear_height,
            );
            Self::clear_to_terminal_bg(buf, clear_area);
            self.stacked_side_content().render(stacked_side_area, buf);
        }

        if footer_area.height > 0 {
            let [note_area, hint_area] = Layout::vertical([
                Constraint::Length(note_height),
                Constraint::Length(if self.footer_hint.is_some() { 1 } else { 0 }),
            ])
            .areas(footer_area);

            if let Some(lines) = note_lines {
                let note_area = Rect {
                    x: note_area.x + 2,
                    y: note_area.y,
                    width: note_area.width.saturating_sub(2),
                    height: note_area.height,
                };
                for (idx, line) in lines.iter().enumerate() {
                    if idx as u16 >= note_area.height {
                        break;
                    }
                    let line_area = Rect {
                        x: note_area.x,
                        y: note_area.y + idx as u16,
                        width: note_area.width,
                        height: 1,
                    };
                    line.clone().render(line_area, buf);
                }
            }

            if let Some(hint) = &self.footer_hint {
                let hint_area = Rect {
                    x: hint_area.x + 2,
                    y: hint_area.y,
                    width: hint_area.width.saturating_sub(2),
                    height: hint_area.height,
                };
                hint.clone().dim().render(hint_area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::bottom_pane::popup_consts::standard_popup_hint_line;
    use crossterm::event::KeyCode;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::style::Style;
    use tokio::sync::mpsc::unbounded_channel;

    struct MarkerRenderable {
        marker: &'static str,
        height: u16,
    }

    impl Renderable for MarkerRenderable {
        fn render(&self, area: Rect, buf: &mut Buffer) {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    if x < buf.area().width && y < buf.area().height {
                        buf[(x, y)].set_symbol(self.marker);
                    }
                }
            }
        }

        fn desired_height(&self, _width: u16) -> u16 {
            self.height
        }
    }

    struct StyledMarkerRenderable {
        marker: &'static str,
        style: Style,
        height: u16,
    }

    impl Renderable for StyledMarkerRenderable {
        fn render(&self, area: Rect, buf: &mut Buffer) {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    if x < buf.area().width && y < buf.area().height {
                        buf[(x, y)].set_symbol(self.marker).set_style(self.style);
                    }
                }
            }
        }

        fn desired_height(&self, _width: u16) -> u16 {
            self.height
        }
    }

    fn make_selection_view(subtitle: Option<&str>) -> ListSelectionView {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "Read Only".to_string(),
                description: Some("Codex can read files".to_string()),
                is_current: true,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Full Access".to_string(),
                description: Some("Codex can edit files".to_string()),
                is_current: false,
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        ListSelectionView::new(
            SelectionViewParams {
                title: Some("Select Approval Mode".to_string()),
                subtitle: subtitle.map(str::to_string),
                footer_hint: Some(standard_popup_hint_line()),
                items,
                ..Default::default()
            },
            tx,
        )
    }

    fn render_lines(view: &ListSelectionView) -> String {
        render_lines_with_width(view, 48)
    }

    fn render_lines_with_width(view: &ListSelectionView, width: u16) -> String {
        render_lines_in_area(view, width, view.desired_height(width))
    }

    fn render_lines_in_area(view: &ListSelectionView, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let lines: Vec<String> = (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line
            })
            .collect();
        lines.join("\n")
    }

    fn description_col(rendered: &str, item_marker: &str, description: &str) -> usize {
        let line = rendered
            .lines()
            .find(|line| line.contains(item_marker) && line.contains(description))
            .expect("expected rendered line to contain row marker and description");
        line.find(description)
            .expect("expected rendered line to contain description")
    }

    fn make_scrolling_width_items() -> Vec<SelectionItem> {
        let mut items: Vec<SelectionItem> = (1..=8)
            .map(|idx| SelectionItem {
                name: format!("Item {idx}"),
                description: Some(format!("desc {idx}")),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect();
        items.push(SelectionItem {
            name: "Item 9 with an intentionally much longer name".to_string(),
            description: Some("desc 9".to_string()),
            dismiss_on_select: true,
            ..Default::default()
        });
        items
    }

    fn render_before_after_scroll_snapshot(col_width_mode: ColumnWidthMode, width: u16) -> String {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: make_scrolling_width_items(),
                col_width_mode,
                ..Default::default()
            },
            tx,
        );

        let before_scroll = render_lines_with_width(&view, width);
        for _ in 0..8 {
            view.handle_key_event(KeyEvent::from(KeyCode::Down));
        }
        let after_scroll = render_lines_with_width(&view, width);

        format!("before scroll:\n{before_scroll}\n\nafter scroll:\n{after_scroll}")
    }

    #[test]
    fn renders_blank_line_between_title_and_items_without_subtitle() {
        let view = make_selection_view(None);
        assert_snapshot!(
            "list_selection_spacing_without_subtitle",
            render_lines(&view)
        );
    }

    #[test]
    fn renders_blank_line_between_subtitle_and_items() {
        let view = make_selection_view(Some("Switch between Codex approval presets"));
        assert_snapshot!("list_selection_spacing_with_subtitle", render_lines(&view));
    }

    #[test]
    fn theme_picker_subtitle_uses_fallback_text_in_94x35_terminal() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let home = dirs::home_dir().expect("home directory should be available");
        let codex_home = home.join(".codex");
        let params =
            crate::theme_picker::build_theme_picker_params(None, Some(&codex_home), Some(94));
        let view = ListSelectionView::new(params, tx);

        let rendered = render_lines_in_area(&view, 94, 35);
        assert!(rendered.contains("Move up/down to live preview themes"));
    }

    #[test]
    fn theme_picker_enables_side_content_background_preservation() {
        let params = crate::theme_picker::build_theme_picker_params(None, None, Some(120));
        assert!(
            params.preserve_side_content_bg,
            "theme picker should preserve side-content backgrounds to keep diff preview styling",
        );
    }

    #[test]
    fn preserve_side_content_bg_keeps_rendered_background_colors() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(StyledMarkerRenderable {
                    marker: "+",
                    style: Style::default().bg(Color::Blue),
                    height: 1,
                }),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 10,
                preserve_side_content_bg: true,
                ..Default::default()
            },
            tx,
        );
        let area = Rect::new(0, 0, 120, 35);
        let mut buf = Buffer::empty(area);

        view.render(area, &mut buf);

        let plus_bg = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .find_map(|(x, y)| {
                let cell = &buf[(x, y)];
                (cell.symbol() == "+").then(|| cell.style().bg)
            })
            .expect("expected side content to render at least one '+' marker");
        assert_eq!(
            plus_bg,
            Some(Color::Blue),
            "expected side-content marker to preserve custom background styling",
        );
    }

    #[test]
    fn snapshot_footer_note_wraps() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![SelectionItem {
            name: "Read Only".to_string(),
            description: Some("Codex can read files".to_string()),
            is_current: true,
            dismiss_on_select: true,
            ..Default::default()
        }];
        let footer_note = Line::from(vec![
            "Note: ".dim(),
            "Use /setup-default-sandbox".cyan(),
            " to allow network access.".dim(),
        ]);
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Select Approval Mode".to_string()),
                footer_note: Some(footer_note),
                footer_hint: Some(standard_popup_hint_line()),
                items,
                ..Default::default()
            },
            tx,
        );
        assert_snapshot!(
            "list_selection_footer_note_wraps",
            render_lines_with_width(&view, 40)
        );
    }

    #[test]
    fn renders_search_query_line_when_enabled() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![SelectionItem {
            name: "Read Only".to_string(),
            description: Some("Codex can read files".to_string()),
            is_current: false,
            dismiss_on_select: true,
            ..Default::default()
        }];
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Select Approval Mode".to_string()),
                footer_hint: Some(standard_popup_hint_line()),
                items,
                is_searchable: true,
                search_placeholder: Some("Type to search branches".to_string()),
                ..Default::default()
            },
            tx,
        );
        view.set_search_query("filters".to_string());

        let lines = render_lines(&view);
        assert!(
            lines.contains("filters"),
            "expected search query line to include rendered query, got {lines:?}"
        );
    }

    #[test]
    fn enter_with_no_matches_triggers_cancel_callback() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                items: vec![SelectionItem {
                    name: "Read Only".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                is_searchable: true,
                on_cancel: Some(Box::new(|tx: &_| {
                    tx.send(AppEvent::OpenApprovalsPopup);
                })),
                ..Default::default()
            },
            tx,
        );
        view.set_search_query("no-matches".to_string());

        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert!(view.is_complete());
        match rx.try_recv() {
            Ok(AppEvent::OpenApprovalsPopup) => {}
            Ok(other) => panic!("expected OpenApprovalsPopup cancel event, got {other:?}"),
            Err(err) => panic!("expected cancel callback event, got {err}"),
        }
    }

    #[test]
    fn move_down_without_selection_change_does_not_fire_callback() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                items: vec![SelectionItem {
                    name: "Only choice".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                on_selection_changed: Some(Box::new(|_idx, tx: &_| {
                    tx.send(AppEvent::OpenApprovalsPopup);
                })),
                ..Default::default()
            },
            tx,
        );

        while rx.try_recv().is_ok() {}

        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        assert!(
            rx.try_recv().is_err(),
            "moving down in a single-item list should not fire on_selection_changed",
        );
    }

    #[test]
    fn wraps_long_option_without_overflowing_columns() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "Yes, proceed".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Yes, and don't ask again for commands that start with `python -mpre_commit run --files eslint-plugin/no-mixed-const-enum-exports.js`".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Approval".to_string()),
                items,
                ..Default::default()
            },
            tx,
        );

        let rendered = render_lines_with_width(&view, 60);
        let command_line = rendered
            .lines()
            .find(|line| line.contains("python -mpre_commit run"))
            .expect("rendered lines should include wrapped command");
        assert!(
            command_line.starts_with("     `python -mpre_commit run"),
            "wrapped command line should align under the numbered prefix:\n{rendered}"
        );
        assert!(
            rendered.contains("eslint-plugin/no-")
                && rendered.contains("mixed-const-enum-exports.js"),
            "long command should not be truncated even when wrapped:\n{rendered}"
        );
    }

    #[test]
    fn width_changes_do_not_hide_rows() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "gpt-5.1-codex".to_string(),
                description: Some(
                    "Optimized for Codex. Balance of reasoning quality and coding ability."
                        .to_string(),
                ),
                is_current: true,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "gpt-5.1-codex-mini".to_string(),
                description: Some(
                    "Optimized for Codex. Cheaper, faster, but less capable.".to_string(),
                ),
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "gpt-4.1-codex".to_string(),
                description: Some(
                    "Legacy model. Use when you need compatibility with older automations."
                        .to_string(),
                ),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Select Model and Effort".to_string()),
                items,
                ..Default::default()
            },
            tx,
        );
        let mut missing: Vec<u16> = Vec::new();
        for width in 60..=90 {
            let rendered = render_lines_with_width(&view, width);
            if !rendered.contains("3.") {
                missing.push(width);
            }
        }
        assert!(
            missing.is_empty(),
            "third option missing at widths {missing:?}"
        );
    }

    #[test]
    fn narrow_width_keeps_all_rows_visible() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let desc = "x".repeat(10);
        let items: Vec<SelectionItem> = (1..=3)
            .map(|idx| SelectionItem {
                name: format!("Item {idx}"),
                description: Some(desc.clone()),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect();
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items,
                ..Default::default()
            },
            tx,
        );
        let rendered = render_lines_with_width(&view, 24);
        assert!(
            rendered.contains("3."),
            "third option missing for width 24:\n{rendered}"
        );
    }

    #[test]
    fn snapshot_model_picker_width_80() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "gpt-5.1-codex".to_string(),
                description: Some(
                    "Optimized for Codex. Balance of reasoning quality and coding ability."
                        .to_string(),
                ),
                is_current: true,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "gpt-5.1-codex-mini".to_string(),
                description: Some(
                    "Optimized for Codex. Cheaper, faster, but less capable.".to_string(),
                ),
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "gpt-4.1-codex".to_string(),
                description: Some(
                    "Legacy model. Use when you need compatibility with older automations."
                        .to_string(),
                ),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Select Model and Effort".to_string()),
                items,
                ..Default::default()
            },
            tx,
        );
        assert_snapshot!(
            "list_selection_model_picker_width_80",
            render_lines_with_width(&view, 80)
        );
    }

    #[test]
    fn snapshot_narrow_width_preserves_third_option() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let desc = "x".repeat(10);
        let items: Vec<SelectionItem> = (1..=3)
            .map(|idx| SelectionItem {
                name: format!("Item {idx}"),
                description: Some(desc.clone()),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect();
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items,
                ..Default::default()
            },
            tx,
        );
        assert_snapshot!(
            "list_selection_narrow_width_preserves_rows",
            render_lines_with_width(&view, 24)
        );
    }

    #[test]
    fn snapshot_auto_visible_col_width_mode_scroll_behavior() {
        assert_snapshot!(
            "list_selection_col_width_mode_auto_visible_scroll",
            render_before_after_scroll_snapshot(ColumnWidthMode::AutoVisible, 96)
        );
    }

    #[test]
    fn snapshot_auto_all_rows_col_width_mode_scroll_behavior() {
        assert_snapshot!(
            "list_selection_col_width_mode_auto_all_rows_scroll",
            render_before_after_scroll_snapshot(ColumnWidthMode::AutoAllRows, 96)
        );
    }

    #[test]
    fn snapshot_fixed_col_width_mode_scroll_behavior() {
        assert_snapshot!(
            "list_selection_col_width_mode_fixed_scroll",
            render_before_after_scroll_snapshot(ColumnWidthMode::Fixed, 96)
        );
    }

    #[test]
    fn auto_all_rows_col_width_does_not_shift_when_scrolling() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);

        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: make_scrolling_width_items(),
                col_width_mode: ColumnWidthMode::AutoAllRows,
                ..Default::default()
            },
            tx,
        );

        let before_scroll = render_lines_with_width(&view, 96);
        for _ in 0..8 {
            view.handle_key_event(KeyEvent::from(KeyCode::Down));
        }
        let after_scroll = render_lines_with_width(&view, 96);

        assert!(
            after_scroll.contains("9. Item 9 with an intentionally much longer name"),
            "expected the scrolled view to include the longer row:\n{after_scroll}"
        );

        let before_col = description_col(&before_scroll, "8. Item 8", "desc 8");
        let after_col = description_col(&after_scroll, "8. Item 8", "desc 8");
        assert_eq!(
            before_col, after_col,
            "description column changed across scroll:\nbefore:\n{before_scroll}\nafter:\n{after_scroll}"
        );
    }

    #[test]
    fn fixed_col_width_is_30_70_and_does_not_shift_when_scrolling() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let width = 96;
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: make_scrolling_width_items(),
                col_width_mode: ColumnWidthMode::Fixed,
                ..Default::default()
            },
            tx,
        );

        let before_scroll = render_lines_with_width(&view, width);
        let before_col = description_col(&before_scroll, "8. Item 8", "desc 8");
        let expected_desc_col = ((width.saturating_sub(2) as usize) * 3) / 10;
        assert_eq!(
            before_col, expected_desc_col,
            "fixed mode should place description column at a 30/70 split:\n{before_scroll}"
        );

        for _ in 0..8 {
            view.handle_key_event(KeyEvent::from(KeyCode::Down));
        }
        let after_scroll = render_lines_with_width(&view, width);
        let after_col = description_col(&after_scroll, "8. Item 8", "desc 8");
        assert_eq!(
            before_col, after_col,
            "fixed description column changed across scroll:\nbefore:\n{before_scroll}\nafter:\n{after_scroll}"
        );
    }

    #[test]
    fn side_layout_width_half_uses_exact_split() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(MarkerRenderable {
                    marker: "W",
                    height: 1,
                }),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 10,
                ..Default::default()
            },
            tx,
        );

        let content_width: u16 = 120;
        let expected = content_width.saturating_sub(SIDE_CONTENT_GAP) / 2;
        assert_eq!(view.side_layout_width(content_width), Some(expected));
    }

    #[test]
    fn side_layout_width_half_falls_back_when_list_would_be_too_narrow() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(MarkerRenderable {
                    marker: "W",
                    height: 1,
                }),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 50,
                ..Default::default()
            },
            tx,
        );

        assert_eq!(view.side_layout_width(80), None);
    }

    #[test]
    fn stacked_side_content_is_used_when_side_by_side_does_not_fit() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(MarkerRenderable {
                    marker: "W",
                    height: 1,
                }),
                stacked_side_content: Some(Box::new(MarkerRenderable {
                    marker: "N",
                    height: 1,
                })),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 60,
                ..Default::default()
            },
            tx,
        );

        let rendered = render_lines_with_width(&view, 70);
        assert!(
            rendered.contains('N'),
            "expected stacked marker to be rendered:\n{rendered}"
        );
        assert!(
            !rendered.contains('W'),
            "wide marker should not render in stacked mode:\n{rendered}"
        );
    }

    #[test]
    fn side_content_clearing_resets_symbols_and_style() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(MarkerRenderable {
                    marker: "W",
                    height: 1,
                }),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 10,
                ..Default::default()
            },
            tx,
        );

        let width = 120;
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        for y in 0..height {
            for x in 0..width {
                buf[(x, y)]
                    .set_symbol("X")
                    .set_style(Style::default().bg(Color::Red));
            }
        }
        view.render(area, &mut buf);

        let cell = &buf[(width - 1, 0)];
        assert_eq!(cell.symbol(), " ");
        let style = cell.style();
        assert_eq!(style.fg, Some(Color::Reset));
        assert_eq!(style.bg, Some(Color::Reset));
        assert_eq!(style.underline_color, Some(Color::Reset));

        let mut saw_marker = false;
        for y in 0..height {
            for x in 0..width {
                let cell = &buf[(x, y)];
                if cell.symbol() == "W" {
                    saw_marker = true;
                    assert_eq!(cell.style().bg, Some(Color::Reset));
                }
            }
        }
        assert!(
            saw_marker,
            "expected side marker renderable to draw into buffer"
        );
    }

    #[test]
    fn side_content_clearing_handles_non_zero_buffer_origin() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Debug".to_string()),
                items: vec![SelectionItem {
                    name: "Item 1".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                side_content: Box::new(MarkerRenderable {
                    marker: "W",
                    height: 1,
                }),
                side_content_width: SideContentWidth::Half,
                side_content_min_width: 10,
                ..Default::default()
            },
            tx,
        );

        let width = 120;
        let height = view.desired_height(width);
        let area = Rect::new(0, 20, width, height);
        let mut buf = Buffer::empty(area);
        for y in area.y..area.y + height {
            for x in area.x..area.x + width {
                buf[(x, y)]
                    .set_symbol("X")
                    .set_style(Style::default().bg(Color::Red));
            }
        }
        view.render(area, &mut buf);

        let cell = &buf[(area.x + width - 1, area.y)];
        assert_eq!(cell.symbol(), " ");
        assert_eq!(cell.style().bg, Some(Color::Reset));
    }
}
