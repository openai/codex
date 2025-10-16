use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::Renderable;
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
