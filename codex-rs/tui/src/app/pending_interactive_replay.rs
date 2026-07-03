use crate::app_command::AppCommand;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ElicitationRequestKey {
    server_name: String,
    request_id: AppServerRequestId,
}

impl ElicitationRequestKey {
    fn new(server_name: String, request_id: AppServerRequestId) -> Self {
        Self {
            server_name,
            request_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TurnItemKey {
    turn_id: String,
    item_id: String,
}

impl TurnItemKey {
    fn new(turn_id: impl Into<String>, item_id: impl Into<String>) -> Self {
        Self {
            turn_id: turn_id.into(),
            item_id: item_id.into(),
        }
    }
}

#[derive(Debug, Default)]
// Tracks which interactive prompts are still unresolved in the thread-event buffer.
//
// Thread snapshots are replayed when switching threads/agents. Most events should replay
// verbatim, but interactive prompts (approvals, request_user_input, MCP elicitations) must
// only replay if they are still pending. This state is updated from:
// - inbound events (`note_event`)
// - outbound ops that resolve a prompt (`note_outbound_op`)
// - buffer eviction (`note_evicted_event`)
//
// The request-id map is authoritative for snapshot replay and pending-status state.
// Semantic-key sets and turn indexes remain compatibility bookkeeping for legacy
// outbound ops that do not carry the outer app-server request ID. `request_user_input`
// removal on that legacy path is FIFO for prompts sharing a `turn_id`.
pub(super) struct PendingInteractiveReplayState {
    exec_approval_call_ids: HashSet<String>,
    exec_approval_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    patch_approval_call_ids: HashSet<String>,
    patch_approval_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    elicitation_requests: HashSet<ElicitationRequestKey>,
    request_permissions_call_ids: HashSet<String>,
    request_permissions_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    request_user_input_call_ids: HashSet<TurnItemKey>,
    request_user_input_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    pending_requests_by_request_id: HashMap<AppServerRequestId, PendingInteractiveRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingInteractiveRequest {
    ExecApproval {
        turn_id: String,
        approval_id: String,
    },
    PatchApproval {
        turn_id: String,
        item_id: String,
    },
    Elicitation(ElicitationRequestKey),
    RequestPermissions {
        turn_id: String,
        item_id: String,
    },
    RequestUserInput {
        turn_id: String,
        item_id: String,
    },
}

impl PendingInteractiveReplayState {
    pub(super) fn op_can_change_state<T>(op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        matches!(
            &op,
            AppCommand::ExecApproval { .. }
                | AppCommand::PatchApproval { .. }
                | AppCommand::ResolveElicitation { .. }
                | AppCommand::RequestPermissionsResponse { .. }
                | AppCommand::UserInputAnswer { .. }
                | AppCommand::Shutdown
        )
    }

    pub(super) fn note_outbound_op<T>(&mut self, op: T)
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        match &op {
            AppCommand::ExecApproval { id, turn_id, .. } => {
                self.exec_approval_call_ids.remove(id);
                if let Some(turn_id) = turn_id {
                    Self::remove_call_id_from_turn_map_entry(
                        &mut self.exec_approval_call_ids_by_turn_id,
                        turn_id,
                        id,
                    );
                }
                self.pending_requests_by_request_id
                    .retain(|_, pending| !matches!(pending, PendingInteractiveRequest::ExecApproval { approval_id, .. } if approval_id == id));
            }
            AppCommand::PatchApproval { id, .. } => {
                self.patch_approval_call_ids.remove(id);
                Self::remove_call_id_from_turn_map(
                    &mut self.patch_approval_call_ids_by_turn_id,
                    id,
                );
                self.pending_requests_by_request_id
                    .retain(|_, pending| !matches!(pending, PendingInteractiveRequest::PatchApproval { item_id, .. } if item_id == id));
            }
            AppCommand::ResolveElicitation {
                server_name,
                request_id,
                ..
            } => {
                self.elicitation_requests
                    .remove(&ElicitationRequestKey::new(
                        server_name.to_string(),
                        request_id.clone(),
                    ));
                self.pending_requests_by_request_id.retain(
                    |_, pending| {
                        !matches!(pending, PendingInteractiveRequest::Elicitation(key) if key.server_name == *server_name && key.request_id == *request_id)
                    },
                );
            }
            AppCommand::RequestPermissionsResponse { id, .. } => {
                self.request_permissions_call_ids.remove(id);
                Self::remove_call_id_from_turn_map(
                    &mut self.request_permissions_call_ids_by_turn_id,
                    id,
                );
                self.pending_requests_by_request_id.retain(
                    |_, pending| {
                        !matches!(pending, PendingInteractiveRequest::RequestPermissions { item_id, .. } if item_id == id)
                    },
                );
            }
            // `Op::UserInputAnswer` identifies the turn, not the prompt call_id. The UI
            // answers queued prompts for the same turn in FIFO order, so remove the oldest
            // queued call_id for that turn.
            AppCommand::UserInputAnswer { id, .. } => {
                let mut remove_turn_entry = false;
                if let Some(call_ids) = self.request_user_input_call_ids_by_turn_id.get_mut(id) {
                    if !call_ids.is_empty() {
                        let call_id = call_ids.remove(0);
                        self.request_user_input_call_ids
                            .remove(&TurnItemKey::new(id, &call_id));
                        self.pending_requests_by_request_id.retain(
                            |_, pending| {
                                !matches!(pending, PendingInteractiveRequest::RequestUserInput { turn_id, item_id } if turn_id == id && *item_id == call_id)
                            },
                        );
                    }
                    if call_ids.is_empty() {
                        remove_turn_entry = true;
                    }
                }
                if remove_turn_entry {
                    self.request_user_input_call_ids_by_turn_id.remove(id);
                }
            }
            AppCommand::Shutdown => self.clear(),
            _ => {}
        }
    }

    pub(super) fn note_server_request(&mut self, request: &ServerRequest) {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let approval_id = params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone());
                self.exec_approval_call_ids.insert(approval_id.clone());
                self.exec_approval_call_ids_by_turn_id
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push(approval_id);
                self.pending_requests_by_request_id.insert(
                    request_id.clone(),
                    PendingInteractiveRequest::ExecApproval {
                        turn_id: params.turn_id.clone(),
                        approval_id: params
                            .approval_id
                            .clone()
                            .unwrap_or_else(|| params.item_id.clone()),
                    },
                );
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                self.patch_approval_call_ids.insert(params.item_id.clone());
                self.patch_approval_call_ids_by_turn_id
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push(params.item_id.clone());
                self.pending_requests_by_request_id.insert(
                    request_id.clone(),
                    PendingInteractiveRequest::PatchApproval {
                        turn_id: params.turn_id.clone(),
                        item_id: params.item_id.clone(),
                    },
                );
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                let key =
                    ElicitationRequestKey::new(params.server_name.clone(), request_id.clone());
                self.elicitation_requests.insert(key.clone());
                self.pending_requests_by_request_id.insert(
                    request_id.clone(),
                    PendingInteractiveRequest::Elicitation(key),
                );
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                self.request_user_input_call_ids
                    .insert(TurnItemKey::new(&params.turn_id, &params.item_id));
                self.request_user_input_call_ids_by_turn_id
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push(params.item_id.clone());
                self.pending_requests_by_request_id.insert(
                    request_id.clone(),
                    PendingInteractiveRequest::RequestUserInput {
                        turn_id: params.turn_id.clone(),
                        item_id: params.item_id.clone(),
                    },
                );
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                self.request_permissions_call_ids
                    .insert(params.item_id.clone());
                self.request_permissions_call_ids_by_turn_id
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push(params.item_id.clone());
                self.pending_requests_by_request_id.insert(
                    request_id.clone(),
                    PendingInteractiveRequest::RequestPermissions {
                        turn_id: params.turn_id.clone(),
                        item_id: params.item_id.clone(),
                    },
                );
            }
            _ => {}
        }
    }

