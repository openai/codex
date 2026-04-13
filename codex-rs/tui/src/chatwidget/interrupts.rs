use std::collections::VecDeque;

use crate::app::app_server_requests::ResolvedAppServerRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_user_input::RequestUserInputEvent;

use super::ChatWidget;

#[derive(Debug)]
pub(crate) enum QueuedInterrupt {
    ExecApproval(ExecApprovalRequestEvent),
    ApplyPatchApproval(ApplyPatchApprovalRequestEvent),
    Elicitation(ElicitationRequestEvent),
    RequestPermissions(RequestPermissionsEvent),
    RequestUserInput(RequestUserInputEvent),
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

    pub(crate) fn push_exec_approval(&mut self, ev: ExecApprovalRequestEvent) {
        self.queue.push_back(QueuedInterrupt::ExecApproval(ev));
    }

    pub(crate) fn push_apply_patch_approval(&mut self, ev: ApplyPatchApprovalRequestEvent) {
        self.queue
            .push_back(QueuedInterrupt::ApplyPatchApproval(ev));
    }

    pub(crate) fn push_elicitation(&mut self, ev: ElicitationRequestEvent) {
        self.queue.push_back(QueuedInterrupt::Elicitation(ev));
    }

    pub(crate) fn push_request_permissions(&mut self, ev: RequestPermissionsEvent) {
        self.queue
            .push_back(QueuedInterrupt::RequestPermissions(ev));
    }

    pub(crate) fn push_user_input(&mut self, ev: RequestUserInputEvent) {
        self.queue.push_back(QueuedInterrupt::RequestUserInput(ev));
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

    pub(crate) fn remove_resolved_prompt(&mut self, request: &ResolvedAppServerRequest) -> bool {
        let original_len = self.queue.len();
        self.queue
            .retain(|queued| !queued.matches_resolved_prompt(request));
        self.queue.len() != original_len
    }

    pub(crate) fn flush_all(&mut self, chat: &mut ChatWidget) {
        while let Some(q) = self.queue.pop_front() {
            match q {
                QueuedInterrupt::ExecApproval(ev) => chat.handle_exec_approval_now(ev),
                QueuedInterrupt::ApplyPatchApproval(ev) => chat.handle_apply_patch_approval_now(ev),
                QueuedInterrupt::Elicitation(ev) => chat.handle_elicitation_request_now(ev),
                QueuedInterrupt::RequestPermissions(ev) => chat.handle_request_permissions_now(ev),
                QueuedInterrupt::RequestUserInput(ev) => chat.handle_request_user_input_now(ev),
                QueuedInterrupt::ExecBegin(ev) => chat.handle_exec_begin_now(ev),
                QueuedInterrupt::ExecEnd(ev) => chat.handle_exec_end_now(ev),
                QueuedInterrupt::McpBegin(ev) => chat.handle_mcp_begin_now(ev),
                QueuedInterrupt::McpEnd(ev) => chat.handle_mcp_end_now(ev),
                QueuedInterrupt::PatchEnd(ev) => chat.handle_patch_apply_end_now(ev),
            }
        }
    }
}

impl QueuedInterrupt {
    fn matches_resolved_prompt(&self, request: &ResolvedAppServerRequest) -> bool {
        match self {
            QueuedInterrupt::ExecApproval(ev) => {
                matches!(request, ResolvedAppServerRequest::ExecApproval { id }
                    if ev.effective_approval_id() == id.as_str())
            }
            QueuedInterrupt::ApplyPatchApproval(ev) => {
                matches!(request, ResolvedAppServerRequest::FileChangeApproval { id }
                    if ev.call_id == id.as_str())
            }
            QueuedInterrupt::Elicitation(ev) => {
                matches!(request, ResolvedAppServerRequest::McpElicitation {
                    server_name,
                    request_id,
                } if ev.server_name == server_name.as_str() && &ev.id == request_id)
            }
            QueuedInterrupt::RequestPermissions(ev) => {
                matches!(request, ResolvedAppServerRequest::PermissionsApproval { id }
                    if ev.call_id == id.as_str())
            }
            QueuedInterrupt::RequestUserInput(ev) => {
                matches!(request, ResolvedAppServerRequest::UserInput { call_id }
                    if ev.call_id == call_id.as_str())
            }
            QueuedInterrupt::ExecBegin(_)
            | QueuedInterrupt::ExecEnd(_)
            | QueuedInterrupt::McpBegin(_)
            | QueuedInterrupt::McpEnd(_)
            | QueuedInterrupt::PatchEnd(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::approvals::ExecApprovalRequestEvent;
    use codex_protocol::protocol::ExecCommandBeginEvent;
    use codex_protocol::protocol::ExecCommandSource;
    use codex_protocol::request_user_input::RequestUserInputEvent;
    use pretty_assertions::assert_eq;

    use super::*;

    fn user_input(call_id: &str, turn_id: &str) -> RequestUserInputEvent {
        RequestUserInputEvent {
            call_id: call_id.to_string(),
            turn_id: turn_id.to_string(),
            questions: Vec::new(),
        }
    }

    fn exec_approval(call_id: &str, approval_id: Option<&str>) -> ExecApprovalRequestEvent {
        ExecApprovalRequestEvent {
            call_id: call_id.to_string(),
            approval_id: approval_id.map(str::to_string),
            turn_id: "turn".to_string(),
            command: vec!["true".to_string()],
            cwd: PathBuf::from("."),
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
            parsed_cmd: Vec::new(),
        }
    }

    fn exec_begin(call_id: &str) -> ExecCommandBeginEvent {
        ExecCommandBeginEvent {
            call_id: call_id.to_string(),
            process_id: None,
            turn_id: "turn".to_string(),
            command: vec!["true".to_string()],
            cwd: PathBuf::from("."),
            parsed_cmd: Vec::new(),
            source: ExecCommandSource::Agent,
            interaction_input: None,
        }
    }

    #[test]
    fn remove_resolved_prompt_removes_matching_user_input_only() {
        let mut manager = InterruptManager::new();
        manager.push_user_input(user_input("call-a", "turn"));
        manager.push_user_input(user_input("call-b", "turn"));

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::UserInput {
                call_id: "call-b".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        let Some(QueuedInterrupt::RequestUserInput(remaining)) = manager.queue.front() else {
            panic!("expected remaining queued user input");
        };
        assert_eq!(remaining.call_id, "call-a");
    }

    #[test]
    fn remove_resolved_prompt_matches_exec_approval_id() {
        let mut manager = InterruptManager::new();
        manager.push_exec_approval(exec_approval("call", Some("approval")));

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                id: "call".to_string(),
            })
        );
        assert_eq!(manager.queue.len(), 1);

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                id: "approval".to_string(),
            })
        );
        assert!(manager.queue.is_empty());
    }

    #[test]
    fn remove_resolved_prompt_keeps_lifecycle_events() {
        let mut manager = InterruptManager::new();
        manager.push_exec_begin(exec_begin("call"));

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                id: "call".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        assert!(matches!(
            manager.queue.front(),
            Some(QueuedInterrupt::ExecBegin(_))
        ));
    }
}
