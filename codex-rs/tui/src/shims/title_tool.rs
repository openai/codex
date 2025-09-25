use crate::app_event::AppEvent;
use crate::shims::EventOutcome;
use crate::shims::HostApi;
use crate::shims::ShimModule;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;

/// Shim that listens for background events from the core signaling a session
/// title update and applies them to the active session/thread in the UI.
#[derive(Default)]
pub(crate) struct TitleToolShim;

impl TitleToolShim {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ShimModule for TitleToolShim {
    fn on_app_event_with_host(
        &mut self,
        event: &mut AppEvent,
        host: &mut dyn HostApi,
    ) -> EventOutcome {
        if let AppEvent::CodexEvent(Event { msg, .. }) = event
            && let EventMsg::BackgroundEvent(ev) = msg
        {
            // Expect messages of the form: "codex:set_session_title:<title>"
            const PREFIX: &str = "codex:set_session_title:";
            if let Some(rest) = ev.message.strip_prefix(PREFIX) {
                // Store the title exactly as provided by the tool call
                host.set_active_title(rest.to_string());
                // Allow the app layer to see this event to refresh any open views.
                return EventOutcome::Continue;
            }
        }
        EventOutcome::Continue
    }
}
