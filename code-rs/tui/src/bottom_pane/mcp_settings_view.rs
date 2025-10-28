#![allow(dead_code)]

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

use super::bottom_pane_view::BottomPaneView;
use super::BottomPane;

#[derive(Clone, Debug)]
pub(crate) struct McpServerRow {
    pub name: String,
    pub enabled: bool,
    pub summary: String,
}

pub(crate) type McpServerRows = Vec<McpServerRow>;

pub(crate) struct McpSettingsView {
    rows: McpServerRows,
    selected: usize,
    is_complete: bool,
    app_event_tx: AppEventSender,
}

impl McpSettingsView {
    pub fn new(rows: McpServerRows, app_event_tx: AppEventSender) -> Self {
        Self { rows, selected: 0, is_complete: false, app_event_tx }
    }

    fn len(&self) -> usize { self.rows.len().saturating_add(2) /* + Add, + Close */ }

    fn on_toggle(&mut self) {
        if self.selected < self.rows.len() {
            let row = &mut self.rows[self.selected];
            let new_enabled = !row.enabled;
            row.enabled = new_enabled;
            self.app_event_tx.send(AppEvent::UpdateMcpServer { name: row.name.clone(), enable: new_enabled });
        }
    }

    fn on_enter(&mut self) {
        match self.selected {
            idx if idx < self.rows.len() => self.on_toggle(),
            idx if idx == self.rows.len() => {
                // Add New… row
                self.app_event_tx.send(AppEvent::PrefillComposer("/mcp add ".to_string()));
                self.is_complete = true;
            }
            _ => { self.is_complete = true; }
        }
    }

    fn process_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent { code: KeyCode::Up, .. } => {
                if self.selected == 0 {
                    self.selected = self.len().saturating_sub(1);
                } else {
                    self.selected -= 1;
                }
            }
            KeyEvent { code: KeyCode::Down, .. } => {
                self.selected = (self.selected + 1) % self.len().max(1);
            }
            KeyEvent { code: KeyCode::Left | KeyCode::Right, .. }
            | KeyEvent { code: KeyCode::Char(' '), modifiers: KeyModifiers::NONE, .. } => {
                self.on_toggle();
            }
            KeyEvent { code: KeyCode::Enter, .. } => self.on_enter(),
            KeyEvent { code: KeyCode::Esc, .. } => {
                self.is_complete = true;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_key_event_direct(&mut self, key_event: KeyEvent) -> bool {
        let handled = matches!(
            key_event,
            KeyEvent { code: KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Esc, .. }
                | KeyEvent { code: KeyCode::Char(' '), modifiers: KeyModifiers::NONE, .. }
        );
        self.process_key_event(key_event);
        handled
    }
}

impl<'a> BottomPaneView<'a> for McpSettingsView {
    fn handle_key_event(&mut self, _pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        self.process_key_event(key_event);
    }

    fn is_complete(&self) -> bool { self.is_complete }

    fn desired_height(&self, _width: u16) -> u16 { 16 }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(crate::colors::border()))
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .title(" MCP Servers ")
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut selected_line_index: usize = 0;

        if self.rows.is_empty() {
            lines.push(Line::from(vec![Span::styled("No MCP servers configured.", Style::default().fg(crate::colors::text_dim()))]));
            lines.push(Line::from(""));
        }

        for (i, row) in self.rows.iter().enumerate() {
            let sel = i == self.selected;
            let check = if row.enabled { "[on ]" } else { "[off]" };
            let name = format!("{} {}", check, row.name);
            let name_style = if sel { Style::default().bg(crate::colors::selection()).add_modifier(Modifier::BOLD) } else { Style::default() };
            let arrow_line_index = lines.len();
            lines.push(Line::from(vec![
                Span::styled(if sel { "› " } else { "  " }, Style::default()),
                Span::styled(name, name_style),
            ]));
            // Summary line
            let sum_style = if sel { Style::default().bg(crate::colors::selection()).fg(crate::colors::secondary()) } else { Style::default().fg(crate::colors::text_dim()) };
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(row.summary.clone(), sum_style),
            ]));
            if sel {
                selected_line_index = arrow_line_index;
            }
        }

        // Add New…
        let add_sel = self.selected == self.rows.len();
        let add_style = if add_sel { Style::default().bg(crate::colors::selection()).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(""));
        let add_line_index = lines.len();
        lines.push(Line::from(vec![Span::styled(if add_sel { "› " } else { "  " }, Style::default()), Span::styled("Add new server…", add_style)]));
        if add_sel {
            selected_line_index = add_line_index;
        }

        // Close
        let close_sel = self.selected == self.rows.len().saturating_add(1);
        let close_style = if close_sel { Style::default().bg(crate::colors::selection()).add_modifier(Modifier::BOLD) } else { Style::default() };
        let close_line_index = lines.len();
        lines.push(Line::from(vec![Span::styled(if close_sel { "› " } else { "  " }, Style::default()), Span::styled("Close", close_style)]));
        if close_sel {
            selected_line_index = close_line_index;
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("↑↓/←→", Style::default().fg(crate::colors::function())),
            Span::styled(" Navigate/Toggle  ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Enter", Style::default().fg(crate::colors::success())),
            Span::styled(" Toggle/Open  ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Esc", Style::default().fg(crate::colors::error())),
            Span::styled(" Close", Style::default().fg(crate::colors::text_dim())),
        ]));

        let total_lines = lines.len();
        let viewport_height = inner.height as usize;
        let selected_line_index = selected_line_index.min(total_lines.saturating_sub(1));
        let mut scroll_top = 0usize;
        if viewport_height > 0 && total_lines > viewport_height {
            let half = viewport_height / 2;
            let mut candidate = selected_line_index.saturating_sub(half);
            let max_scroll = total_lines - viewport_height;
            if candidate > max_scroll {
                candidate = max_scroll;
            }
            scroll_top = candidate;
        }

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .scroll((scroll_top as u16, 0));
        paragraph.render(Rect { x: inner.x.saturating_add(1), y: inner.y, width: inner.width.saturating_sub(2), height: inner.height }, buf);
    }
}
