use crate::app_event_sender::AppEventSender;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::any::Any;
use std::cell::RefCell;

use super::BottomPane;
use super::bottom_pane_view::BottomPaneView;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use super::standard_popup_hint_line;

pub(crate) type TablePickerOnSelected<T> =
    Box<dyn Fn(&AppEventSender, &mut BottomPane, T) + Send + Sync>;

pub(crate) struct TablePickerItem<T: Clone> {
    pub value: T,
    pub label: String,
    pub description: Option<String>,
    pub search_value: String,
    pub detail_builder: Option<Box<dyn Fn() -> Option<Vec<Span<'static>>>>>,
}

pub(crate) struct SearchableTablePickerView<T: Clone> {
    title: String,
    search_placeholder: String,
    empty_message: String,
    items: Vec<TablePickerItem<T>>,
    filtered_indices: Vec<usize>,
    query: String,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    on_selected: TablePickerOnSelected<T>,
    max_visible_rows: usize,
    detail_cache: Vec<RefCell<Option<Vec<Span<'static>>>>>,
}

impl<T: Clone> SearchableTablePickerView<T> {
    fn clear_area(area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let x_end = area.x.saturating_add(area.width);
        let y_end = area.y.saturating_add(area.height);
        for y in area.y..y_end {
            for x in area.x..x_end {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
    }

    fn render_left_gutter_line(area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        Self::clear_area(area, buf);
        Paragraph::new(Line::from("▌ ".dim())).render(area, buf);
    }

    pub(crate) fn new(
        title: String,
        search_placeholder: String,
        empty_message: String,
        items: Vec<TablePickerItem<T>>,
        max_visible_rows: usize,
        app_event_tx: AppEventSender,
        on_selected: TablePickerOnSelected<T>,
    ) -> Self {
        let max_visible_rows = max_visible_rows.max(1);
        let mut view = Self {
            title,
            search_placeholder,
            empty_message,
            items,
            filtered_indices: Vec::new(),
            query: String::new(),
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            on_selected,
            max_visible_rows,
            detail_cache: Vec::new(),
        };
        view.reset_detail_cache();
        view.apply_filter();
        view
    }

    fn detail_for(&self, idx: usize) -> Option<Vec<Span<'static>>> {
        let cache = self.detail_cache.get(idx)?;
        if let Some(detail) = cache.borrow().clone() {
            return Some(detail);
        }
        let detail = self
            .items
            .get(idx)
            .and_then(|item| item.detail_builder.as_ref())
            .and_then(|builder| builder());
        cache.replace(detail.clone());
        detail
    }

    fn reset_detail_cache(&mut self) {
        self.detail_cache = self.items.iter().map(|_| RefCell::new(None)).collect();
    }

    fn apply_filter(&mut self) {
        self.filtered_indices = if self.query.is_empty() {
            (0..self.items.len()).collect()
        } else {
            let query_lower = self.query.to_lowercase();
            self.items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    let haystack = item.search_value.to_lowercase();
                    haystack.contains(&query_lower).then_some(idx)
                })
                .collect()
        };

        let len = self.filtered_indices.len();
        self.state.clamp_selection(len);
        self.state
            .ensure_visible(len, len.min(self.max_visible_rows).max(1));
    }

    fn move_up(&mut self) {
        let len = self.filtered_indices.len();
        self.state.move_up_wrap(len);
        self.state
            .ensure_visible(len, len.min(self.max_visible_rows).max(1));
    }

    fn move_down(&mut self) {
        let len = self.filtered_indices.len();
        self.state.move_down_wrap(len);
        self.state
            .ensure_visible(len, len.min(self.max_visible_rows).max(1));
    }

    fn accept(&mut self, pane: &mut BottomPane) {
        if let Some(selected_idx) = self.state.selected_idx
            && let Some(actual_idx) = self.filtered_indices.get(selected_idx)
            && let Some(item) = self.items.get(*actual_idx)
        {
            (self.on_selected)(&self.app_event_tx, pane, item.value.clone());
            self.complete = true;
        } else {
            self.complete = true;
        }
    }

    fn cancel(&mut self) {
        self.complete = true;
    }

    fn render_title(&self, area: Rect, buf: &mut Buffer) {
        let spans = vec!["▌ ".dim(), self.title.clone().bold()];
        Paragraph::new(Line::from(spans)).render(area, buf);
    }

    fn render_search(&self, area: Rect, buf: &mut Buffer) {
        let line = if self.query.is_empty() {
            Line::from(vec!["▌ ".dim(), self.search_placeholder.clone().dim()])
        } else {
            let query = &self.query;
            Line::from(vec!["▌ ".dim(), query.into()])
        };
        Paragraph::new(line).render(area, buf);
    }
}

