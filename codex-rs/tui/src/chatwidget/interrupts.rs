//! Queue prompt overlays and deferred tool activity while another interrupt is visible.

use std::collections::VecDeque;

use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::approval_events::ApplyPatchApprovalRequestEvent;
use crate::approval_events::ExecApprovalRequestEvent;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_protocol::ThreadId;
use codex_protocol::request_permissions::RequestPermissionsEvent;

use super::ChatWidget;

#[derive(Debug)]
pub(crate) enum QueuedInterrupt {
    ExecApproval {
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: ExecApprovalRequestEvent,
    },
    ApplyPatchApproval {
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: ApplyPatchApprovalRequestEvent,
    },
    Elicitation {
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    },
    RequestPermissions {
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: RequestPermissionsEvent,
    },
    RequestUserInput {
        app_server_request_id: Option<AppServerRequestId>,
        params: ToolRequestUserInputParams,
    },
    ItemStarted(ThreadItem),
    ItemCompleted(ThreadItem),
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

    pub(crate) fn push_exec_approval(
        &mut self,
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: ExecApprovalRequestEvent,
    ) {
        self.queue.push_back(QueuedInterrupt::ExecApproval {
            thread_id,
            app_server_request_id,
            event,
        });
    }

    pub(crate) fn push_apply_patch_approval(
        &mut self,
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: ApplyPatchApprovalRequestEvent,
    ) {
        self.queue.push_back(QueuedInterrupt::ApplyPatchApproval {
            thread_id,
            app_server_request_id,
            event,
        });
    }

    pub(crate) fn push_elicitation(
        &mut self,
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    ) {
        self.queue
            .push_back(QueuedInterrupt::Elicitation { request_id, params });
    }

    pub(crate) fn push_request_permissions(
        &mut self,
        thread_id: Option<ThreadId>,
        app_server_request_id: Option<AppServerRequestId>,
        event: RequestPermissionsEvent,
    ) {
        self.queue.push_back(QueuedInterrupt::RequestPermissions {
            thread_id,
            app_server_request_id,
            event,
        });
    }

    pub(crate) fn push_user_input(
        &mut self,
        app_server_request_id: Option<AppServerRequestId>,
        params: ToolRequestUserInputParams,
    ) {
        self.queue.push_back(QueuedInterrupt::RequestUserInput {
            app_server_request_id,
            params,
        });
    }

    pub(crate) fn push_item_started(&mut self, item: ThreadItem) {
        self.queue.push_back(QueuedInterrupt::ItemStarted(item));
    }

    pub(crate) fn push_item_completed(&mut self, item: ThreadItem) {
        self.queue.push_back(QueuedInterrupt::ItemCompleted(item));
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
                QueuedInterrupt::ExecApproval {
                    thread_id,
                    app_server_request_id,
                    event,
                } => chat.handle_exec_approval_now(thread_id, app_server_request_id, event),
                QueuedInterrupt::ApplyPatchApproval {
                    thread_id,
                    app_server_request_id,
                    event,
                } => chat.handle_apply_patch_approval_now(thread_id, app_server_request_id, event),
                QueuedInterrupt::Elicitation { request_id, params } => {
                    chat.handle_elicitation_request_now(request_id, params);
                }
                QueuedInterrupt::RequestPermissions {
                    thread_id,
                    app_server_request_id,
                    event,
                } => chat.handle_request_permissions_now(thread_id, app_server_request_id, event),
                QueuedInterrupt::RequestUserInput {
                    app_server_request_id,
                    params,
                } => chat.handle_request_user_input_now(app_server_request_id, params),
                QueuedInterrupt::ItemStarted(item) => chat.handle_queued_item_started_now(item),
                QueuedInterrupt::ItemCompleted(item) => {
                    chat.handle_queued_item_completed_now(item);
                }
            }
        }
    }
}

