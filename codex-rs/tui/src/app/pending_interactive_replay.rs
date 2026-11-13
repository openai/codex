//! Tracks unresolved interactive requests in the thread replay buffer.

use crate::app_command::AppCommand;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItemPayload;

#[derive(Debug, Default)]
pub(super) struct PendingInteractiveReplayState {
    // In arrival order so an answer addressed to a turn resolves its oldest question.
    requests: Vec<(AppServerRequestId, PendingInteractiveRequest)>,
}

#[derive(Debug, PartialEq, Eq)]
enum PendingInteractiveRequest {
    ExecApproval {
        turn_id: String,
        approval_id: String,
    },
    PatchApproval {
        turn_id: String,
        item_id: String,
    },
    Elicitation {
        server_name: String,
        request_id: AppServerRequestId,
    },
    RequestPermissions {
        turn_id: String,
        item_id: String,
    },
    RequestUserInput {
        turn_id: String,
        item_id: String,
    },
}

impl PendingInteractiveRequest {
    fn from_server_request(request: &ServerRequest) -> Option<Self> {
        Some(match request {
            ServerRequest::CommandExecutionRequestApproval { params, .. } => Self::ExecApproval {
                turn_id: params.turn_id.clone(),
                approval_id: params
                    .approval_id
                    .as_ref()
                    .unwrap_or(&params.item_id)
                    .clone(),
            },
            ServerRequest::FileChangeRequestApproval { params, .. } => Self::PatchApproval {
                turn_id: params.turn_id.clone(),
                item_id: params.item_id.clone(),
            },
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                Self::Elicitation {
                    server_name: params.server_name.clone(),
                    request_id: request_id.clone(),
                }
            }
            ServerRequest::PermissionsRequestApproval { params, .. } => Self::RequestPermissions {
                turn_id: params.turn_id.clone(),
                item_id: params.item_id.clone(),
            },
            ServerRequest::ToolRequestUserInput { params, .. } => Self::RequestUserInput {
                turn_id: params.turn_id.clone(),
                item_id: params.item_id.clone(),
            },
            _ => return None,
        })
    }

    fn turn_id(&self) -> Option<&str> {
        match self {
            Self::ExecApproval { turn_id, .. }
            | Self::PatchApproval { turn_id, .. }
            | Self::RequestPermissions { turn_id, .. }
            | Self::RequestUserInput { turn_id, .. } => Some(turn_id),
            Self::Elicitation { .. } => None,
        }
    }
}

impl PendingInteractiveReplayState {
    pub(super) fn op_can_change_state<T>(op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        matches!(
            op.into(),
            AppCommand::ExecApproval { .. }
                | AppCommand::PatchApproval { .. }
                | AppCommand::ResolveElicitation { .. }
                | AppCommand::ResolveUserVerification { .. }
                | AppCommand::RequestPermissionsResponse { .. }
                | AppCommand::UserInputAnswer { .. }
        )
    }

    pub(super) fn note_outbound_op<T>(&mut self, op: T)
    where
        T: Into<AppCommand>,
    {
        use PendingInteractiveRequest as Request;

        let op = op.into();
        if let AppCommand::UserInputAnswer { id, .. } = &op {
            if let Some(index) = self.requests.iter().position(|(_, request)| {
                matches!(request, Request::RequestUserInput { turn_id, .. } if turn_id == id)
            }) {
                let (_, answered) = self.requests.remove(index);
                self.requests.retain(|(_, request)| request != &answered);
            }
            return;
        }
        self.requests.retain(|(_, request)| match (&op, request) {
            (AppCommand::ExecApproval { id, .. }, Request::ExecApproval { approval_id, .. }) => {
                id != approval_id
            }
            (AppCommand::PatchApproval { id, .. }, Request::PatchApproval { item_id, .. })
            | (
                AppCommand::RequestPermissionsResponse { id, .. },
                Request::RequestPermissions { item_id, .. },
            ) => id != item_id,
            (
                AppCommand::ResolveElicitation {
                    server_name,
                    request_id,
                    ..
                }
                | AppCommand::ResolveUserVerification {
                    server_name,
                    request_id,
                    ..
                },
                Request::Elicitation {
                    server_name: pending_server,
                    request_id: pending_id,
                },
            ) => server_name != pending_server || request_id != pending_id,
            _ => true,
        });
    }

