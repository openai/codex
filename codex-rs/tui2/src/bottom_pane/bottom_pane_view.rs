//! Contracts for modal views hosted inside the bottom pane overlay stack.
//!
//! Bottom-pane views temporarily replace the composer while they are active, and the bottom pane
//! drives their lifecycle by routing input, rendering them, and removing them when they report
//! completion. Each view owns its own state, may intercept paste or cancellation input, and
//! chooses whether to consume approval requests or forward them, which lets a single overlay
//! coordinate multi-step prompts before the stack advances.
use crate::bottom_pane::ApprovalRequest;
use crate::render::renderable::Renderable;
use crossterm::event::KeyEvent;

use super::CancellationEvent;

/// Defines the behavior every bottom-pane overlay must provide.
///
/// Implementations handle input routing, track completion state, and optionally consume approval
/// requests so a single overlay can manage multiple related prompts. The bottom pane owns the
/// lifecycle and will drop the view once it reports completion.
pub(crate) trait BottomPaneView: Renderable {
    /// Handle a key event while the view is active.
    ///
    /// The bottom pane always schedules a redraw after this call, so handlers should update local
    /// state and set completion flags as needed instead of requesting rendering directly.
    fn handle_key_event(&mut self, _key_event: KeyEvent) {}

    /// Report whether the view has finished and should be removed.
    ///
    /// Implementations should treat completion as a terminal state; once `true`, the view must be
    /// safe to drop and should continue to return `true` on subsequent checks.
    fn is_complete(&self) -> bool {
        false
    }

    /// Handle Ctrl-C while this view is active.
    ///
    /// Returning [`CancellationEvent::Handled`] prevents the bottom pane from applying its default
    /// cancellation behavior, which allows overlays to intercept the key and clean up their state.
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        CancellationEvent::NotHandled
    }

    /// Handle pasted text when this view is active.
    ///
    /// Return `true` when the paste mutates state so the caller can schedule a redraw.
    fn handle_paste(&mut self, _pasted: String) -> bool {
        false
    }

    /// Offer an approval request to the view before it is routed elsewhere.
    ///
    /// Return `None` to consume the request and retain it in local state, or `Some(request)` to
    /// forward the request unchanged to the next overlay.
    fn try_consume_approval_request(
        &mut self,
        request: ApprovalRequest,
    ) -> Option<ApprovalRequest> {
        Some(request)
    }
}
