use crate::app_event::AppEvent;
use crate::shims::EventOutcome;
use crate::shims::HostApi;
use crate::shims::ShimModule;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

/// Placeholder commands shim: consumes nothing today. Intended for F7/F8 and
/// thread picker/ops later.
#[derive(Default)]
pub(crate) struct ThreadCommandShim;

impl ThreadCommandShim {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ShimModule for ThreadCommandShim {
    fn on_app_event_with_host(
        &mut self,
        _event: &mut AppEvent,
        _host: &mut dyn HostApi,
    ) -> EventOutcome {
        // Title is controlled via the dedicated tool; this shim no longer auto-derives titles.
        EventOutcome::Continue
    }

    fn on_key_event_with_host(&mut self, event: &KeyEvent, host: &mut dyn HostApi) -> EventOutcome {
        match event {
            // Ctrl-Left: switch to previous thread
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if host.switch_prev() {
                    EventOutcome::Consumed
                } else {
                    EventOutcome::Continue
                }
            }
            // Ctrl-Right: switch to next thread
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if host.switch_next() {
                    EventOutcome::Consumed
                } else {
                    EventOutcome::Continue
                }
            }
            KeyEvent {
                code: KeyCode::F(7),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if host.switch_prev() {
                    EventOutcome::Consumed
                } else {
                    EventOutcome::Continue
                }
            }
            KeyEvent {
                code: KeyCode::F(8),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if host.switch_next() {
                    EventOutcome::Consumed
                } else {
                    EventOutcome::Continue
                }
            }
            _ => EventOutcome::Continue,
        }
    }
}

// Title derivation helpers removed.