    pub(super) fn note_server_request(&mut self, request: &ServerRequest) {
        if let Some(pending) = PendingInteractiveRequest::from_server_request(request)
            && !self
                .requests
                .iter()
                .any(|(id, existing)| id == request.id() && existing == &pending)
        {
            self.requests.push((request.id().clone(), pending));
        }
    }

    pub(super) fn note_server_notification(&mut self, notification: &ServerNotification) {
        use PendingInteractiveRequest as Request;

        match notification {
            ServerNotification::ItemStarted(notification) => {
                self.requests
                    .retain(|(_, request)| match (&notification.item.payload, request) {
                        (
                            ThreadItemPayload::CommandExecution { id, .. },
                            Request::ExecApproval { approval_id, .. },
                        ) => id != approval_id,
                        (
                            ThreadItemPayload::FileChange { id, .. },
                            Request::PatchApproval { item_id, .. },
                        ) => id != item_id,
                        _ => true,
                    });
            }
            ServerNotification::TurnCompleted(notification) => self
                .requests
                .retain(|(_, request)| request.turn_id() != Some(notification.turn.id.as_str())),
            ServerNotification::ServerRequestResolved(notification) => {
                if let Some(index) = self
                    .requests
                    .iter()
                    .rposition(|(id, _)| id == &notification.request_id)
                {
                    let (_, resolved) = self.requests.remove(index);
                    self.requests.retain(|(_, request)| request != &resolved);
                }
            }
            ServerNotification::ThreadClosed(_) => self.requests.clear(),
            _ => {}
        }
    }

    pub(super) fn note_evicted_server_request(&mut self, request: &ServerRequest) {
        if let Some(evicted) = PendingInteractiveRequest::from_server_request(request) {
            self.requests.retain(|(_, request)| request != &evicted);
        }
    }

    pub(super) fn should_replay_snapshot_request(&self, request: &ServerRequest) -> bool {
        PendingInteractiveRequest::from_server_request(request)
            .is_none_or(|pending| self.requests.iter().any(|(_, request)| request == &pending))
    }

    pub(super) fn has_pending_thread_approvals(&self) -> bool {
        self.requests.iter().any(|(_, request)| {
            !matches!(request, PendingInteractiveRequest::RequestUserInput { .. })
        })
    }

