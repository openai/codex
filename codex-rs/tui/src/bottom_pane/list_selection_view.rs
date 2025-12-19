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
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::app_event_sender::AppEventSender;
use crate::key_hint::KeyBinding;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use unicode_width::UnicodeWidthStr;

/// One selectable item in the generic selection list.
pub(crate) type SelectionAction = Box<dyn Fn(&AppEventSender) + Send + Sync>;

#[derive(Default)]
pub(crate) struct SelectionItem {
    pub name: String,
    pub display_shortcut: Option<KeyBinding>,
    pub description: Option<String>,
    pub selected_description: Option<String>,
    pub is_current: bool,
    pub is_default: bool,
    pub actions: Vec<SelectionAction>,
    pub dismiss_on_select: bool,
    pub search_value: Option<String>,
    pub disabled_reason: Option<String>,
}

pub(crate) struct SelectionViewParams {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub footer_hint: Option<Line<'static>>,
    pub items: Vec<SelectionItem>,
    pub is_searchable: bool,
    pub search_placeholder: Option<String>,
    pub header: Box<dyn Renderable>,
    pub initial_selected_idx: Option<usize>,
}

impl Default for SelectionViewParams {
    fn default() -> Self {
        Self {
            title: None,
            subtitle: None,
            footer_hint: None,
            items: Vec::new(),
            is_searchable: false,
            search_placeholder: None,
            header: Box::new(()),
            initial_selected_idx: None,
        }
    }
}

pub(crate) struct ListSelectionView {
    footer_hint: Option<Line<'static>>,
    items: Vec<SelectionItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    is_searchable: bool,
    search_query: String,
    search_placeholder: Option<String>,
    filtered_indices: Vec<usize>,
    last_selected_actual_idx: Option<usize>,
    header: Box<dyn Renderable>,
    initial_selected_idx: Option<usize>,
}

impl ListSelectionView {
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
            filtered_indices: Vec::new(),
            last_selected_actual_idx: None,
            header,
            initial_selected_idx: params.initial_selected_idx,
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

    fn apply_filter(&mut self) {
        let previously_selected = self
            .state
            .selected_idx
            .and_then(|visible_idx| self.filtered_indices.get(visible_idx).copied())
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
                    let display_name = format!("{wrap_prefix}{name_with_marker}");
                    let description = is_selected
                        .then(|| item.selected_description.clone())
                        .flatten()
                        .or_else(|| item.description.clone());
                    let wrap_indent = description.is_none().then_some(wrap_prefix_width);
                    GenericDisplayRow {
                        name: display_name,
                        display_shortcut: item.display_shortcut,
                        match_indices: None,
                        description,
                        wrap_indent,
                        disabled_reason: item.disabled_reason.clone(),
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
        self.skip_disabled_up();
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        self.state.move_down_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
        self.skip_disabled_down();
    }

    fn accept(&mut self) {
        if let Some(idx) = self.state.selected_idx
            && let Some(actual_idx) = self.filtered_indices.get(idx)
            && let Some(item) = self.items.get(*actual_idx)
            && item.disabled_reason.is_none()
        {
            self.last_selected_actual_idx = Some(*actual_idx);
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

    pub(crate) fn take_last_selected_index(&mut self) -> Option<usize> {
        self.last_selected_actual_idx.take()
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn skip_disabled_down(&mut self) {
        let len = self.visible_len();
        for _ in 0..len {
            if let Some(idx) = self.state.selected_idx
                && let Some(actual_idx) = self.filtered_indices.get(idx)
                && self
                    .items
                    .get(*actual_idx)
                    .is_some_and(|item| item.disabled_reason.is_some())
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
                    .is_some_and(|item| item.disabled_reason.is_some())
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
                        .is_some_and(|item| item.disabled_reason.is_none())
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

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for ListSelectionView {
    fn desired_height(&self, width: u16) -> u16 {
        // Measure wrapped height for up to MAX_POPUP_ROWS items at the given width.
        // Build the same display rows used by the renderer so wrapping math matches.
        let rows = self.build_rows();
        let rows_width = Self::rows_width(width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );

        // Subtract 4 for the padding on the left and right of the header.
        let mut height = self.header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 3);
        if self.is_searchable {
            height = height.saturating_add(1);
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

        let [content_area, footer_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(if self.footer_hint.is_some() { 1 } else { 0 }),
        ])
        .areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let header_height = self
            .header
            // Subtract 4 for the padding on the left and right of the header.
            .desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );
        let [header_area, _, search_area, list_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(if self.is_searchable { 1 } else { 0 }),
            Constraint::Length(rows_height),
        ])
        .areas(content_area.inset(Insets::vh(1, 2)));

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

        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows(
                render_area,
                buf,
                &rows,
                &self.state,
                render_area.height as usize,
                "no matches",
            );
        }

        if let Some(hint) = &self.footer_hint {
            let hint_area = Rect {
                x: footer_area.x + 2,
                y: footer_area.y,
                width: footer_area.width.saturating_sub(2),
                height: footer_area.height,
            };
            hint.clone().dim().render(hint_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::bottom_pane::popup_consts::standard_popup_hint_line;
    use insta::assert_snapshot;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

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
        let height = view.desired_height(width);
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
}
