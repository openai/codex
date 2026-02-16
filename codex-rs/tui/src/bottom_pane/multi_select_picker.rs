//! Multi-select picker widget for selecting multiple items from a list.
//!
//! This module provides a fuzzy-searchable, scrollable picker that allows users
//! to toggle multiple items on/off. It supports:
//!
//! - **Fuzzy search**: Type to filter items by name
//! - **Toggle selection**: Space to toggle items on/off
//! - **Reordering**: Optional left/right arrow support to reorder items
//! - **Live preview**: Optional callback to show a preview of current selections
//! - **Callbacks**: Hooks for change, confirm, and cancel events
//!
//! # Example
//!
//! ```ignore
//! let picker = MultiSelectPicker::new(
//!     "Select Items".to_string(),
//!     Some("Choose which items to enable".to_string()),
//!     app_event_tx,
//! )
//! .items(vec![
//!     MultiSelectItem { id: "a".into(), name: "Item A".into(), description: None, enabled: true },
//!     MultiSelectItem { id: "b".into(), name: "Item B".into(), description: None, enabled: false },
//! ])
//! .on_confirm(|selected_ids, tx| { /* handle confirmation */ })
//! .build();
//! ```

use codex_utils_fuzzy_match::fuzzy_match;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use super::selection_popup_common::GenericDisplayRow;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::render_rows_single_line;
use crate::bottom_pane::selection_popup_common::truncate_line_with_ellipsis_if_overflow;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;
use crate::text_formatting::truncate_text;

/// Maximum display length for item names before truncation.
const ITEM_NAME_TRUNCATE_LEN: usize = 21;

/// Placeholder text shown in the search input when empty.
const SEARCH_PLACEHOLDER: &str = "Type to search";

/// Prefix displayed before the search query (mimics a command prompt).
const SEARCH_PROMPT_PREFIX: &str = "> ";

/// Direction for reordering items in the list.
enum Direction {
    Up,
    Down,
}

/// Callback invoked when any item's state changes (toggled or reordered).
/// Receives the full list of items and the event sender.
pub type ChangeCallBack = Box<dyn Fn(&[MultiSelectItem], &AppEventSender) + Send + Sync>;

/// Callback invoked when the user confirms their selection (presses Enter).
/// Receives a list of IDs for all enabled items.
pub type ConfirmCallback = Box<dyn Fn(&[String], &AppEventSender) + Send + Sync>;

/// Callback invoked when the user cancels the picker (presses Escape).
pub type CancelCallback = Box<dyn Fn(&AppEventSender) + Send + Sync>;

/// Callback to generate an optional preview line based on current item states.
/// Returns `None` to hide the preview area.
pub type PreviewCallback = Box<dyn Fn(&[MultiSelectItem]) -> Option<Line<'static>> + Send + Sync>;

/// A single selectable item in the multi-select picker.
///
/// Each item has a unique identifier, display name, optional description,
/// and an enabled/disabled state that can be toggled by the user.
#[derive(Default)]
pub(crate) struct MultiSelectItem {
    /// Unique identifier returned in the confirm callback when this item is enabled.
    pub id: String,

    /// Display name shown in the picker list. Will be truncated if too long.
    pub name: String,

    /// Optional description shown alongside the name (dimmed).
    pub description: Option<String>,

    /// Whether this item is currently selected/enabled.
    pub enabled: bool,
}

/// A multi-select picker widget with fuzzy search and optional reordering.
///
/// The picker displays a scrollable list of items with checkboxes. Users can:
/// - Type to fuzzy-search and filter the list
/// - Use Up/Down (or Ctrl+P/Ctrl+N) to navigate
/// - Press Space to toggle the selected item
/// - Press Enter to confirm and close
/// - Press Escape to cancel and close
/// - Use Left/Right arrows to reorder items (if ordering is enabled)
///
/// Create instances using the builder pattern via [`MultiSelectPicker::new`].
pub(crate) struct MultiSelectPicker {
    /// All items in the picker (unfiltered).
    items: Vec<MultiSelectItem>,

    /// Scroll and selection state for the visible list.
    state: ScrollState,