    pub(super) fn has_pending_thread_user_input(&self) -> bool {
        self.requests.iter().any(|(_, request)| {
            matches!(request, PendingInteractiveRequest::RequestUserInput { .. })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::ThreadBufferedEvent;
    use super::super::ThreadEventStore;
    use crate::app_command::AppCommand as Op;
    use codex_app_server_protocol::CommandExecutionApprovalDecision;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::McpElicitationObjectType;
    use codex_app_server_protocol::McpElicitationSchema;
    use codex_app_server_protocol::McpServerElicitationAction;
    use codex_app_server_protocol::McpServerElicitationRequest;
    use codex_app_server_protocol::McpServerElicitationRequestParams;
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ServerRequest;
    use codex_app_server_protocol::ServerRequestResolvedNotification;
    use codex_app_server_protocol::ThreadClosedNotification;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_app_server_protocol::ToolRequestUserInputResponse;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnStatus;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;
    use std::collections::HashMap;

    fn request_user_input_request(call_id: &str, turn_id: &str) -> ServerRequest {
        ServerRequest::ToolRequestUserInput {
            request_id: AppServerRequestId::Integer(1),
            params: ToolRequestUserInputParams {
                thread_id: "thread-1".to_string(),
                turn_id: turn_id.to_string(),
                item_id: call_id.to_string(),
                questions: Vec::new(),
                is_blocking: true,
                auto_resolution_ms: None,
            },
        }
    }

    fn exec_approval_request(
        call_id: &str,
        approval_id: Option<&str>,
        turn_id: &str,
    ) -> ServerRequest {
        ServerRequest::CommandExecutionRequestApproval {
            request_id: AppServerRequestId::Integer(2),
            params: CommandExecutionRequestApprovalParams {
                kind: Default::default(),
                thread_id: "thread-1".to_string(),
                turn_id: turn_id.to_string(),
                item_id: call_id.to_string(),
                started_at_ms: 0,
                approval_id: approval_id.map(str::to_string),
                environment_id: None,
                reason: None,
                network_approval_context: None,
                command: Some("echo hi".to_string()),
                cwd: Some(test_path_buf("/tmp").abs().into()),
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        }
    }

    fn patch_approval_request(call_id: &str, turn_id: &str) -> ServerRequest {
        ServerRequest::FileChangeRequestApproval {
            request_id: AppServerRequestId::Integer(3),
            params: FileChangeRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: turn_id.to_string(),
                item_id: call_id.to_string(),
                started_at_ms: 0,
                reason: None,
                grant_root: None,
            },
        }
    }

    fn elicitation_request(server_name: &str, request_id: &str, turn_id: &str) -> ServerRequest {
        ServerRequest::McpServerElicitationRequest {
            request_id: AppServerRequestId::String(request_id.to_string()),
            params: McpServerElicitationRequestParams {
                environment_id: None,
                thread_id: "thread-1".to_string(),
                turn_id: Some(turn_id.to_string()),
                server_name: server_name.to_string(),
                request: McpServerElicitationRequest::Form {
                    meta: None,
                    message: "Please confirm".to_string(),
                    requested_schema: McpElicitationSchema {
                        schema_uri: None,
                        type_: McpElicitationObjectType::Object,
                        properties: BTreeMap::new(),
                        required: None,
                    },
                },
            },
        }
    }

    fn turn_completed(turn_id: &str) -> ServerNotification {
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: turn_id.to_string(),
                root_turn_id: None,
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: Some(1),
            },
        })
    }

