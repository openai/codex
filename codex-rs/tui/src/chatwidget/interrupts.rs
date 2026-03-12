use std::collections::VecDeque;

use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::request_permissions::RequestPermissionsEvent;

use super::ChatWidget;
use crate::bottom_pane::ThreadUserInputRequest;

#[derive(Debug)]
pub(crate) enum QueuedInterrupt {
    ExecApproval {
        thread_id: ThreadId,
        event: ExecApprovalRequestEvent,
    },
    ApplyPatchApproval {
        thread_id: ThreadId,
        event: ApplyPatchApprovalRequestEvent,
    },
    Elicitation {
        thread_id: ThreadId,
        event: ElicitationRequestEvent,
    },
    RequestPermissions {
        thread_id: ThreadId,
        event: RequestPermissionsEvent,
    },
    RequestUserInput(ThreadUserInputRequest),
    ExecBegin(ExecCommandBeginEvent),
    ExecEnd(ExecCommandEndEvent),
    McpBegin(McpToolCallBeginEvent),
    McpEnd(McpToolCallEndEvent),
    PatchEnd(PatchApplyEndEvent),
}

#[derive(Default)]
pub(crate) struct InterruptManager {
    queue: VecDeque<QueuedInterrupt>,
}

impl InterruptManager {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(crate) fn push_exec_approval(&mut self, thread_id: ThreadId, ev: ExecApprovalRequestEvent) {
        self.queue.push_back(QueuedInterrupt::ExecApproval {
            thread_id,
            event: ev,
        });
    }

    pub(crate) fn push_apply_patch_approval(
        &mut self,
        thread_id: ThreadId,
        ev: ApplyPatchApprovalRequestEvent,
    ) {
        self.queue.push_back(QueuedInterrupt::ApplyPatchApproval {
            thread_id,
            event: ev,
        });
    }

    pub(crate) fn push_elicitation(&mut self, thread_id: ThreadId, ev: ElicitationRequestEvent) {
        self.queue.push_back(QueuedInterrupt::Elicitation {
            thread_id,
            event: ev,
        });
    }

    pub(crate) fn push_request_permissions(
        &mut self,
        thread_id: ThreadId,
        ev: RequestPermissionsEvent,
    ) {
        self.queue.push_back(QueuedInterrupt::RequestPermissions {
            thread_id,
            event: ev,
        });
    }

    pub(crate) fn push_user_input(&mut self, request: ThreadUserInputRequest) {
        self.queue
            .push_back(QueuedInterrupt::RequestUserInput(request));
    }

    pub(crate) fn push_exec_begin(&mut self, ev: ExecCommandBeginEvent) {
        self.queue.push_back(QueuedInterrupt::ExecBegin(ev));
    }

    pub(crate) fn push_exec_end(&mut self, ev: ExecCommandEndEvent) {
        self.queue.push_back(QueuedInterrupt::ExecEnd(ev));
    }

    pub(crate) fn push_mcp_begin(&mut self, ev: McpToolCallBeginEvent) {
        self.queue.push_back(QueuedInterrupt::McpBegin(ev));
    }

    pub(crate) fn push_mcp_end(&mut self, ev: McpToolCallEndEvent) {
        self.queue.push_back(QueuedInterrupt::McpEnd(ev));
    }

    pub(crate) fn push_patch_end(&mut self, ev: PatchApplyEndEvent) {
        self.queue.push_back(QueuedInterrupt::PatchEnd(ev));
    }

    pub(crate) fn flush_all(&mut self, chat: &mut ChatWidget) {
        while let Some(q) = self.queue.pop_front() {
            match q {
                QueuedInterrupt::ExecApproval { thread_id, event } => {
                    chat.handle_exec_approval_for_thread(thread_id, event)
                }
                QueuedInterrupt::ApplyPatchApproval { thread_id, event } => {
                    chat.handle_apply_patch_approval_for_thread(thread_id, event)
                }
                QueuedInterrupt::Elicitation { thread_id, event } => {
                    chat.handle_elicitation_request_for_thread(thread_id, event)
                }
                QueuedInterrupt::RequestPermissions { thread_id, event } => {
                    chat.handle_request_permissions_for_thread(thread_id, event)
                }
                QueuedInterrupt::RequestUserInput(request) => {
                    chat.handle_request_user_input_now(request);
                }
                QueuedInterrupt::ExecBegin(ev) => chat.handle_exec_begin_now(ev),
                QueuedInterrupt::ExecEnd(ev) => chat.handle_exec_end_now(ev),
                QueuedInterrupt::McpBegin(ev) => chat.handle_mcp_begin_now(ev),
                QueuedInterrupt::McpEnd(ev) => chat.handle_mcp_end_now(ev),
                QueuedInterrupt::PatchEnd(ev) => chat.handle_patch_apply_end_now(ev),
            }
        }
    }
}
