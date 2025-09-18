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

pub(crate) struct SelectionItem {
    pub name: String,
    pub description: Option<String>,
    pub is_current: bool,
    pub actions: Vec<SelectionAction>,
}

pub(crate) struct ListSelectionView {
    title: String,
    subtitle: Option<String>,
    footer_hint: Option<String>,
    items: Vec<SelectionItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
}

impl ListSelectionView {
    fn dim_prefix_span() -> Span<'static> {
        "▌ ".dim()
    }

    fn render_dim_prefix_line(area: Rect, buf: &mut Buffer) {
        let para = Paragraph::new(Line::from(Self::dim_prefix_span()));
        para.render(area, buf);
    }
    pub fn new(
        title: String,
        subtitle: Option<String>,
        footer_hint: Option<String>,
        items: Vec<SelectionItem>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut s = Self {
            title,
            subtitle,
            footer_hint,
            items,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
        };
        let len = s.items.len();
        if let Some(idx) = s.items.iter().position(|it| it.is_current) {
            s.state.selected_idx = Some(idx);
        }
        s.state.clamp_selection(len);
        s.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
        s
    }

    fn move_up(&mut self) {
        let len = self.items.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn move_down(&mut self) {
        let len = self.items.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn accept(&mut self) {
        if let Some(idx) = self.state.selected_idx {
            if let Some(item) = self.items.get(idx) {
                for act in &item.actions {
                    act(&self.app_event_tx);
                }
                self.complete = true;
            }
        } else {
            self.complete = true;
        }
    }

    fn cancel(&mut self) {
        // Close the popup without performing any actions.
        self.complete = true;
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
                code: KeyCode::Esc, ..
            } => self.cancel(),
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
        let rows: Vec<GenericDisplayRow> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let is_selected = self.state.selected_idx == Some(i);
                let prefix = if is_selected { '>' } else { ' ' };
                let name_with_marker = if it.is_current {
                    format!("{} (current)", it.name)
                } else {
                    it.name.clone()
                };
                let display_name = format!("{} {}. {}", prefix, i + 1, name_with_marker);
                GenericDisplayRow {
                    name: display_name,
                    match_indices: None,
                    is_current: it.is_current,
                    description: it.description.clone(),
                }
            })
            .collect();

        let rows_height = measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width);

        // +1 for the title row, +1 for a spacer line beneath the header,
        // +1 for optional subtitle, +1 for optional footer (2 lines incl. spacing)
        let mut height = rows_height + 2;
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

        let rows: Vec<GenericDisplayRow> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let is_selected = self.state.selected_idx == Some(i);
                let prefix = if is_selected { '>' } else { ' ' };
                let name_with_marker = if it.is_current {
                    format!("{} (current)", it.name)
                } else {
                    it.name.clone()
                };
                let display_name = format!("{} {}. {}", prefix, i + 1, name_with_marker);
                GenericDisplayRow {
                    name: display_name,
                    match_indices: None,
                    is_current: it.is_current,
                    description: it.description.clone(),
                }
            })
            .collect();
        if rows_area.height > 0 {
            render_rows(
                rows_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                true,
                "no matches",
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
                actions: vec![],
            },
            SelectionItem {
                name: "Full Access".to_string(),
                description: Some("Codex can edit files".to_string()),
                is_current: false,
                actions: vec![],
            },
        ];
        ListSelectionView::new(
            "Select Approval Mode".to_string(),
            subtitle.map(str::to_string),
            Some("Press Enter to confirm or Esc to go back".to_string()),
            items,
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
}