    fn thread_closed() -> ServerNotification {
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: "thread-1".to_string(),
        })
    }

    fn request_resolved(request_id: AppServerRequestId) -> ServerNotification {
        ServerNotification::ServerRequestResolved(ServerRequestResolvedNotification {
            thread_id: "thread-1".to_string(),
            request_id,
        })
    }

    #[test]
    fn thread_event_snapshot_keeps_pending_request_user_input() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        let request = request_user_input_request("call-1", "turn-1");

        store.push_request(request);

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first(),
            Some(ThreadBufferedEvent::Request(request))
                if matches!(
                    request.as_ref(),
                    ServerRequest::ToolRequestUserInput { params, .. }
                        if params.item_id == "call-1"
                )
        ));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_request_user_input_after_user_answer() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(request_user_input_request("call-1", "turn-1"));

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: ToolRequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved request_user_input prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_request_user_input_after_server_resolution() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(request_user_input_request("call-1", "turn-1"));

        store.push_notification(request_resolved(AppServerRequestId::Integer(1)));

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.iter().all(|event| {
                !matches!(
                    event,
                    ThreadBufferedEvent::Request(request)
                        if matches!(
                            request.as_ref(),
                            ServerRequest::ToolRequestUserInput { .. }
                        )
                )
            }),
            "server-resolved request_user_input prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_exec_approval_after_outbound_approval_id() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            "call-1",
            Some("approval-1"),
            "turn-1",
        ));

        store.note_outbound_op(&Op::ExecApproval {
            id: "approval-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: CommandExecutionApprovalDecision::Accept,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved exec approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_exec_approval_after_server_resolution() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            "call-1",
            Some("approval-1"),
            "turn-1",
        ));

        store.push_notification(request_resolved(AppServerRequestId::Integer(2)));

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.iter().all(|event| {
                !matches!(
                    event,
                    ThreadBufferedEvent::Request(request)
                        if matches!(
                            request.as_ref(),
                            ServerRequest::CommandExecutionRequestApproval { .. }
                        )
                )
            }),
            "server-resolved exec approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_answered_request_user_input_for_multi_prompt_turn() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(request_user_input_request("call-1", "turn-1"));

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: ToolRequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        store.push_request(request_user_input_request("call-2", "turn-1"));

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first(),
            Some(ThreadBufferedEvent::Request(request))
                if matches!(
                    request.as_ref(),
                    ServerRequest::ToolRequestUserInput { params, .. }
                        if params.item_id == "call-2"
                )
        ));
    }

    #[test]
    fn thread_event_snapshot_keeps_newer_request_user_input_pending_when_same_turn_has_queue() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(request_user_input_request("call-1", "turn-1"));
        store.push_request(request_user_input_request("call-2", "turn-1"));

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: ToolRequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first(),
            Some(ThreadBufferedEvent::Request(request))
                if matches!(
                    request.as_ref(),
                    ServerRequest::ToolRequestUserInput { params, .. }
                        if params.item_id == "call-2"
                )
        ));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_patch_approval_after_outbound_approval() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(patch_approval_request("call-1", "turn-1"));

        store.note_outbound_op(&Op::PatchApproval {
            id: "call-1".to_string(),
            decision: codex_app_server_protocol::FileChangeApprovalDecision::Accept,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved patch approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_pending_approvals_when_turn_completes() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            "exec-call-1",
            Some("approval-1"),
            "turn-1",
        ));
        store.push_request(patch_approval_request("patch-call-1", "turn-1"));
        store.push_notification(turn_completed("turn-1"));

        let snapshot = store.snapshot();
        assert!(snapshot.events.iter().all(|event| {
            !matches!(
                event,
                ThreadBufferedEvent::Request(request)
                    if matches!(
                        request.as_ref(),
                        ServerRequest::CommandExecutionRequestApproval { .. }
                            | ServerRequest::FileChangeRequestApproval { .. }
                    )
            )
        }));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_elicitation_after_outbound_resolution() {
        let request_id = AppServerRequestId::String("request-1".to_string());
        for response in [
            Op::ResolveElicitation {
                server_name: "server-1".to_string(),
                request_id: request_id.clone(),
                decision: McpServerElicitationAction::Accept,
                content: None,
                meta: None,
            },
            Op::ResolveUserVerification {
                server_name: "server-1".to_string(),
                request_id,
                response: crate::app_command::UserVerificationResponse::Cancel,
            },
        ] {
            let mut store = ThreadEventStore::new(/*capacity*/ 8);
            let mut request = elicitation_request("server-1", "request-1", "turn-1");
            if matches!(&response, Op::ResolveUserVerification { .. })
                && let ServerRequest::McpServerElicitationRequest { params, .. } = &mut request
            {
                params.request = McpServerElicitationRequest::UserVerification {
                    meta: None,
                    title: "Verify".to_string(),
                    description: "Approve deployment".to_string(),
                    challenge: "AQID".to_string(),
                };
            }
            store.push_request(request);
            assert_eq!(store.snapshot().events.len(), 1);
            store.note_outbound_op(&response);
            assert!(
                store.snapshot().events.is_empty(),
                "resolved elicitation prompt should not replay on thread switch"
            );
        }
    }

    #[test]
    fn thread_event_store_reports_pending_thread_approvals() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        assert_eq!(store.has_pending_thread_approvals(), false);

        store.push_request(exec_approval_request(
            "call-1", /*approval_id*/ None, "turn-1",
        ));

        assert_eq!(store.has_pending_thread_approvals(), true);

        store.note_outbound_op(&Op::ExecApproval {
            id: "call-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: CommandExecutionApprovalDecision::Accept,
        });

        assert_eq!(store.has_pending_thread_approvals(), false);
    }

    #[test]
    fn request_user_input_does_not_count_as_pending_thread_approval() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(request_user_input_request("call-1", "turn-1"));

        assert_eq!(store.has_pending_thread_approvals(), false);
    }

    #[test]
    fn thread_event_snapshot_drops_pending_requests_when_thread_closes() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            "call-1", /*approval_id*/ None, "turn-1",
        ));
        store.push_notification(thread_closed());

        assert!(store.snapshot().events.iter().all(|event| {
            !matches!(
                event,
                ThreadBufferedEvent::Request(request)
                    if matches!(
                        request.as_ref(),
                        ServerRequest::CommandExecutionRequestApproval { .. }
                    )
            )
        }));
    }
}
