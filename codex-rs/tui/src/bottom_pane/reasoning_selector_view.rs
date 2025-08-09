use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::colors::LIGHT_BLUE;
use crate::slash_command::SlashCommand;
use codex_core::config_types::ReasoningEffort;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use ratatui::prelude::Widget;

/// Modal-style selector for choosing reasoning effort.
pub(crate) struct ReasoningSelectorView {
    state: ScrollState,
    options: Vec<(String, ReasoningEffort)>,
    current: ReasoningEffort,
    app_event_tx: AppEventSender,
    done: bool,
}

impl ReasoningSelectorView {
    pub(crate) fn new(current: ReasoningEffort, app_event_tx: AppEventSender) -> Self {
        let options = vec![
            ("low".to_string(), ReasoningEffort::Low),
            ("medium".to_string(), ReasoningEffort::Medium),
            ("high".to_string(), ReasoningEffort::High),
        ];

        let mut state = ScrollState::new();
        // Default selection to current effort when present among options; otherwise first.
        let default_idx = options
            .iter()
            .position(|(_, eff)| *eff == current)
            .unwrap_or(0);
        state.selected_idx = Some(default_idx);
        state.ensure_visible(options.len(), options.len().min(MAX_POPUP_ROWS));

        Self {
            state,
            options,
            current,
            app_event_tx,
            done: false,
        }
    }

    fn confirm_selection(&mut self) {
        if let Some(idx) = self.state.selected_idx {
            if let Some((name, _eff)) = self.options.get(idx) {
                // Dispatch `/reasoning <name>` to the app layer.
                self.app_event_tx.send(AppEvent::DispatchCommandWithArgs {
                    cmd: SlashCommand::Reasoning,
                    args: name.clone(),
                });
                self.done = true;
            }
        }
    }
}

impl<'a> BottomPaneView<'a> for ReasoningSelectorView {
    fn handle_key_event(&mut self, _pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                let len = self.options.len();
                self.state.move_up_wrap(len);
                self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                let len = self.options.len();
                self.state.move_down_wrap(len);
                self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.confirm_selection();
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.done = true; // cancel
            }
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane<'a>) -> CancellationEvent {
        self.done = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn desired_height(&self, _width: u16) -> u16 {
        // Enough rows to show all options or cap by MAX_POPUP_ROWS.
        self.options.len().clamp(1, MAX_POPUP_ROWS) as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Draw a left border to match other popups, then render content in the inner area.
        let border = Block::default()
            .borders(Borders::LEFT)
            .border_type(BorderType::QuadrantOutside)
            .border_style(Style::default().fg(Color::DarkGray));
        border.render(area, buf);

        // Inner area avoids drawing over the left border when width permits.
        let inner = if area.width > 1 {
            Rect {
                x: area.x + 1,
                y: area.y,
                width: area.width - 1,
                height: area.height,
            }
        } else {
            area
        };

        // Compute visible window from scroll state; height is capped by MAX_POPUP_ROWS via desired_height().
        let total = self.options.len();
        let max_rows_from_area = inner.height as usize;
        let visible_rows = MAX_POPUP_ROWS.min(total).min(max_rows_from_area.max(1));
        let mut start_idx = self.state.scroll_top.min(total.saturating_sub(1));
        if let Some(sel) = self.state.selected_idx {
            if sel < start_idx {
                start_idx = sel;
            } else if visible_rows > 0 {
                let bottom = start_idx + visible_rows - 1;
                if sel > bottom {
                    start_idx = sel + 1 - visible_rows;
                }
            }
        }

        // Build lines with numbering and a caret for the highlighted selection.
        let mut lines: Vec<Line> = Vec::new();
        for (i, (name, eff)) in self
            .options
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(visible_rows)
        {
            let is_selected = Some(i) == self.state.selected_idx;
            let is_current = *eff == self.current;

            if is_selected {
                // Selected row: use LIGHT_BLUE like the onboarding auth screen.
                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::styled(
                    format!("> {}. ", i + 1),
                    Style::default().fg(LIGHT_BLUE).add_modifier(Modifier::DIM),
                ));
                spans.push(Span::styled(name.clone(), Style::default().fg(LIGHT_BLUE)));
                lines.push(Line::from(spans));
            } else {
                // Non-selected row: plain text with numbering; add a subtle current marker when applicable.
                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::from(format!("  {}. ", i + 1)));
                spans.push(Span::from(name.clone()));
                if is_current {
                    spans.push(Span::from("  "));
                    spans.push(Span::styled(
                        "(current)",
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                lines.push(Line::from(spans));
            }
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}