impl QueuedInterrupt {
    fn matches_resolved_prompt(&self, request: &ResolvedAppServerRequest) -> bool {
        match self {
            QueuedInterrupt::ExecApproval {
                thread_id,
                app_server_request_id,
                event,
            } => {
                matches!(request, ResolvedAppServerRequest::ExecApproval { thread_id: resolved_thread_id, request_id, id }
                    if *thread_id == Some(*resolved_thread_id)
                        && app_server_request_id.as_ref() == Some(request_id)
                        && event.effective_approval_id() == id.as_str())
            }
            QueuedInterrupt::ApplyPatchApproval {
                thread_id,
                app_server_request_id,
                event,
            } => {
                matches!(request, ResolvedAppServerRequest::FileChangeApproval { thread_id: resolved_thread_id, request_id, id }
                    if *thread_id == Some(*resolved_thread_id)
                        && app_server_request_id.as_ref() == Some(request_id)
                        && event.call_id == id.as_str())
            }
            QueuedInterrupt::Elicitation { request_id, params } => {
                matches!(request, ResolvedAppServerRequest::McpElicitation {
                    thread_id,
                    server_name,
                    request_id: resolved_request_id,
                } if ThreadId::from_string(&params.thread_id).ok() == Some(*thread_id)
                    && params.server_name == server_name.as_str()
                    && request_id == resolved_request_id)
            }
            QueuedInterrupt::RequestPermissions {
                thread_id,
                app_server_request_id,
                event,
            } => {
                matches!(request, ResolvedAppServerRequest::PermissionsApproval { thread_id: resolved_thread_id, request_id, id }
                    if *thread_id == Some(*resolved_thread_id)
                        && app_server_request_id.as_ref() == Some(request_id)
                        && event.call_id == id.as_str())
            }
            QueuedInterrupt::RequestUserInput {
                app_server_request_id,
                params,
            } => {
                matches!(request, ResolvedAppServerRequest::UserInput { thread_id, request_id, turn_id, call_id }
                    if ThreadId::from_string(&params.thread_id).ok() == Some(*thread_id)
                        && app_server_request_id.as_ref() == Some(request_id)
                        && params.turn_id == turn_id.as_str()
                        && params.item_id == call_id.as_str())
            }
            QueuedInterrupt::ItemStarted(_) | QueuedInterrupt::ItemCompleted(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::approval_events::ExecApprovalRequestEvent;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_app_server_protocol::ThreadItem;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::*;

    fn thread_id() -> ThreadId {
        ThreadId::try_from("00000000-0000-0000-0000-000000000001").expect("valid thread id")
    }

    fn user_input(call_id: &str, turn_id: &str) -> ToolRequestUserInputParams {
        ToolRequestUserInputParams {
            thread_id: thread_id().to_string(),
            item_id: call_id.to_string(),
            turn_id: turn_id.to_string(),
            questions: Vec::new(),
            auto_resolution_ms: None,
        }
    }

    fn exec_approval(call_id: &str, approval_id: Option<&str>) -> ExecApprovalRequestEvent {
        ExecApprovalRequestEvent {
            call_id: call_id.to_string(),
            approval_id: approval_id.map(str::to_string),
            turn_id: "turn".to_string(),
            environment_id: None,
            command: vec!["true".to_string()],
            cwd: AbsolutePathBuf::current_dir().expect("current dir"),
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
        }
    }

    fn command_execution(call_id: &str) -> ThreadItem {
        ThreadItem::CommandExecution {
            id: call_id.to_string(),
            command: "true".to_string(),
            cwd: AbsolutePathBuf::current_dir().expect("current dir").into(),
            process_id: None,
            source: CommandExecutionSource::Agent,
            status: CommandExecutionStatus::InProgress,
            command_actions: Vec::new(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        }
    }

    #[test]
    fn remove_resolved_prompt_removes_matching_user_input_only() {
        let mut manager = InterruptManager::new();
        let request_a = AppServerRequestId::String("request-a".to_string());
        let request_b = AppServerRequestId::String("request-b".to_string());
        manager.push_user_input(Some(request_a), user_input("call", "turn-a"));
        manager.push_user_input(Some(request_b.clone()), user_input("call", "turn-b"));

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::UserInput {
                thread_id: thread_id(),
                request_id: request_b,
                turn_id: "turn-b".to_string(),
                call_id: "call".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        let Some(QueuedInterrupt::RequestUserInput {
            params: remaining, ..
        }) = manager.queue.front()
        else {
            panic!("expected remaining queued user input");
        };
        assert_eq!(remaining.turn_id, "turn-a");
    }

    #[test]
    fn remove_resolved_prompt_matches_exec_approval_id() {
        let mut manager = InterruptManager::new();
        let request_id = AppServerRequestId::String("request-b".to_string());
        manager.push_exec_approval(
            Some(thread_id()),
            Some(request_id.clone()),
            exec_approval("call", Some("approval")),
        );

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: ThreadId::try_from("00000000-0000-0000-0000-000000000002")
                    .expect("valid thread id"),
                request_id: request_id.clone(),
                id: "approval".to_string(),
            })
        );
        assert_eq!(manager.queue.len(), 1);
        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: thread_id(),
                request_id: request_id.clone(),
                id: "call".to_string(),
            })
        );
        assert_eq!(manager.queue.len(), 1);
        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: thread_id(),
                request_id: AppServerRequestId::String("request-a".to_string()),
                id: "approval".to_string(),
            }),
            "a stale request with the same semantic key must not remove the current prompt"
        );
        assert_eq!(manager.queue.len(), 1);

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: thread_id(),
                request_id,
                id: "approval".to_string(),
            })
        );
        assert!(manager.queue.is_empty());
    }

    #[test]
    fn remove_resolved_prompt_keeps_lifecycle_events() {
        let mut manager = InterruptManager::new();
        manager.push_item_started(command_execution("call"));

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: thread_id(),
                request_id: AppServerRequestId::String("request".to_string()),
                id: "call".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        assert!(matches!(
            manager.queue.front(),
            Some(QueuedInterrupt::ItemStarted(_))
        ));
    }
}
