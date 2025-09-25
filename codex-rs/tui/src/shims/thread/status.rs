use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::app_event::AppEvent;
use crate::shims::EventOutcome;
use crate::shims::HostApi;
use crate::shims::ShimModule;
use crossterm::event::KeyEvent;

/// Placeholder status shim: currently a no-op. In future, this will compute
/// a status line reflecting thread/session state.
#[derive(Default)]
pub(crate) struct ThreadStatusShim;

impl ThreadStatusShim {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ShimModule for ThreadStatusShim {
    fn on_app_event(&mut self, _event: &mut AppEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn on_key_event(&mut self, _event: &KeyEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn augment_status_line_with_host(&self, line: &mut Option<Line<'static>>, host: &dyn HostApi) {
        // Left-side identity: session status line starting with a label.
        // Example: "session: main-2 • Refactor API". Title is dim; name uses default fg.
        let name = host.active_display_name();
        let title_opt = host.active_title().filter(|t| !t.trim().is_empty());

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push("session:".dim());
        spans.push(" ".into());
        spans.push(Span::from(name));
        if let Some(title) = title_opt {
            spans.push(" • ".dim());
            spans.push(Span::from(title).dim());
        }
        *line = Some(Line::from(spans));
    }

    fn augment_header_with_host(&self, _lines: &mut Vec<Line<'static>>, _host: &dyn HostApi) {}
}