    pub(super) fn note_server_notification(&mut self, notification: &ServerNotification) {
        match notification {
            ServerNotification::ItemStarted(notification) => match &notification.item {
                ThreadItem::CommandExecution { id, .. } => {
                    let request_ids = self
                        .pending_requests_by_request_id
                        .iter()
                        .filter(|&(_request_id, pending)| {
                            matches!(
                                pending,
                                PendingInteractiveRequest::ExecApproval {
                                    turn_id,
                                    approval_id,
                                } if turn_id == &notification.turn_id && approval_id == id
                            )
                        })
                        .map(|(request_id, _pending)| request_id.clone())
                        .collect::<Vec<_>>();
                    for request_id in request_ids {
                        self.remove_request(&request_id);
                    }
                    self.exec_approval_call_ids.remove(id);
                    Self::remove_call_id_from_turn_map(
                        &mut self.exec_approval_call_ids_by_turn_id,
                        id,
                    );
                }
                ThreadItem::FileChange { id, .. } => {
                    let request_ids = self
                        .pending_requests_by_request_id
                        .iter()
                        .filter(|&(_request_id, pending)| {
                            matches!(
                                pending,
                                PendingInteractiveRequest::PatchApproval { turn_id, item_id }
                                    if turn_id == &notification.turn_id && item_id == id
                            )
                        })
                        .map(|(request_id, _pending)| request_id.clone())
                        .collect::<Vec<_>>();
                    for request_id in request_ids {
                        self.remove_request(&request_id);
                    }
                    self.patch_approval_call_ids.remove(id);
                    Self::remove_call_id_from_turn_map(
                        &mut self.patch_approval_call_ids_by_turn_id,
                        id,
                    );
                }
                _ => {}
            },
            ServerNotification::TurnCompleted(notification) => {
                self.clear_exec_approval_turn(&notification.turn.id);
                self.clear_patch_approval_turn(&notification.turn.id);
                self.clear_request_permissions_turn(&notification.turn.id);
                self.clear_request_user_input_turn(&notification.turn.id);
            }
            ServerNotification::ServerRequestResolved(notification) => {
                self.remove_request(&notification.request_id);
            }
            ServerNotification::ThreadClosed(_) => self.clear(),
            _ => {}
        }
    }

