use ratatui::text::Line;

use crate::app_event::AppEvent;
use crate::shims::EventOutcome;
use crate::shims::HostApi;
use crate::shims::ShimModule;
use crossterm::event::KeyEvent;

/// Placeholder header shim: currently a no-op. In future, this will compute
/// banner/path lines based on active session and augment the header.
#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct ThreadHeaderShim;

impl ThreadHeaderShim {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ShimModule for ThreadHeaderShim {
    fn on_app_event(&mut self, _event: &mut AppEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn on_key_event(&mut self, _event: &KeyEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn augment_header_with_host(&self, _lines: &mut Vec<Line<'static>>, _host: &dyn HostApi) {}
}