    /// Whether the picker has been closed (confirmed or cancelled).
    pub(crate) complete: bool,

    /// Channel for sending application events.
    app_event_tx: AppEventSender,

    /// Header widget displaying title and subtitle.
    header: Box<dyn Renderable>,

    /// Footer line showing keyboard hints.
    footer_hint: Line<'static>,

    /// Current search/filter query entered by the user.
    search_query: String,

    /// Indices into `items` that match the current filter, in display order.
    filtered_indices: Vec<usize>,

    /// Whether left/right arrow reordering is enabled.
    ordering_enabled: bool,

    /// Optional callback to generate a preview line from current item states.
    preview_builder: Option<PreviewCallback>,

    /// Cached preview line (updated on item changes).
    preview_line: Option<Line<'static>>,

    /// Callback invoked when items change (toggle or reorder).
    on_change: Option<ChangeCallBack>,

    /// Callback invoked when the user confirms their selection.
    on_confirm: Option<ConfirmCallback>,

    /// Callback invoked when the user cancels the picker.
    on_cancel: Option<CancelCallback>,
}

impl MultiSelectPicker {
    /// Creates a new builder for constructing a `MultiSelectPicker`.
    ///
    /// # Arguments
    ///
    /// * `title` - The main title displayed at the top of the picker
    /// * `subtitle` - Optional subtitle displayed below the title (dimmed)
    /// * `app_event_tx` - Event sender for dispatching application events
    pub fn builder(
        title: String,
        subtitle: Option<String>,
        app_event_tx: AppEventSender,
    ) -> MultiSelectPickerBuilder {
        MultiSelectPickerBuilder::new(title, subtitle, app_event_tx)
    }

    /// Applies the current search query to filter and sort items.
    ///
    /// Updates `filtered_indices` to contain only matching items, sorted by
    /// fuzzy match score. Attempts to preserve the current selection if it
    /// still matches the filter.
    fn apply_filter(&mut self) {
        // Filter + sort while preserving the current selection when possible.
        let previously_selected = self
            .state
            .selected_idx
            .and_then(|visible_idx| self.filtered_indices.get(visible_idx).copied());

        let filter = self.search_query.trim();
        if filter.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            let mut matches: Vec<(usize, i32)> = Vec::new();
            for (idx, item) in self.items.iter().enumerate() {
                let display_name = item.name.as_str();
                if let Some((_indices, score)) = match_item(filter, display_name, &item.name) {
                    matches.push((idx, score));
                }
            }

            matches.sort_by(|a, b| {
                a.1.cmp(&b.1).then_with(|| {
                    let an = self.items[a.0].name.as_str();
                    let bn = self.items[b.0].name.as_str();
                    an.cmp(bn)
                })
            });

            self.filtered_indices = matches.into_iter().map(|(idx, _score)| idx).collect();
        }

        let len = self.filtered_indices.len();
        self.state.selected_idx = previously_selected
            .and_then(|actual_idx| {
                self.filtered_indices
                    .iter()
                    .position(|idx| *idx == actual_idx)
            })
            .or_else(|| (len > 0).then_some(0));