impl<T: Clone + 'static> BottomPaneView for SearchableTablePickerView<T> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn handle_key_event(&mut self, pane: &mut super::BottomPane, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => self.cancel(),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.accept(pane),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.query.pop();
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                ..
            } => {
                if !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT)
                {
                    self.query.push(c);
                    self.apply_filter();
                }
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self, _pane: &mut super::BottomPane) -> super::CancellationEvent {
        self.cancel();
        super::CancellationEvent::Handled
    }

    fn desired_height(&self, _width: u16) -> u16 {
        let rows = self
            .filtered_indices
            .len()
            .clamp(1, self.max_visible_rows)
            .max(1) as u16;
        5 + rows
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        self.render_title(title_area, buf);

        let search_area = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: 1,
        };
        self.render_search(search_area, buf);

        let (spacer_height, rows_offset) = if area.height > 2 { (1, 3) } else { (0, 2) };
        if spacer_height == 1 {
            let spacer_area = Rect {
                x: area.x,
                y: area.y.saturating_add(2),
                width: area.width,
                height: 1,
            };
            Self::render_left_gutter_line(spacer_area, buf);
        }

        let remaining_height = area.height.saturating_sub(rows_offset);
        let hint_reserved = match remaining_height {
            h if h >= 2 => 2,
            1 => 1,
            _ => 0,
        };
        let rows_area = Rect {
            x: area.x,
            y: area.y.saturating_add(rows_offset),
            width: area.width,
            height: remaining_height.saturating_sub(hint_reserved),
        };

        let rows: Vec<GenericDisplayRow> = self
            .filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(visible_idx, source_idx)| {
                self.items.get(*source_idx).map(|item| {
                    let is_selected = self.state.selected_idx == Some(visible_idx);
                    let prefix = if is_selected { '>' } else { ' ' };
                    let number = visible_idx + 1;
                    let label = &item.label;
                    let prefix_str = format!("{prefix} {number}. ");
                    let display = format!("{prefix_str}{label}");
                    let detail_spans = if is_selected {
                        self.detail_for(*source_idx)
                    } else {
                        None
                    };
                    let styled_name = detail_spans.map(|mut detail| {
                        let mut spans: Vec<Span<'static>> = Vec::new();
                        spans.push(display.clone().into());
                        if !detail.is_empty() {
                            spans.push(" ".into());
                        }
                        spans.append(&mut detail);
                        spans
                    });
                    GenericDisplayRow {
                        name: display,
                        match_indices: None,
                        is_current: false,
                        description: item.description.clone(),
                        styled_name,
                    }
                })
            })
            .collect();

        render_rows(
            rows_area,
            buf,
            &rows,
            &self.state,
            self.max_visible_rows,
            true,
            &self.empty_message,
        );

        if hint_reserved > 0 {
            let hint_y = area.y.saturating_add(area.height).saturating_sub(1);
            if hint_y >= area.y {
                let hint_area = Rect {
                    x: area.x,
                    y: hint_y,
                    width: area.width,
                    height: 1,
                };
                Paragraph::new(standard_popup_hint_line()).render(hint_area, buf);
            }

            if hint_reserved >= 2 {
                let spacer_y = hint_y.saturating_sub(1);
                if spacer_y >= area.y {
                    let spacer_area = Rect {
                        x: area.x,
                        y: spacer_y,
                        width: area.width,
                        height: 1,
                    };
                    Self::clear_area(spacer_area, buf);
                }
            }
        }
    }
}
