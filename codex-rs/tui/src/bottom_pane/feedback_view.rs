use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;
use codex_protocol::ConversationId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;
use std::path::PathBuf;

pub(crate) struct FeedbackView {
    app_event_tx: AppEventSender,
    file_path: PathBuf,
    snapshot: codex_feedback::CodexLogSnapshot,
    session_id: Option<ConversationId>,
    complete: bool,
}

impl FeedbackView {
    pub fn new(
        app_event_tx: AppEventSender,
        file_path: PathBuf,
        snapshot: codex_feedback::CodexLogSnapshot,
        session_id: Option<ConversationId>,
    ) -> Self {
        Self {
            app_event_tx,
            file_path,
            snapshot,
            session_id,
            complete: false,
        }
    }

    fn header_lines(&self) -> Vec<Line<'static>> {
        vec![
            Line::from("Upload Codex logs to Sentry".bold()),
            "".into(),
            Line::from(
                "Logs might contain the entire conversion history of this Codex process (prompt, tools calls and their results).",
            ),
            Line::from(
                "Logs are persisted for 90 days and are used exclusively to help diagnose issues.",
            ),
            "".into(),
            Line::from(vec![
                "You can inspect the exact content of logs to be uploaded at:".into(),
            ]),
            Line::from(self.file_path.display().to_string().dim()),
            "".into(),
            Line::from("Press Enter to upload to Sentry, or Esc to cancel".dim()),
        ]
    }

    fn render_inner(&self, area: Rect, buf: &mut Buffer) {
        let header_height: u16 = self
            .header_lines()
            .iter()
            .map(|l| l.desired_height(area.width))
            .sum();
        let [header_rect] = Layout::vertical([Constraint::Length(header_height)]).areas(area);

        for (i, line) in self.header_lines().into_iter().enumerate() {
            let line_area = Rect::new(
                header_rect.x,
                header_rect.y + i as u16,
                header_rect.width,
                1,
            )
            .intersection(header_rect);
            line.render(line_area, buf);
        }
    }
}

impl Renderable for FeedbackView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        Block::default()
            .style(user_message_style())
            .render(area, buf);
        let inner = area.inset(Insets::vh(1, 2));
        self.render_inner(inner, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let insets = Insets::vh(1, 2);
        let inner_width = width.saturating_sub(insets.left + insets.right);
        let header_height: u16 = self
            .header_lines()
            .iter()
            .map(|l| l.desired_height(inner_width))
            .sum();
        header_height + insets.top + insets.bottom
    }
}

impl BottomPaneView for FeedbackView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Enter => {
                match self.snapshot.upload_to_sentry(self.session_id) {
                    Ok(()) => {
                        let issue_url = format!(
                            "https://github.com/openai/codex/issues/new?template=2-bug-report.yml&steps=Uploaded%20thread:%20{}",
                            self.session_id.unwrap_or_default()
                        );

                        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                            PlainHistoryCell::new(
                                vec![
                                    Line::from("• Uploaded Codex logs to Sentry. Please open an issue using the following URL:"), 
                                    "".into(),
                                Line::from(vec!["  ".into(), issue_url.cyan().underlined()])
                                ],
                            )),
                        ));
                    }
                    Err(e) => {
                        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                            history_cell::new_error_event(format!(
                                "Failed to upload feedback logs: {e}"
                            )),
                        )));
                    }
                }
                self.complete = true;
            }
            KeyCode::Esc => {
                self.complete = true;
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn cursor_pos(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::style::user_message_style;
    use codex_feedback::CodexFeedback;
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use std::path::PathBuf;
    use tokio::sync::mpsc::unbounded_channel;

    fn buffer_to_string(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..buffer.area.width {
                    let symbol = buffer[(buffer.area.x + col, buffer.area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_feedback_view_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let app_event_tx = AppEventSender::new(tx_raw);
        let snapshot = CodexFeedback::new().snapshot();
        let file_path = PathBuf::from("/tmp/codex-feedback.log");

        let view = FeedbackView::new(
            app_event_tx,
            file_path.clone(),
            snapshot,
            /* session_id */ None,
        );

        let width = 72;
        let height = view.desired_height(width).max(1);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, height);
                view.render(area, frame.buffer_mut());
            })
            .unwrap();

        let rendered = buffer_to_string(terminal.backend().buffer())
            .replace(&file_path.display().to_string(), "<LOG_PATH>");
        assert_snapshot!("feedback_view_render", rendered);

        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let cell_style = buf[(area.x, area.y)].style();
        let expected_bg = user_message_style().bg.unwrap_or(Color::Reset);
        assert_eq!(cell_style.bg.unwrap_or(Color::Reset), expected_bg);
    }
}
