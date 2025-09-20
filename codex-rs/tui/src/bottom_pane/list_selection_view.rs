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

use crate::app_event_sender::AppEventSender;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;

/// One selectable item in the generic selection list.
pub(crate) type SelectionAction = Box<dyn Fn(&AppEventSender) + Send + Sync>;

/// Callback invoked when a multi‑select view is accepted.
/// The provided `Vec<usize>` contains the indices (into the `items` vector)
/// of all entries that are currently checked.
pub(crate) type MultiSelectAcceptAction = Box<dyn Fn(&AppEventSender, &Vec<usize>) + Send + Sync>;

pub(crate) struct SelectionItem {
    pub name: String,
    pub description: Option<String>,
    pub is_current: bool,
    pub actions: Vec<SelectionAction>,
    pub dismiss_on_select: bool,
    pub search_value: Option<String>,
}

#[derive(Default)]
pub(crate) struct SelectionViewParams {
    pub title: String,
    pub subtitle: Option<String>,
    pub footer_hint: Option<String>,
    pub items: Vec<SelectionItem>,
    pub is_searchable: bool,
    pub search_placeholder: Option<String>,
    pub empty_message: Option<String>,
    /// When true, the list supports toggling multiple items via Space and
    /// submitting all selections with Enter.
    pub is_multi_select: bool,
    /// Optional callback invoked on Enter when `is_multi_select` is true.
    /// If `None`, accepting the view will simply dismiss it.
    pub on_accept_multi: Option<MultiSelectAcceptAction>,
}

pub(crate) struct ListSelectionView {
    title: String,
    subtitle: Option<String>,
    footer_hint: Option<String>,
    items: Vec<SelectionItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    is_searchable: bool,
    search_query: String,
    search_placeholder: Option<String>,
    empty_message: Option<String>,
    filtered_indices: Vec<usize>,
    is_multi_select: bool,
    /// Set of item indices (into `items`) that are currently checked.
    checked: std::collections::HashSet<usize>,
    on_accept_multi: Option<MultiSelectAcceptAction>,
}

