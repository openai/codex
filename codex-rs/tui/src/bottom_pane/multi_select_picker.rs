use codex_common::fuzzy_match::fuzzy_match;
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
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::render_rows_single_line;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;
use crate::text_formatting::truncate_text;

const ITEM_NAME_TRUNCATE_LEN: usize = 21;
const SEARCH_PLACEHOLDER: &str = "Type to search";
const SEARCH_PROMPT_PREFIX: &str = "> ";

enum Direction {
    Up,
    Down,
}

pub type ChangeCallBack = Box<dyn Fn(&[MultiSelectItem], &AppEventSender) + Send + Sync>;
pub type ConfirmCallback = Box<dyn Fn(&[String], &AppEventSender) + Send + Sync>;
pub type CancelCallback = Box<dyn Fn(&AppEventSender) + Send + Sync>;
pub type PreviewCallback = Box<dyn Fn(&[MultiSelectItem]) -> Option<Line<'static>> + Send + Sync>;

#[derive(Default)]
pub(crate) struct MultiSelectItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub search_value: Option<String>,
}

pub(crate) struct MultiSelectPicker {
    items: Vec<MultiSelectItem>,
    state: ScrollState,
    pub(crate) complete: bool,
    app_event_tx: AppEventSender,
    header: Box<dyn Renderable>,
    footer_hint: Line<'static>,
    search_query: String,
    filtered_indices: Vec<usize>,
    ordering_enabled: bool,
    preview_builder: Option<PreviewCallback>,
    preview_line: Option<Line<'static>>,
    on_change: Option<ChangeCallBack>,
    on_confirm: Option<ConfirmCallback>,
    #[allow(dead_code)]
    on_cancel: Option<CancelCallback>,
}

impl MultiSelectPicker {
    pub fn new(
        title: String,
        subtitle: Option<String>,
        app_event_tx: AppEventSender,
    ) -> MultiSelectPickerBuilder {
        MultiSelectPickerBuilder::new(title, subtitle, app_event_tx)
    }

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

    fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    fn max_visible_rows(len: usize) -> usize {
        MAX_POPUP_ROWS.min(len.max(1))
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn rows_height(&self, rows: &[GenericDisplayRow]) -> u16 {
        rows.len().clamp(1, MAX_POPUP_ROWS).try_into().unwrap_or(1)
    }

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

    fn move_up(&mut self) {
        let len = self.visible_len();
        self.state.move_up_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        self.state.move_down_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

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

    fn update_preview_line(&mut self) {
        self.preview_line = self
            .preview_builder
            .as_ref()
            .and_then(|builder| builder(&self.items));
    }

    pub fn close(&mut self) {
        if self.complete {
            return;
        }
        self.complete = true;
    }
}

impl BottomPaneView for MultiSelectPicker {
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
                self.on_ctrl_c();
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
            preview_line.clone().render(preview_area, buf);
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

    pub fn items(mut self, items: Vec<MultiSelectItem>) -> Self {
        self.items = items;
        self
    }

    pub fn instructions(mut self, instructions: Vec<Span<'static>>) -> Self {
        self.instructions = instructions;
        self
    }

    pub fn enable_ordering(mut self) -> Self {
        self.ordering_enabled = true;
        self
    }

    pub fn on_preview<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[MultiSelectItem]) -> Option<Line<'static>> + Send + Sync + 'static,
    {
        self.preview_builder = Some(Box::new(callback));
        self
    }

    #[allow(dead_code)]
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[MultiSelectItem], &AppEventSender) + Send + Sync + 'static,
    {
        self.on_change = Some(Box::new(callback));
        self
    }

    pub fn on_confirm<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[String], &AppEventSender) + Send + Sync + 'static,
    {
        self.on_confirm = Some(Box::new(callback));
        self
    }

    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(&AppEventSender) + Send + Sync + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
        self
    }

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
                " or ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " to toggle; ".into(),
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

pub(crate) fn match_item(
    filter: &str,
    display_name: &str,
    skill_name: &str,
) -> Option<(Option<Vec<usize>>, i32)> {
    if let Some((indices, score)) = fuzzy_match(display_name, filter) {
        return Some((Some(indices), score));
    }
    if display_name != skill_name
        && let Some((_indices, score)) = fuzzy_match(skill_name, filter)
    {
        return Some((None, score));
    }
    None
}
