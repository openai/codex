use crate::bottom_pane::ApprovalRequest;
use crate::render::renderable::Renderable;
use codex_protocol::request_user_input::RequestUserInputEvent;
use crossterm::event::KeyEvent;

use super::CancellationEvent;

/// Trait implemented by every view that can be shown in the bottom pane.
pub(crate) trait BottomPaneView: Renderable {
    /// Handle a key event while the view is active. A redraw is always
    /// scheduled after this call.
    fn handle_key_event(&mut self, _key_event: KeyEvent) {}

    /// Return `true` if the view has finished and should be removed.
    fn is_complete(&self) -> bool {
        false
    }

    /// Handle Ctrl-C while this view is active.
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        CancellationEvent::NotHandled
    }

    /// Optional paste handler. Return true if the view modified its state and
    /// needs a redraw.
    fn handle_paste(&mut self, _pasted: String) -> bool {
        false
    }

    /// Flush any pending paste-burst state. Return true if state changed.
    ///
    /// This lets a modal that reuses `ChatComposer` participate in the same
    /// time-based paste burst flushing as the primary composer.
    fn flush_paste_burst_if_due(&mut self) -> bool {
        false
    }

    /// Whether the view is currently holding paste-burst transient state.
    ///
    /// When `true`, the bottom pane will schedule a short delayed redraw to
    /// give the burst time window a chance to flush.
    fn is_in_paste_burst(&self) -> bool {
        false
    }

    /// Try to handle approval request; return the original value if not
    /// consumed.
    fn try_consume_approval_request(
        &mut self,
        request: ApprovalRequest,
    ) -> Option<ApprovalRequest> {
        Some(request)
    }

    /// Try to handle request_user_input; return the original value if not
    /// consumed.
    fn try_consume_user_input_request(
        &mut self,
        request: RequestUserInputEvent,
    ) -> Option<RequestUserInputEvent> {
        Some(request)
    }
}