impl ListSelectionView {
    fn dim_prefix_span() -> Span<'static> {
        "▌ ".dim()
    }

    fn render_dim_prefix_line(area: Rect, buf: &mut Buffer) {
        let para = Paragraph::new(Line::from(Self::dim_prefix_span()));
        para.render(area, buf);
    }

    pub fn new(params: SelectionViewParams, app_event_tx: AppEventSender) -> Self {
        let mut s = Self {
            title: params.title,
            subtitle: params.subtitle,
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
            empty_message: params.empty_message,
            filtered_indices: Vec::new(),
            is_multi_select: params.is_multi_select,
            checked: Default::default(),
            on_accept_multi: params.on_accept_multi,
        };
        if s.is_multi_select {
            // Seed checked set from items marked as current.
            for (idx, it) in s.items.iter().enumerate() {
                if it.is_current {
                    s.checked.insert(idx);
                }
            }
        }
        s.apply_filter();
        s
    }

    fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    fn max_visible_rows(len: usize) -> usize {
        MAX_POPUP_ROWS.min(len.max(1))
    }

    fn apply_filter(&mut self) {
        let previously_selected = self
            .state
            .selected_idx
            .and_then(|visible_idx| self.filtered_indices.get(visible_idx).copied())
            .or_else(|| {
                (!self.is_searchable)
                    .then(|| self.items.iter().position(|item| item.is_current))
                    .flatten()
            });

        if self.is_searchable && !self.search_query.is_empty() {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_indices = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    let matches = if let Some(search_value) = &item.search_value {
                        search_value.to_lowercase().contains(&query_lower)
                    } else {
                        let mut matches = item.name.to_lowercase().contains(&query_lower);
                        if !matches && let Some(desc) = &item.description {
                            matches = desc.to_lowercase().contains(&query_lower);
                        }
                        matches
                    };
                    matches.then_some(idx)
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
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(visible_idx, actual_idx)| {
                self.items.get(*actual_idx).map(|item| {
                    let is_selected = self.state.selected_idx == Some(visible_idx);
                    let prefix = if is_selected { '>' } else { ' ' };
                    let name = item.name.as_str();
                    let name_with_marker = if self.is_multi_select {
                        // In multi‑select mode, `is_current` seeds the initial checkbox
                        // state and is reflected via [x]/[ ] rather than "(current)".
                        item.name.clone()
                    } else if item.is_current {
                        format!("{name} (current)")
                    } else {
                        item.name.clone()
                    };
                    let n = visible_idx + 1;
                    let display_name = if self.is_multi_select {
                        let actual = *actual_idx;
                        let checked = if self.checked.contains(&actual) {
                            "[x]"
                        } else {
                            "[ ]"
                        };
                        format!("{prefix} {n}. {checked} {name_with_marker}")
                    } else {
                        format!("{prefix} {n}. {name_with_marker}")
                    };
                    GenericDisplayRow {
                        name: display_name,
                        match_indices: None,
                        is_current: item.is_current,
                        description: item.description.clone(),
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

    fn accept(&mut self) {
        if self.is_multi_select {
            if let Some(cb) = &self.on_accept_multi {
                let mut selected: Vec<usize> = self.checked.iter().copied().collect();
                selected.sort_unstable();
                cb(&self.app_event_tx, &selected);
            }
            self.complete = true;
            return;
        }

        if let Some(idx) = self.state.selected_idx
            && let Some(actual_idx) = self.filtered_indices.get(idx)
            && let Some(item) = self.items.get(*actual_idx)
        {
            for act in &item.actions {
                act(&self.app_event_tx);
            }
            if item.dismiss_on_select {
                self.complete = true;
            }
        } else {
            self.complete = true;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.apply_filter();
    }
}

impl BottomPaneView for ListSelectionView {
    fn handle_key_event(&mut self, _pane: &mut BottomPane, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Char(' '),
                ..
            } if self.is_multi_select => {
                if let Some(visible_idx) = self.state.selected_idx
                    && let Some(actual_idx) = self.filtered_indices.get(visible_idx)
                {
                    if self.checked.contains(actual_idx) {
                        self.checked.remove(actual_idx);
                    } else {
                        self.checked.insert(*actual_idx);
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } if self.is_searchable => {
                self.search_query.pop();
                self.apply_filter();
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

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, width: u16) -> u16 {
        // Measure wrapped height for up to MAX_POPUP_ROWS items at the given width.
        // Build the same display rows used by the renderer so wrapping math matches.
        let rows = self.build_rows();

        let rows_height = measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width);

        // +1 for the title row, +1 for a spacer line beneath the header,
        // +1 for optional subtitle, +1 for optional footer (2 lines incl. spacing)
        let mut height = rows_height + 2;
        if self.is_searchable {
            height = height.saturating_add(1);
        }
        if self.subtitle.is_some() {
            // +1 for subtitle (the spacer is accounted for above)
            height = height.saturating_add(1);
        }
        if self.footer_hint.is_some() {
            height = height.saturating_add(2);
        }
        height
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

        let title_spans: Vec<Span<'static>> =
            vec![Self::dim_prefix_span(), self.title.clone().bold()];
        let title_para = Paragraph::new(Line::from(title_spans));
        title_para.render(title_area, buf);

        let mut next_y = area.y.saturating_add(1);
        if self.is_searchable {
            let search_area = Rect {
                x: area.x,
                y: next_y,
                width: area.width,
                height: 1,
            };
            let query_span: Span<'static> = if self.search_query.is_empty() {
                self.search_placeholder
                    .as_ref()
                    .map(|placeholder| placeholder.clone().dim())
                    .unwrap_or_else(|| "".into())
            } else {
                self.search_query.clone().into()
            };
            Paragraph::new(Line::from(vec![Self::dim_prefix_span(), query_span]))
                .render(search_area, buf);
            next_y = next_y.saturating_add(1);
        }
        if let Some(sub) = &self.subtitle {
            let subtitle_area = Rect {
                x: area.x,
                y: next_y,
                width: area.width,
                height: 1,
            };
            let subtitle_spans: Vec<Span<'static>> =
                vec![Self::dim_prefix_span(), sub.clone().dim()];
            let subtitle_para = Paragraph::new(Line::from(subtitle_spans));
            subtitle_para.render(subtitle_area, buf);
            next_y = next_y.saturating_add(1);
        }

        let spacer_area = Rect {
            x: area.x,
            y: next_y,
            width: area.width,
            height: 1,
        };
        Self::render_dim_prefix_line(spacer_area, buf);
        next_y = next_y.saturating_add(1);

        let footer_reserved = if self.footer_hint.is_some() { 2 } else { 0 };
        let rows_area = Rect {
            x: area.x,
            y: next_y,
            width: area.width,
            height: area
                .height
                .saturating_sub(next_y.saturating_sub(area.y))
                .saturating_sub(footer_reserved),
        };

        let rows = self.build_rows();
        if rows_area.height > 0 {
            render_rows(
                rows_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                true,
                self.empty_message.as_deref().unwrap_or("no matches"),
            );
        }

        if let Some(hint) = &self.footer_hint {
            let footer_area = Rect {
                x: area.x,
                y: area.y + area.height - 1,
                width: area.width,
                height: 1,
            };
            let footer_para = Paragraph::new(hint.clone().dim());
            footer_para.render(footer_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BottomPaneView;
    use super::*;
    use crate::app_event::AppEvent;
    use crate::bottom_pane::BottomPane;
    use crate::bottom_pane::BottomPaneParams;
    use crate::bottom_pane::popup_consts::STANDARD_POPUP_HINT_LINE;
    use crate::tui::FrameRequester;
    use insta::assert_snapshot;
    use ratatui::layout::Rect;
    // KeyEvent::new suffices; the view doesn't check kind/state.
    use tokio::sync::mpsc::unbounded_channel;

    fn make_selection_view(subtitle: Option<&str>) -> ListSelectionView {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "Read Only".to_string(),
                description: Some("Codex can read files".to_string()),
                is_current: true,
                actions: vec![],
                dismiss_on_select: true,
                search_value: None,
            },
            SelectionItem {
                name: "Full Access".to_string(),
                description: Some("Codex can edit files".to_string()),
                is_current: false,
                actions: vec![],
                dismiss_on_select: true,
                search_value: None,
            },
        ];
        ListSelectionView::new(
            SelectionViewParams {
                title: "Select Approval Mode".to_string(),
                subtitle: subtitle.map(str::to_string),
                footer_hint: Some(STANDARD_POPUP_HINT_LINE.to_string()),
                items,
                ..Default::default()
            },
            tx,
        )
    }

    fn render_lines(view: &ListSelectionView) -> String {
        let width = 48;
        let height = BottomPaneView::desired_height(view, width);
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
    fn renders_search_query_line_when_enabled() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![SelectionItem {
            name: "Read Only".to_string(),
            description: Some("Codex can read files".to_string()),
            is_current: false,
            actions: vec![],
            dismiss_on_select: true,
            search_value: None,
        }];
        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: "Select Approval Mode".to_string(),
                footer_hint: Some(STANDARD_POPUP_HINT_LINE.to_string()),
                items,
                is_searchable: true,
                search_placeholder: Some("Type to search branches".to_string()),
                empty_message: Some("no matches".to_string()),
                ..Default::default()
            },
            tx,
        );
        view.set_search_query("filters".to_string());

        let lines = render_lines(&view);
        assert!(lines.contains("▌ filters"));
    }

    #[test]
    fn multi_select_toggles_and_accepts() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SelectionItem {
                name: "Read Only".to_string(),
                description: Some("Codex can read files".to_string()),
                is_current: true, // pre-checked via seed
                actions: vec![],
                dismiss_on_select: false,
                search_value: None,
            },
            SelectionItem {
                name: "Full Access".to_string(),
                description: Some("Codex can edit files".to_string()),
                is_current: false,
                actions: vec![],
                dismiss_on_select: false,
                search_value: None,
            },
            SelectionItem {
                name: "Third".to_string(),
                description: None,
                is_current: false,
                actions: vec![],
                dismiss_on_select: false,
                search_value: None,
            },
        ];

        let accepted: Arc<Mutex<Option<Vec<usize>>>> = Arc::new(Mutex::new(None));
        let accepted_clone = accepted.clone();

        let mut view = ListSelectionView::new(
            SelectionViewParams {
                title: "Experimental features".to_string(),
                footer_hint: Some(STANDARD_POPUP_HINT_LINE.to_string()),
                items,
                is_multi_select: true,
                on_accept_multi: Some(Box::new(move |_tx, selected| {
                    *accepted_clone.lock().unwrap() = Some(selected.clone());
                })),
                ..Default::default()
            },
            tx,
        );

        // Initially, first item should be checked ([x]) via is_current seed
        let initial = render_lines(&view);
        assert!(initial.contains("1. [x] Read Only"));
        assert!(initial.contains("2. [ ] Full Access"));

        // Build a minimal BottomPane to deliver key events to the view.
        let (tx2_raw, _rx2) = unbounded_channel::<AppEvent>();
        let tx2 = AppEventSender::new(tx2_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx2,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: String::new(),
            disable_paste_burst: false,
            include_comment_command: false,
        });

        // Toggle first item off with Space
        view.handle_key_event(
            &mut pane,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        let after_toggle = render_lines(&view);
        assert!(after_toggle.contains("1. [ ] Read Only"));

        // Move to second item and toggle it on
        view.handle_key_event(&mut pane, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        view.handle_key_event(
            &mut pane,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        let after_second = render_lines(&view);
        assert!(after_second.contains("2. [x] Full Access"));

        // Accept selection with Enter: should call the callback with [1]
        assert!(!view.is_complete());
        view.handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(view.is_complete());

        let accepted_vec = accepted.lock().unwrap().clone().unwrap();
        assert_eq!(accepted_vec, vec![1]);
    }
}