        let visible = Self::max_visible_rows(len);
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, visible);
    }

    /// Returns the number of items visible after filtering.
    fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns the maximum number of rows that can be displayed at once.
    fn max_visible_rows(len: usize) -> usize {
        MAX_POPUP_ROWS.min(len.max(1))
    }

    /// Calculates the width available for row content (accounts for borders).
    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    /// Calculates the height needed for the row list area.
    fn rows_height(&self, rows: &[GenericDisplayRow]) -> u16 {
        rows.len().clamp(1, MAX_POPUP_ROWS).try_into().unwrap_or(1)
    }

    /// Builds the display rows for all currently visible (filtered) items.
    ///
    /// Each row shows: `› [x] Item Name` where `›` indicates cursor position
    /// and `[x]` or `[ ]` indicates enabled/disabled state.
    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(visible_idx, actual_idx)| {
                self.items.get(*actual_idx).map(|item| {
                    let is_selected = self.state.selected_idx == Some(visible_idx);
                    let prefix = if is_selected { '›' } else { ' ' };
                    let marker = if item.enabled { 'x' } else { ' ' };
                    let item_name = truncate_text(&item.name, ITEM_NAME_TRUNCATE_LEN);
                    let name = format!("{prefix} [{marker}] {item_name}");
                    GenericDisplayRow {
                        name,
                        description: item.description.clone(),
                        ..Default::default()
                    }
                })
            })
            .collect()
    }

    /// Moves the selection cursor up, wrapping to the bottom if at the top.
    fn move_up(&mut self) {
        let len = self.visible_len();
        self.state.move_up_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

    /// Moves the selection cursor down, wrapping to the top if at the bottom.
    fn move_down(&mut self) {
        let len = self.visible_len();
        self.state.move_down_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

    /// Toggles the enabled state of the currently selected item.
    ///
    /// Updates the preview line and invokes the `on_change` callback if set.
    fn toggle_selected(&mut self) {
        let Some(idx) = self.state.selected_idx else {
            return;
        };
        let Some(actual_idx) = self.filtered_indices.get(idx).copied() else {
            return;
        };
        let Some(item) = self.items.get_mut(actual_idx) else {
            return;
        };

        item.enabled = !item.enabled;
        self.update_preview_line();
        if let Some(on_change) = &self.on_change {
            on_change(&self.items, &self.app_event_tx);
        }
    }

    /// Confirms the current selection and closes the picker.
    ///
    /// Collects the IDs of all enabled items and passes them to the
    /// `on_confirm` callback. Does nothing if already complete.
    fn confirm_selection(&mut self) {
        if self.complete {
            return;
        }
        self.complete = true;

        if let Some(on_confirm) = &self.on_confirm {
            let selected_ids: Vec<String> = self
                .items
                .iter()
                .filter(|item| item.enabled)
                .map(|item| item.id.clone())
                .collect();
            on_confirm(&selected_ids, &self.app_event_tx);
        }
    }

    /// Moves the currently selected item up or down in the list.
    ///
    /// Only works when:
    /// - The search query is empty (reordering is disabled during filtering)
    /// - Ordering is enabled via [`MultiSelectPickerBuilder::enable_ordering`]
    ///
    /// Updates the preview line and invokes the `on_change` callback.
    fn move_selected_item(&mut self, direction: Direction) {
        if !self.search_query.is_empty() {
            return;
        }

        let Some(visible_idx) = self.state.selected_idx else {
            return;
        };
        let Some(actual_idx) = self.filtered_indices.get(visible_idx).copied() else {
            return;
        };

        let len = self.items.len();
        if len == 0 {
            return;
        }

        let new_idx = match direction {
            Direction::Up if actual_idx > 0 => actual_idx - 1,
            Direction::Down if actual_idx + 1 < len => actual_idx + 1,
            _ => return,
        };

        // move item in underlying list
        self.items.swap(actual_idx, new_idx);

        self.update_preview_line();
        if let Some(on_change) = &self.on_change {
            on_change(&self.items, &self.app_event_tx);
        }

        // rebuild filtered indices to keep search/filter consistent
        self.apply_filter();

        // restore selection to moved item
        let moved_idx = new_idx;
        if let Some(new_visible_idx) = self
            .filtered_indices
            .iter()
            .position(|idx| *idx == moved_idx)
        {
            self.state.selected_idx = Some(new_visible_idx);
        }
    }

    /// Regenerates the preview line using the preview callback.
    ///
    /// Called after any item state change (toggle or reorder).
    fn update_preview_line(&mut self) {
        self.preview_line = self
            .preview_builder
            .as_ref()
            .and_then(|builder| builder(&self.items));
    }

    /// Closes the picker without confirming, invoking the `on_cancel` callback.
    ///
    /// Does nothing if already complete.
    pub fn close(&mut self) {
        if self.complete {
            return;
        }
        self.complete = true;
        if let Some(on_cancel) = &self.on_cancel {
            on_cancel(&self.app_event_tx);
        }
    }
}

impl BottomPaneView for MultiSelectPicker {
    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.close();
        CancellationEvent::Handled
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent { code: KeyCode::Left, .. } if self.ordering_enabled => {
                self.move_selected_item(Direction::Up);
            }
            KeyEvent { code: KeyCode::Right, .. } if self.ordering_enabled => {
                self.move_selected_item(Direction::Down);
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^P */ => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
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
                code: KeyCode::Backspace,
                ..
            } => {
                self.search_query.pop();
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.confirm_selection(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.close();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_query.push(c);
                self.apply_filter();
            }
            _ => {}
        }
    }
}

