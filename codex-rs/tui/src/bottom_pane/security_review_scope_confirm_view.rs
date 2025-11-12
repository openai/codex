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
use tokio::sync::oneshot;

use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::render::renderable::Renderable;
use crate::security_review::SecurityReviewMode;
use crate::text_formatting::truncate_text;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;

pub(crate) struct SecurityReviewScopeConfirmView {
    mode: SecurityReviewMode,
    prompt: String,
    selections: Vec<SecurityReviewAutoScopeSelection>,
    responder: Option<oneshot::Sender<bool>>,
    complete: bool,
}

impl SecurityReviewScopeConfirmView {
    pub(crate) fn new(
        mode: SecurityReviewMode,
        prompt: String,
        selections: Vec<SecurityReviewAutoScopeSelection>,
        responder: oneshot::Sender<bool>,
    ) -> Self {
        Self {
            mode,
            prompt,
            selections,
            responder: Some(responder),
            complete: false,
        }
    }

    fn send_response(&mut self, accept: bool) {
        if let Some(responder) = self.responder.take() {
            let _ = responder.send(accept);
        }
        self.complete = true;
    }
}

impl BottomPaneView for SecurityReviewScopeConfirmView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.send_response(true);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.send_response(false);
            }
            _ if key_event.modifiers.contains(KeyModifiers::CONTROL) => {}
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.send_response(false);
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, _pasted: String) -> bool {
        false
    }
}

impl Renderable for SecurityReviewScopeConfirmView {
    fn desired_height(&self, _width: u16) -> u16 {
        let base_lines: u16 = 5;
        let selection_lines = if self.selections.is_empty() {
            1
        } else {
            self.selections.len() as u16
        };
        base_lines.saturating_add(selection_lines)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        Clear.render(area, buf);

        let mut lines: Vec<Line> = Vec::new();
        lines.push(vec!["Confirm auto-detected scope".bold()].into());
        lines.push(vec![format!("Mode: {}", self.mode.as_str()).dim()].into());

        if !self.prompt.trim().is_empty() {
            let summary = truncate_text(self.prompt.trim(), 96);
            lines.push(vec!["Prompt: ".dim(), Span::from(summary)].into());
        }

        if self.selections.is_empty() {
            lines.push(
                vec!["No specific directories selected; review the entire repository.".dim()]
                    .into(),
            );
        } else {
            for (idx, selection) in self.selections.iter().enumerate() {
                let label = format!("{:>2}. {}", idx + 1, selection.display_path);
                let mut spans: Vec<Span> = vec![Span::from(label)];
                if let Some(reason) = selection.reason.as_ref() {
                    spans.push(" — ".dim());
                    spans.push(Span::from(reason.clone()).dim());
                }
                lines.push(spans.into());
            }
        }

        lines.push(Line::from(Vec::<Span>::new()));
        lines.push(
            vec![
                "Continue with these paths? ".into(),
                "(y)es".bold(),
                " / ".into(),
                "(n)o to refine scope".bold(),
            ]
            .into(),
        );

        Paragraph::new(lines).render(area, buf);
    }
}
