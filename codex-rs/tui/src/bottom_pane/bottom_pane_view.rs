use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::bottom_pane::ThreadUserInputRequest;
use crate::render::renderable::Renderable;
use codex_protocol::ThreadId;
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

    /// Stable identifier for views that need external refreshes while open.
    fn view_id(&self) -> Option<&'static str> {
        None
    }

    /// Actual item index for list-based views that want to preserve selection
    /// across external refreshes.
    fn selected_index(&self) -> Option<usize> {
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
        request: ThreadUserInputRequest,
    ) -> Option<ThreadUserInputRequest> {
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

    /// Whether this view should remain visible after a turn interrupt clears
    /// turn-scoped approvals.
    fn preserve_on_turn_interrupt(&self) -> bool {
        false
    }

    /// Owning thread for overlays scoped to a specific Codex thread.
    fn thread_id(&self) -> Option<ThreadId> {
        None
    }

    /// Drop interrupted-thread state while preserving queued work for other
    /// threads when possible. Returns `true` if the view should remain visible.
    fn dismiss_on_turn_interrupt(&mut self, interrupted_thread_id: ThreadId) -> bool {
        self.preserve_on_turn_interrupt()
            || self
                .thread_id()
                .is_some_and(|thread_id| thread_id != interrupted_thread_id)
    }
}