impl Renderable for MultiSelectPicker {
    fn desired_height(&self, width: u16) -> u16 {
        let rows = self.build_rows();
        let rows_height = self.rows_height(&rows);
        let preview_height = if self.preview_line.is_some() { 1 } else { 0 };

        let mut height = self.header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 3);
        height = height.saturating_add(2);
        height.saturating_add(1 + preview_height)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Reserve the footer line for the key-hint row.
        let preview_height = if self.preview_line.is_some() { 1 } else { 0 };
        let footer_height = 1 + preview_height;
        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(footer_height)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let header_height = self
            .header
            .desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = self.rows_height(&rows);
        let [header_area, _, search_area, list_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(2),
            Constraint::Length(rows_height),
        ])
        .areas(content_area.inset(Insets::vh(1, 2)));

        self.header.render(header_area, buf);

        // Render the search prompt as two lines to mimic the composer.
        if search_area.height >= 2 {
            let [placeholder_area, input_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(search_area);
            Line::from(SEARCH_PLACEHOLDER.dim()).render(placeholder_area, buf);
            let line = if self.search_query.is_empty() {
                Line::from(vec![SEARCH_PROMPT_PREFIX.dim()])
            } else {
                Line::from(vec![
                    SEARCH_PROMPT_PREFIX.dim(),
                    self.search_query.clone().into(),
                ])
            };
            line.render(input_area, buf);
        } else if search_area.height > 0 {
            let query_span = if self.search_query.is_empty() {
                SEARCH_PLACEHOLDER.dim()
            } else {
                self.search_query.clone().into()
            };
            Line::from(query_span).render(search_area, buf);
        }

        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows_single_line(
                render_area,
                buf,
                &rows,
                &self.state,
                render_area.height as usize,
                "no matches",
            );
        }

        let hint_area = if let Some(preview_line) = &self.preview_line {
            let [preview_area, hint_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer_area);
            let preview_area = Rect {
                x: preview_area.x + 2,
                y: preview_area.y,
                width: preview_area.width.saturating_sub(2),
                height: preview_area.height,
            };
            let max_preview_width = preview_area.width.saturating_sub(2) as usize;
            let preview_line =
                truncate_line_with_ellipsis_if_overflow(preview_line.clone(), max_preview_width);
            preview_line.render(preview_area, buf);
            hint_area
        } else {
            footer_area
        };
        let hint_area = Rect {
            x: hint_area.x + 2,
            y: hint_area.y,
            width: hint_area.width.saturating_sub(2),
            height: hint_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }
}

/// Builder for constructing a [`MultiSelectPicker`] with a fluent API.
///
/// # Example
///
/// ```ignore
/// let picker = MultiSelectPicker::new("Title".into(), None, tx)
///     .items(items)
///     .enable_ordering()
///     .on_preview(|items| Some(Line::from("Preview")))
///     .on_confirm(|ids, tx| { /* handle */ })
///     .on_cancel(|tx| { /* handle */ })
///     .build();
/// ```
pub(crate) struct MultiSelectPickerBuilder {
    title: String,
    subtitle: Option<String>,
    instructions: Vec<Span<'static>>,
    items: Vec<MultiSelectItem>,
    ordering_enabled: bool,
    app_event_tx: AppEventSender,
    preview_builder: Option<PreviewCallback>,
    on_change: Option<ChangeCallBack>,
    on_confirm: Option<ConfirmCallback>,
    on_cancel: Option<CancelCallback>,
}

impl MultiSelectPickerBuilder {
    /// Creates a new builder with the given title, optional subtitle, and event sender.
    pub fn new(title: String, subtitle: Option<String>, app_event_tx: AppEventSender) -> Self {
        Self {
            title,
            subtitle,
            instructions: Vec::new(),
            items: Vec::new(),
            ordering_enabled: false,
            app_event_tx,
            preview_builder: None,
            on_change: None,
            on_confirm: None,
            on_cancel: None,
        }
    }

