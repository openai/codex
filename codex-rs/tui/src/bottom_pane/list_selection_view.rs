use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::any::Any;

use crate::app_event_sender::AppEventSender;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use super::standard_popup_hint_line;

/// One selectable item in the generic selection list.
pub(crate) type SelectionAction = Box<dyn Fn(&mut BottomPane, &AppEventSender) + Send + Sync>;

pub(crate) struct SelectionItem {
    pub name: String,
    pub description: Option<String>,
    pub is_current: bool,
    pub actions: Vec<SelectionAction>,
    pub styled_label: Option<Vec<Span<'static>>>,
    pub dismiss_on_select: bool,
}

pub(crate) struct ListSelectionView {
    title: String,
    subtitle: Option<String>,
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
        if area.width == 0 || area.height == 0 {
            return;
        }
        Clear.render(area, buf);
        let para = Paragraph::new(Line::from(Self::dim_prefix_span()));
        para.render(area, buf);
    }
    pub fn new(
        title: String,
        subtitle: Option<String>,
        items: Vec<SelectionItem>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut s = Self {
            title,
            subtitle,
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

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn update_item<F>(&mut self, index: usize, mut update: F)
    where
        F: FnMut(&mut SelectionItem),
    {
        if let Some(item) = self.items.get_mut(index) {
            update(item);
        }
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

    fn accept(&mut self, pane: &mut BottomPane) {
        if let Some(idx) = self.state.selected_idx {
            if let Some(item) = self.items.get(idx) {
                for act in &item.actions {
                    act(pane, &self.app_event_tx);
                }
                if item.dismiss_on_select {
                    self.complete = true;
                }
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
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn handle_key_event(&mut self, pane: &mut BottomPane, key_event: KeyEvent) {
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

    fn desired_height(&self, _width: u16) -> u16 {
        let rows = (self.items.len()).clamp(1, MAX_POPUP_ROWS);
        // +1 for the title row, +1 for a spacer line beneath the header,
        // +1 for optional subtitle, +1 for optional footer
        let mut height = rows as u16 + 2;
        if self.subtitle.is_some() {
            // +1 for subtitle (the spacer is accounted for above)
            height = height.saturating_add(1);
        }
        height.saturating_add(2)
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

        let footer_reserved = 2;
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
                let number = i + 1;
                let label_prefix = format!("{prefix} {number}. ");
                let styled_name = if let Some(styled_label) = it.styled_label.as_ref() {
                    let mut spans = Vec::new();
                    spans.push(label_prefix.into());
                    spans.extend(styled_label.clone());
                    Some(spans)
                } else {
                    None
                };
                let name_with_marker = if it.is_current {
                    format!("{} (current)", it.name)
                } else {
                    it.name.clone()
                };
                let display_name = format!("{prefix} {number}. {name_with_marker}");
                GenericDisplayRow {
                    name: display_name,
                    match_indices: None,
                    is_current: it.is_current,
                    description: it.description.clone(),
                    styled_name,
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

        if area.height >= 2 {
            let spacer_area = Rect {
                x: area.x,
                y: area.y + area.height - 2,
                width: area.width,
                height: 1,
            };
            Clear.render(spacer_area, buf);
        }
        let footer_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        Paragraph::new(standard_popup_hint_line()).render(footer_area, buf);
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
                styled_label: None,
                dismiss_on_select: true,
            },
            SelectionItem {
                name: "Full Access".to_string(),
                description: Some("Codex can edit files".to_string()),
                is_current: false,
                actions: vec![],
                styled_label: None,
                dismiss_on_select: true,
            },
        ];
        ListSelectionView::new(
            "Select Approval Mode".to_string(),
            subtitle.map(str::to_string),
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