    pub(super) fn note_resolved_app_server_request(&mut self, request_id: &AppServerRequestId) {
        self.remove_request(request_id);
    }

    pub(super) fn note_evicted_server_request(&mut self, request: &ServerRequest) {
        let request_id = match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, .. }
            | ServerRequest::FileChangeRequestApproval { request_id, .. }
            | ServerRequest::McpServerElicitationRequest { request_id, .. }
            | ServerRequest::ToolRequestUserInput { request_id, .. }
            | ServerRequest::PermissionsRequestApproval { request_id, .. } => request_id,
            _ => return,
        };
        if self
            .pending_requests_by_request_id
            .get(request_id)
            .is_some_and(|pending| Self::request_matches_server_request(pending, request))
        {
            self.remove_request(request_id);
        }
    }

    pub(super) fn should_replay_snapshot_request(&self, request: &ServerRequest) -> bool {
        let request_id = match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, .. }
            | ServerRequest::FileChangeRequestApproval { request_id, .. }
            | ServerRequest::McpServerElicitationRequest { request_id, .. }
            | ServerRequest::ToolRequestUserInput { request_id, .. }
            | ServerRequest::PermissionsRequestApproval { request_id, .. } => request_id,
            _ => return true,
        };
        self.pending_requests_by_request_id
            .get(request_id)
            .is_some_and(|pending| Self::request_matches_server_request(pending, request))
    }

    pub(super) fn has_pending_thread_approvals(&self) -> bool {
        self.pending_requests_by_request_id.values().any(|pending| {
            matches!(
                pending,
                PendingInteractiveRequest::ExecApproval { .. }
                    | PendingInteractiveRequest::PatchApproval { .. }
                    | PendingInteractiveRequest::Elicitation(_)
                    | PendingInteractiveRequest::RequestPermissions { .. }
            )
        })
    }

    pub(super) fn has_pending_thread_user_input(&self) -> bool {
        self.pending_requests_by_request_id
            .values()
            .any(|pending| matches!(pending, PendingInteractiveRequest::RequestUserInput { .. }))
    }

    fn clear_request_user_input_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.request_user_input_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.request_user_input_call_ids
                    .remove(&TurnItemKey::new(turn_id, call_id));
            }
        }
        self.pending_requests_by_request_id.retain(
            |_, pending| {
                !matches!(pending, PendingInteractiveRequest::RequestUserInput { turn_id: pending_turn_id, .. } if pending_turn_id == turn_id)
            },
        );
    }

    fn clear_request_permissions_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.request_permissions_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.request_permissions_call_ids.remove(&call_id);
            }
        }
        self.pending_requests_by_request_id.retain(
            |_, pending| {
                !matches!(pending, PendingInteractiveRequest::RequestPermissions { turn_id: pending_turn_id, .. } if pending_turn_id == turn_id)
            },
        );
    }

    fn clear_exec_approval_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.exec_approval_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.exec_approval_call_ids.remove(&call_id);
            }
        }
        self.pending_requests_by_request_id.retain(
            |_, pending| {
                !matches!(pending, PendingInteractiveRequest::ExecApproval { turn_id: pending_turn_id, .. } if pending_turn_id == turn_id)
            },
        );
    }

    fn clear_patch_approval_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.patch_approval_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.patch_approval_call_ids.remove(&call_id);
            }
        }
        self.pending_requests_by_request_id.retain(
            |_, pending| {
                !matches!(pending, PendingInteractiveRequest::PatchApproval { turn_id: pending_turn_id, .. } if pending_turn_id == turn_id)
            },
        );
    }

    fn remove_call_id_from_turn_map(
        call_ids_by_turn_id: &mut HashMap<String, Vec<String>>,
        call_id: &str,
    ) {
        call_ids_by_turn_id.retain(|_, call_ids| {
            call_ids.retain(|queued_call_id| queued_call_id != call_id);
            !call_ids.is_empty()
        });
    }

    fn remove_call_id_from_turn_map_entry(
        call_ids_by_turn_id: &mut HashMap<String, Vec<String>>,
        turn_id: &str,
        call_id: &str,
    ) {
        let mut remove_turn_entry = false;
        if let Some(call_ids) = call_ids_by_turn_id.get_mut(turn_id) {
            call_ids.retain(|queued_call_id| queued_call_id != call_id);
            if call_ids.is_empty() {
                remove_turn_entry = true;
            }
        }
        if remove_turn_entry {
            call_ids_by_turn_id.remove(turn_id);
        }
    }

    fn clear(&mut self) {
        self.exec_approval_call_ids.clear();
        self.exec_approval_call_ids_by_turn_id.clear();
        self.patch_approval_call_ids.clear();
        self.patch_approval_call_ids_by_turn_id.clear();
        self.elicitation_requests.clear();
        self.request_permissions_call_ids.clear();
        self.request_permissions_call_ids_by_turn_id.clear();
        self.request_user_input_call_ids.clear();
        self.request_user_input_call_ids_by_turn_id.clear();
        self.pending_requests_by_request_id.clear();
    }

    fn remove_request(&mut self, request_id: &AppServerRequestId) {
        let Some(pending) = self.pending_requests_by_request_id.remove(request_id) else {
            return;
        };
        match pending {
            PendingInteractiveRequest::ExecApproval {
                turn_id,
                approval_id,
            } => {
                self.exec_approval_call_ids.remove(&approval_id);
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.exec_approval_call_ids_by_turn_id,
                    &turn_id,
                    &approval_id,
                );
            }
            PendingInteractiveRequest::PatchApproval { turn_id, item_id } => {
                self.patch_approval_call_ids.remove(&item_id);
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.patch_approval_call_ids_by_turn_id,
                    &turn_id,
                    &item_id,
                );
            }
            PendingInteractiveRequest::Elicitation(key) => {
                self.elicitation_requests.remove(&key);
            }
            PendingInteractiveRequest::RequestPermissions { turn_id, item_id } => {
                self.request_permissions_call_ids.remove(&item_id);
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.request_permissions_call_ids_by_turn_id,
                    &turn_id,
                    &item_id,
                );
            }
            PendingInteractiveRequest::RequestUserInput { turn_id, item_id } => {
                self.request_user_input_call_ids
                    .remove(&TurnItemKey::new(&turn_id, &item_id));
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.request_user_input_call_ids_by_turn_id,
                    &turn_id,
                    &item_id,
                );
            }
        }
    }

    fn request_matches_server_request(
        pending: &PendingInteractiveRequest,
        request: &ServerRequest,
    ) -> bool {
        match (pending, request) {
            (
                PendingInteractiveRequest::ExecApproval {
                    turn_id,
                    approval_id,
                },
                ServerRequest::CommandExecutionRequestApproval { params, .. },
            ) => {
                turn_id == &params.turn_id
                    && approval_id == params.approval_id.as_ref().unwrap_or(&params.item_id)
            }
            (
                PendingInteractiveRequest::PatchApproval { turn_id, item_id },
                ServerRequest::FileChangeRequestApproval { params, .. },
            ) => turn_id == &params.turn_id && item_id == &params.item_id,
            (
                PendingInteractiveRequest::Elicitation(key),
                ServerRequest::McpServerElicitationRequest { request_id, params },
            ) => key.server_name == params.server_name && key.request_id == *request_id,
            (
                PendingInteractiveRequest::RequestPermissions { turn_id, item_id },
                ServerRequest::PermissionsRequestApproval { params, .. },
            ) => turn_id == &params.turn_id && item_id == &params.item_id,
            (
                PendingInteractiveRequest::RequestUserInput { turn_id, item_id },
                ServerRequest::ToolRequestUserInput { params, .. },
            ) => turn_id == &params.turn_id && item_id == &params.item_id,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ThreadBufferedEvent;
    use super::super::ThreadEventStore;
    use crate::app_command::AppCommand as Op;
    use codex_app_server_protocol::CommandExecutionApprovalDecision;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::FileUpdateChange;
    use codex_app_server_protocol::ItemStartedNotification;
    use codex_app_server_protocol::McpElicitationObjectType;
    use codex_app_server_protocol::McpElicitationSchema;
    use codex_app_server_protocol::McpServerElicitationAction;
    use codex_app_server_protocol::McpServerElicitationRequest;
    use codex_app_server_protocol::McpServerElicitationRequestParams;
    use codex_app_server_protocol::PatchApplyStatus;
    use codex_app_server_protocol::PatchChangeKind;
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ServerRequest;
    use codex_app_server_protocol::ServerRequestResolvedNotification;
    use codex_app_server_protocol::ThreadClosedNotification;
    use codex_app_server_protocol::ThreadItem;
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
                auto_resolution_ms: None,
            },
        }
    }

    fn exec_approval_request(
        call_id: &str,
        approval_id: Option<&str>,
        turn_id: &str,
    ) -> ServerRequest {
        exec_approval_request_with_request_id(
            AppServerRequestId::Integer(2),
            call_id,
            approval_id,
            turn_id,
        )
    }

    fn exec_approval_request_with_request_id(
        request_id: AppServerRequestId,
        call_id: &str,
        approval_id: Option<&str>,
        turn_id: &str,
    ) -> ServerRequest {
        ServerRequest::CommandExecutionRequestApproval {
            request_id,
            params: CommandExecutionRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: turn_id.to_string(),
                item_id: call_id.to_string(),
                started_at_ms: 0,
                approval_id: approval_id.map(str::to_string),
                approval_purpose: None,
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

    fn item_started(turn_id: &str, item: ThreadItem) -> ServerNotification {
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: turn_id.to_string(),
            started_at_ms: 0,
            item,
        })
    }

    fn command_execution_item(item_id: &str) -> ThreadItem {
        ThreadItem::CommandExecution {
            id: item_id.to_string(),
            command: "true".to_string(),
            cwd: test_path_buf("/tmp").abs().into(),
            process_id: None,
            source: CommandExecutionSource::Agent,
            status: CommandExecutionStatus::InProgress,
            command_actions: Vec::new(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        }
    }

    fn file_change_item(item_id: &str) -> ThreadItem {
        ThreadItem::FileChange {
            id: item_id.to_string(),
            changes: vec![FileUpdateChange {
                path: "README.md".to_string(),
                kind: PatchChangeKind::Add,
                diff: "hello\n".to_string(),
            }],
            status: PatchApplyStatus::InProgress,
        }
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
            Some(ThreadBufferedEvent::Request(ServerRequest::ToolRequestUserInput { params, .. }))
                if params.item_id == "call-1"
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
                    ThreadBufferedEvent::Request(ServerRequest::ToolRequestUserInput { .. })
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
    fn thread_event_snapshot_keeps_retry_callback_after_initial_item_approval() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            "call-1", /*approval_id*/ None, "turn-1",
        ));
        store.note_outbound_op(&Op::ExecApproval {
            id: "call-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: CommandExecutionApprovalDecision::Accept,
        });
        store.push_request(exec_approval_request(
            "call-1",
            Some("retry-approval-1"),
            "turn-1",
        ));

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first(),
            Some(ThreadBufferedEvent::Request(
                ServerRequest::CommandExecutionRequestApproval { params, .. }
            )) if params.item_id == "call-1"
                && params.approval_id.as_deref() == Some("retry-approval-1")
        ));
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
                    ThreadBufferedEvent::Request(
                        ServerRequest::CommandExecutionRequestApproval { .. }
                    )
                )
            }),
            "server-resolved exec approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_replays_only_exact_same_key_replacement() {
        let original_request_id = AppServerRequestId::Integer(2);
        let replacement_request_id = AppServerRequestId::String("2".to_string());

        for capacity in [2, 8] {
            let mut store = ThreadEventStore::new(capacity);
            store.push_request(exec_approval_request_with_request_id(
                original_request_id.clone(),
                "call-a",
                Some("shared-approval"),
                "turn-1",
            ));
            store.push_notification(request_resolved(original_request_id.clone()));
            store.push_request(exec_approval_request_with_request_id(
                replacement_request_id.clone(),
                "call-b",
                Some("shared-approval"),
                "turn-2",
            ));
            store.push_notification(turn_completed("turn-1"));
            assert!(
                store.has_pending_thread_approvals(),
                "stale A eviction and turn completion at capacity {capacity} must preserve B's pending indicator"
            );

            let replayed_request_ids = store
                .snapshot()
                .events
                .into_iter()
                .filter_map(|event| match event {
                    ThreadBufferedEvent::Request(
                        ServerRequest::CommandExecutionRequestApproval { request_id, .. },
                    ) => Some(request_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                replayed_request_ids,
                vec![replacement_request_id.clone()],
                "capacity {capacity} must neither replay stale A nor evict replacement B state"
            );
        }
    }

    #[test]
    fn delayed_item_started_for_old_turn_preserves_same_key_replacement() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        let original_request_id = AppServerRequestId::Integer(2);
        let replacement_request_id = AppServerRequestId::String("2".to_string());
        store.push_request(exec_approval_request_with_request_id(
            original_request_id.clone(),
            "shared-call",
            /*approval_id*/ None,
            "turn-1",
        ));
        store.push_notification(request_resolved(original_request_id));
        store.push_request(exec_approval_request_with_request_id(
            replacement_request_id.clone(),
            "shared-call",
            /*approval_id*/ None,
            "turn-2",
        ));

        store.push_notification(item_started(
            "turn-1",
            command_execution_item("shared-call"),
        ));

        assert!(store.has_pending_thread_approvals());
        assert_eq!(
            store
                .pending_replay_requests()
                .into_iter()
                .filter_map(|request| match request {
                    ServerRequest::CommandExecutionRequestApproval { request_id, .. } => {
                        Some(request_id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![replacement_request_id]
        );
    }

    #[test]
    fn item_started_suppresses_only_matching_implicit_exec_and_patch_requests() {
        let mut implicit_exec_store = ThreadEventStore::new(/*capacity*/ 8);
        implicit_exec_store.push_request(exec_approval_request_with_request_id(
            AppServerRequestId::Integer(20),
            "exec-call",
            /*approval_id*/ None,
            "turn-1",
        ));
        implicit_exec_store
            .push_notification(item_started("turn-1", command_execution_item("exec-call")));
        assert!(implicit_exec_store.pending_replay_requests().is_empty());
        assert!(!implicit_exec_store.has_pending_thread_approvals());

        let mut explicit_exec_store = ThreadEventStore::new(/*capacity*/ 8);
        explicit_exec_store.push_request(exec_approval_request_with_request_id(
            AppServerRequestId::Integer(21),
            "exec-call",
            Some("retry-approval"),
            "turn-1",
        ));
        explicit_exec_store
            .push_notification(item_started("turn-1", command_execution_item("exec-call")));
        assert_eq!(explicit_exec_store.pending_replay_requests().len(), 1);
        assert!(explicit_exec_store.has_pending_thread_approvals());

        let mut patch_store = ThreadEventStore::new(/*capacity*/ 8);
        patch_store.push_request(ServerRequest::FileChangeRequestApproval {
            request_id: AppServerRequestId::Integer(22),
            params: FileChangeRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "patch-call".to_string(),
                started_at_ms: 0,
                reason: None,
                grant_root: None,
            },
        });
        patch_store.push_notification(item_started("turn-1", file_change_item("patch-call")));
        assert!(patch_store.pending_replay_requests().is_empty());
        assert!(!patch_store.has_pending_thread_approvals());
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
            Some(ThreadBufferedEvent::Request(ServerRequest::ToolRequestUserInput { params, .. }))
                if params.item_id == "call-2"
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
            Some(ThreadBufferedEvent::Request(ServerRequest::ToolRequestUserInput { params, .. }))
                if params.item_id == "call-2"
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
                ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval { .. })
                    | ThreadBufferedEvent::Request(ServerRequest::FileChangeRequestApproval { .. })
            )
        }));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_elicitation_after_outbound_resolution() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        let request_id = AppServerRequestId::String("request-1".to_string());
        store.push_request(elicitation_request("server-1", "request-1", "turn-1"));

        store.note_outbound_op(&Op::ResolveElicitation {
            server_name: "server-1".to_string(),
            request_id,
            decision: McpServerElicitationAction::Accept,
            content: None,
            meta: None,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved elicitation prompt should not replay on thread switch"
        );
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
                ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval { .. })
            )
        }));
    }
}
