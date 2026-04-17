use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::render::renderable::Renderable;
use codex_protocol::request_user_input::RequestUserInputEvent;
use crossterm::event::KeyEvent;

use super::CancellationEvent;

/// Reason an active bottom-pane view finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewCompletion {
    Accepted,
    Cancelled,
}

/// Trait implemented by every view that can be shown in the bottom pane.
pub(crate) trait BottomPaneView: Renderable {
    /// Handle a key event while the view is active. A redraw is always
    /// scheduled after this call.
    fn handle_key_event(&mut self, _key_event: KeyEvent) {}

    /// Return `true` if the view has finished and should be removed.
    fn is_complete(&self) -> bool {
        false
    }

    /// Return the completion reason once the view has finished.
    fn completion(&self) -> Option<ViewCompletion> {
        None
    }

    /// Return true when this view should be removed after a child view is accepted.
    fn dismiss_after_child_accept(&self) -> bool {
        false
    }

    /// Clear any pending child-flow cleanup marker after a child view is cancelled.
    fn clear_dismiss_after_child_accept(&mut self) {}

    /// Stable identifier for views that need external refreshes while open.
    fn view_id(&self) -> Option<&'static str> {
        None
    }

    /// Actual item index for list-based views that want to preserve selection
    /// across external refreshes.
    fn selected_index(&self) -> Option<usize> {
        None
    }

    /// Active tab id for tabbed list-based views.
    #[allow(dead_code)]
    fn active_tab_id(&self) -> Option<&str> {
        None
    }

    /// Handle Ctrl-C while this view is active.
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        CancellationEvent::NotHandled
    }

    /// Return true if Esc should be routed through `handle_key_event` instead
    /// of the `on_ctrl_c` cancellation path.
    fn prefer_esc_to_handle_key_event(&self) -> bool {
        false
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

    /// Try to handle a supported MCP server elicitation form request; return the original value if
    /// not consumed.
    fn try_consume_mcp_server_elicitation_request(
        &mut self,
        request: McpServerElicitationFormRequest,
    ) -> Option<McpServerElicitationFormRequest> {
        Some(request)
    }

    /// Dismiss a request that was resolved by another client.
    ///
    /// Returns `true` when the view changed state.
    fn dismiss_app_server_request(&mut self, _request: &ResolvedAppServerRequest) -> bool {
        false
    }

    /// Whether this view means the session is blocked waiting for the user.
    ///
    /// Views that return `true` surface an "Action Required" terminal title
    /// instead of the normal working spinner so terminal tabs clearly show that
    /// Codex needs user input.
    fn terminal_title_requires_action(&self) -> bool {
        false
    }
}
