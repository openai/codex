use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget as _;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

pub(crate) struct LoginScreen {
    app_event_tx: AppEventSender,

    /// Use this with login_with_chatgpt() in login/src/lib.rs and, if
    /// successful, update the in-memory config via
    /// codex_core::openai_api_key::set_openai_api_key().
    #[allow(dead_code)]
    codex_home: PathBuf,
}

impl LoginScreen {
    pub(crate) fn new(app_event_tx: AppEventSender, codex_home: PathBuf) -> Self {
        Self {
            app_event_tx,
            codex_home,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let KeyCode::Char('q') = key_event.code {
            self.app_event_tx.send(AppEvent::ExitRequest);
        }
    }
}

impl WidgetRef for &LoginScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let text = Paragraph::new(
            "Login using `codex login` and then run this command again. 'q' to quit.",
        );
        text.render(area, buf);
    }
}
