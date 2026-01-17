//! Queues interrupt-style events until the chat widget is ready to handle them.
//!
//! The chat widget receives protocol events that can interrupt normal rendering (approval
//! requests, tool call boundaries, execution begin/end notifications). This module buffers those
//! events in arrival order and provides a single flush point so the UI can drain them once the
//! widget is in a safe state to update.
//!
//! The manager owns only the queue; it does not interpret events beyond routing them to the
//! matching `ChatWidget` handler. Callers decide when to enqueue (typically during streaming) and
//! when to flush (typically after rendering or state transitions).
//!
//! Ordering is preserved with a FIFO queue, and flushing always routes to the corresponding
//! immediate handler on [`ChatWidget`].

use std::collections::VecDeque;

use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_protocol::approvals::ElicitationRequestEvent;

use super::ChatWidget;

/// A deferred interrupt event awaiting delivery to [`ChatWidget`].
#[derive(Debug)]
pub(crate) enum QueuedInterrupt {
    /// Request for a user decision on an exec command, keyed by approval id.
    ExecApproval(String, ExecApprovalRequestEvent),

    /// Request for a user decision on a patch application, keyed by approval id.
    ApplyPatchApproval(String, ApplyPatchApprovalRequestEvent),

    /// Out-of-band elicitation prompt to show in the UI.
    Elicitation(ElicitationRequestEvent),

    /// Execution begin boundary, used to create or update history cells.
    ExecBegin(ExecCommandBeginEvent),

    /// Execution end boundary, used to finalize history cells.
    ExecEnd(ExecCommandEndEvent),

    /// MCP tool call begin boundary for history cell tracking.
    McpBegin(McpToolCallBeginEvent),

    /// MCP tool call end boundary for history cell tracking.
    McpEnd(McpToolCallEndEvent),

    /// Patch application completion notification.
    ///
    /// This signals that a patch has finished applying and any related UI can
    /// transition from "working" to a terminal state.
    PatchEnd(PatchApplyEndEvent),
}

/// FIFO buffer for interrupt events that must be applied in order.
#[derive(Default)]
pub(crate) struct InterruptManager {
    /// Pending interrupt events in arrival order.
    queue: VecDeque<QueuedInterrupt>,
}

impl InterruptManager {
    /// Create an empty interrupt queue.
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Return true if there are no queued interrupts to process.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Queue an exec approval request until the UI can render it.
    pub(crate) fn push_exec_approval(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        self.queue.push_back(QueuedInterrupt::ExecApproval(id, ev));
    }

    /// Queue a patch approval request until the UI can render it.
    pub(crate) fn push_apply_patch_approval(
        &mut self,
        id: String,
        ev: ApplyPatchApprovalRequestEvent,
    ) {
        self.queue
            .push_back(QueuedInterrupt::ApplyPatchApproval(id, ev));
    }

    /// Queue an elicitation request until the UI can render it.
    pub(crate) fn push_elicitation(&mut self, ev: ElicitationRequestEvent) {
        self.queue.push_back(QueuedInterrupt::Elicitation(ev));
    }

    /// Queue an exec-begin event until the UI can update history cells.
    pub(crate) fn push_exec_begin(&mut self, ev: ExecCommandBeginEvent) {
        self.queue.push_back(QueuedInterrupt::ExecBegin(ev));
    }

    /// Queue an exec-end event until the UI can update history cells.
    pub(crate) fn push_exec_end(&mut self, ev: ExecCommandEndEvent) {
        self.queue.push_back(QueuedInterrupt::ExecEnd(ev));
    }

    /// Queue an MCP tool call begin event until the UI can update history cells.
    pub(crate) fn push_mcp_begin(&mut self, ev: McpToolCallBeginEvent) {
        self.queue.push_back(QueuedInterrupt::McpBegin(ev));
    }

    /// Queue an MCP tool call end event until the UI can update history cells.
    pub(crate) fn push_mcp_end(&mut self, ev: McpToolCallEndEvent) {
        self.queue.push_back(QueuedInterrupt::McpEnd(ev));
    }

    /// Queue a patch end event until the UI can update history cells.
    pub(crate) fn push_patch_end(&mut self, ev: PatchApplyEndEvent) {
        self.queue.push_back(QueuedInterrupt::PatchEnd(ev));
    }

    /// Drain all queued interrupts into the `ChatWidget` in FIFO order.
    ///
    /// This hands each buffered event to the corresponding immediate handler,
    /// preserving arrival ordering so the UI reflects the same causal sequence
    /// as the backend stream.
    pub(crate) fn flush_all(&mut self, chat: &mut ChatWidget) {
        while let Some(q) = self.queue.pop_front() {
            match q {
                QueuedInterrupt::ExecApproval(id, ev) => chat.handle_exec_approval_now(id, ev),
                QueuedInterrupt::ApplyPatchApproval(id, ev) => {
                    chat.handle_apply_patch_approval_now(id, ev)
                }
                QueuedInterrupt::Elicitation(ev) => chat.handle_elicitation_request_now(ev),
                QueuedInterrupt::ExecBegin(ev) => chat.handle_exec_begin_now(ev),
                QueuedInterrupt::ExecEnd(ev) => chat.handle_exec_end_now(ev),
                QueuedInterrupt::McpBegin(ev) => chat.handle_mcp_begin_now(ev),
                QueuedInterrupt::McpEnd(ev) => chat.handle_mcp_end_now(ev),
                QueuedInterrupt::PatchEnd(ev) => chat.handle_patch_apply_end_now(ev),
            }
        }
    }
}