    /// Sets the list of selectable items.
    pub fn items(mut self, items: Vec<MultiSelectItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets custom instruction spans for the footer hint line.
    ///
    /// If not set, default instructions are shown (Space to toggle, Enter to
    /// confirm, Escape to close).
    pub fn instructions(mut self, instructions: Vec<Span<'static>>) -> Self {
        self.instructions = instructions;
        self
    }

    /// Enables left/right arrow keys for reordering items.
    ///
    /// Reordering is only active when the search query is empty.
    pub fn enable_ordering(mut self) -> Self {
        self.ordering_enabled = true;
        self
    }

    /// Sets a callback to generate a preview line from the current item states.
    ///
    /// The callback receives all items and should return a [`Line`] to display,
    /// or `None` to hide the preview area.
    pub fn on_preview<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[MultiSelectItem]) -> Option<Line<'static>> + Send + Sync + 'static,
    {
        self.preview_builder = Some(Box::new(callback));
        self
    }

    /// Sets a callback invoked whenever an item's state changes.
    ///
    /// This includes both toggles and reordering operations.
    #[allow(dead_code)]
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[MultiSelectItem], &AppEventSender) + Send + Sync + 'static,
    {
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Sets a callback invoked when the user confirms their selection (Enter).
    ///
    /// The callback receives a list of IDs for all enabled items.
    pub fn on_confirm<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[String], &AppEventSender) + Send + Sync + 'static,
    {
        self.on_confirm = Some(Box::new(callback));
        self
    }

    /// Sets a callback invoked when the user cancels the picker (Escape).
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(&AppEventSender) + Send + Sync + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    /// Builds the [`MultiSelectPicker`] with all configured options.
    ///
    /// Initializes the filter to show all items and generates the initial
    /// preview line if a preview callback was set.
    pub fn build(self) -> MultiSelectPicker {
        let mut header = ColumnRenderable::new();
        header.push(Line::from(self.title.bold()));

        if let Some(subtitle) = self.subtitle {
            header.push(Line::from(subtitle.dim()));
        }

        let instructions = if self.instructions.is_empty() {
            vec![
                "Press ".into(),
                key_hint::plain(KeyCode::Char(' ')).into(),
                " to toggle; ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " to confirm and close; ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " to close".into(),
            ]
        } else {
            self.instructions
        };

        let mut view = MultiSelectPicker {
            items: self.items,
            state: ScrollState::new(),
            complete: false,
            app_event_tx: self.app_event_tx,
            header: Box::new(header),
            footer_hint: Line::from(instructions),
            ordering_enabled: self.ordering_enabled,
            search_query: String::new(),
            filtered_indices: Vec::new(),
            preview_builder: self.preview_builder,
            preview_line: None,
            on_change: self.on_change,
            on_confirm: self.on_confirm,
            on_cancel: self.on_cancel,
        };
        view.apply_filter();
        view.update_preview_line();
        view
    }
}

/// Performs fuzzy matching on an item against a filter string.
///
/// Tries to match against the display name first, then falls back to name if different. Returns
/// the matching character indices (if matched on display name) and a score for sorting.
///
/// # Arguments
///
/// * `filter` - The search query to match against
/// * `display_name` - The primary name to match (shown to user)
/// * `name` - A secondary/canonical name to try if display name doesn't match
///
/// # Returns
///
/// * `Some((Some(indices), score))` - Matched on display name with highlight indices
/// * `Some((None, score))` - Matched on skill name only (no highlights for display)
/// * `None` - No match
pub(crate) fn match_item(
    filter: &str,
    display_name: &str,
    name: &str,
) -> Option<(Option<Vec<usize>>, i32)> {
    if let Some((indices, score)) = fuzzy_match(display_name, filter) {
        return Some((Some(indices), score));
    }
    if display_name != name
        && let Some((_indices, score)) = fuzzy_match(name, filter)
    {
        return Some((None, score));
    }
    None
}
