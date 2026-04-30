use std::collections::HashMap;
use std::collections::VecDeque;

use super::App;
use crate::app_command::AppCommand;
use crate::app_server_approval_conversions::granted_permission_profile_from_request;
use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerRequest;

impl App {
    pub(super) async fn reject_app_server_request(
        &self,
        app_server_client: &AppServerSession,
        request_id: AppServerRequestId,
        reason: String,
    ) -> std::result::Result<(), String> {
        app_server_client
            .reject_server_request(
                request_id,
                JSONRPCErrorError {
                    code: -32000,
                    message: reason,
                    data: None,
                },
            )
            .await
            .map_err(|err| format!("failed to reject app-server request: {err}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AppServerRequestResolution {
    pub(super) request_id: AppServerRequestId,
    pub(super) result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnsupportedAppServerRequest {
    pub(super) request_id: AppServerRequestId,
    pub(super) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedAppServerRequest {
    ExecApproval {
        id: String,
    },
    FileChangeApproval {
        id: String,
    },
    PermissionsApproval {
        id: String,
    },
    UserInput {
        call_id: String,
    },
    McpElicitation {
        server_name: String,
        request_id: AppServerRequestId,
    },
}

#[derive(Debug, Default)]
pub(super) struct PendingAppServerRequests {
    exec_approvals: HashMap<String, AppServerRequestId>,
    file_change_approvals: HashMap<String, AppServerRequestId>,
    permissions_approvals: HashMap<String, AppServerRequestId>,
    user_inputs: HashMap<String, VecDeque<PendingUserInputRequest>>,
    mcp_requests: HashMap<McpRequestKey, AppServerRequestId>,
}

impl PendingAppServerRequests {
    pub(super) fn clear(&mut self) {
        self.exec_approvals.clear();
        self.file_change_approvals.clear();
        self.permissions_approvals.clear();
        self.user_inputs.clear();
        self.mcp_requests.clear();
    }

    pub(super) fn note_server_request(
        &mut self,
        request: &ServerRequest,
    ) -> Option<UnsupportedAppServerRequest> {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let approval_id = params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone());
                self.exec_approvals.insert(approval_id, request_id.clone());
                None
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                self.file_change_approvals
                    .insert(params.item_id.clone(), request_id.clone());
                None
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                self.permissions_approvals
                    .insert(params.item_id.clone(), request_id.clone());
                None
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                self.user_inputs
                    .entry(params.turn_id.clone())
                    .or_default()
                    .push_back(PendingUserInputRequest {
                        item_id: params.item_id.clone(),
                        request_id: request_id.clone(),
                    });
                None
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                self.mcp_requests.insert(
                    McpRequestKey {
                        server_name: params.server_name.clone(),
                        request_id: request_id.clone(),
                    },
                    request_id.clone(),
                );
                None
            }
            ServerRequest::DynamicToolCall { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Dynamic tool calls are not available in TUI yet.".to_string(),
                })
            }
            ServerRequest::ChatgptAuthTokensRefresh { .. } => None,
            ServerRequest::ApplyPatchApproval { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Legacy patch approval requests are not available in TUI yet."
                        .to_string(),
                })
            }
            ServerRequest::ExecCommandApproval { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Legacy command approval requests are not available in TUI yet."
                        .to_string(),
                })
            }
        }
    }

    pub(super) fn take_resolution<T>(
        &mut self,
        op: T,
    ) -> Result<Option<AppServerRequestResolution>, String>
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        let resolution = match &op {
            AppCommand::ExecApproval { id, decision, .. } => self
                .exec_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(CommandExecutionRequestApprovalResponse {
                            decision: decision.clone(),
                        })
                        .map_err(|err| {
                            format!(
                                "failed to serialize command execution approval response: {err}"
                            )
                        })?,
                    })
                })
                .transpose()?,
            AppCommand::PatchApproval { id, decision } => self
                .file_change_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(FileChangeRequestApprovalResponse {
                            decision: decision.clone(),
                        })
                        .map_err(|err| {
                            format!("failed to serialize file change approval response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommand::RequestPermissionsResponse { id, response } => self
                .permissions_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(PermissionsRequestApprovalResponse {
                            permissions: granted_permission_profile_from_request(
                                response.permissions.clone(),
                            ),
                            scope: response.scope.into(),
                            strict_auto_review: response.strict_auto_review.then_some(true),
                        })
                        .map_err(|err| {
                            format!("failed to serialize permissions approval response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommand::UserInputAnswer { id, response } => self
                .pop_user_input_request_for_turn(id)
                .map(|pending| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id: pending.request_id,
                        result: serde_json::to_value(response).map_err(|err| {
                            format!("failed to serialize request_user_input response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommand::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => self
                .mcp_requests
                .remove(&McpRequestKey {
                    server_name: server_name.to_string(),
                    request_id: request_id.clone(),
                })
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(McpServerElicitationRequestResponse {
                            action: *decision,
                            content: content.clone(),
                            meta: meta.clone(),
                        })
                        .map_err(|err| {
                            format!("failed to serialize MCP elicitation response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            _ => None,
        };
        Ok(resolution)
    }

    pub(super) fn resolve_notification(
        &mut self,
        request_id: &AppServerRequestId,
    ) -> Option<ResolvedAppServerRequest> {
        if let Some(id) = self
            .exec_approvals
            .iter()
            .find_map(|(id, value)| (value == request_id).then(|| id.clone()))
        {
            self.exec_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::ExecApproval { id });
        }

        if let Some(id) = self
            .file_change_approvals
            .iter()
            .find_map(|(id, value)| (value == request_id).then(|| id.clone()))
        {
            self.file_change_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::FileChangeApproval { id });
        }

        if let Some(id) = self
            .permissions_approvals
            .iter()
            .find_map(|(id, value)| (value == request_id).then(|| id.clone()))
        {
            self.permissions_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::PermissionsApproval { id });
        }

        if let Some(pending) = self.remove_user_input_request(request_id) {
            return Some(ResolvedAppServerRequest::UserInput {
                call_id: pending.item_id,
            });
        }

        if let Some(key) = self
            .mcp_requests
            .iter()
            .find_map(|(key, value)| (value == request_id).then(|| key.clone()))
        {
            self.mcp_requests.remove(&key);
            return Some(ResolvedAppServerRequest::McpElicitation {
                server_name: key.server_name,
                request_id: key.request_id,
            });
        }

        None
    }

    pub(super) fn contains_server_request(&self, request: &ServerRequest) -> bool {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, .. } => self
                .exec_approvals
                .values()
                .any(|pending_request_id| pending_request_id == request_id),
            ServerRequest::FileChangeRequestApproval { request_id, .. } => self
                .file_change_approvals
                .values()
                .any(|pending_request_id| pending_request_id == request_id),
            ServerRequest::PermissionsRequestApproval { request_id, .. } => self
                .permissions_approvals
                .values()
                .any(|pending_request_id| pending_request_id == request_id),
            ServerRequest::ToolRequestUserInput { request_id, .. } => {
                self.user_inputs.values().any(|queue| {
                    queue
                        .iter()
                        .any(|pending| &pending.request_id == request_id)
                })
            }
            ServerRequest::McpServerElicitationRequest { request_id, .. } => self
                .mcp_requests
                .values()
                .any(|pending_request_id| pending_request_id == request_id),
            ServerRequest::DynamicToolCall { .. }
            | ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. } => true,
        }
    }

    fn pop_user_input_request_for_turn(
        &mut self,
        turn_id: &str,
    ) -> Option<PendingUserInputRequest> {
        let pending = self
            .user_inputs
            .get_mut(turn_id)
            .and_then(VecDeque::pop_front);
        if self
            .user_inputs
            .get(turn_id)
            .is_some_and(VecDeque::is_empty)
        {
            self.user_inputs.remove(turn_id);
        }
        pending
    }

    fn remove_user_input_request(
        &mut self,
        request_id: &AppServerRequestId,
    ) -> Option<PendingUserInputRequest> {
        let (turn_id, index) = self.user_inputs.iter().find_map(|(turn_id, queue)| {
            queue
                .iter()
                .position(|pending| &pending.request_id == request_id)
                .map(|index| (turn_id.clone(), index))
        })?;
        let queue = self.user_inputs.get_mut(&turn_id)?;
        let removed = queue.remove(index);
        if queue.is_empty() {
            self.user_inputs.remove(&turn_id);
        }
        removed
    }
}

#[derive(Debug)]
struct PendingUserInputRequest {
    item_id: String,
    request_id: AppServerRequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct McpRequestKey {
    server_name: String,
    request_id: AppServerRequestId,
}

#[cfg(test)]
mod tests {
    use super::PendingAppServerRequests;
    use super::ResolvedAppServerRequest;
    use crate::app_command::AppCommand as Op;
    use codex_app_server_protocol::AdditionalFileSystemPermissions;
    use codex_app_server_protocol::AdditionalNetworkPermissions;
    use codex_app_server_protocol::CommandExecutionApprovalDecision;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::FileChangeApprovalDecision;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::McpElicitationObjectType;
    use codex_app_server_protocol::McpElicitationSchema;
    use codex_app_server_protocol::McpServerElicitationAction;
    use codex_app_server_protocol::McpServerElicitationRequest;
    use codex_app_server_protocol::McpServerElicitationRequestParams;
    use codex_app_server_protocol::PermissionGrantScope;
    use codex_app_server_protocol::PermissionsRequestApprovalParams;
    use codex_app_server_protocol::PermissionsRequestApprovalResponse;
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::ServerRequest;
    use codex_app_server_protocol::ToolRequestUserInputAnswer;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_app_server_protocol::ToolRequestUserInputResponse;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::request_permissions::RequestPermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn resolves_exec_approval_through_app_server_request_id() {
        let mut pending = PendingAppServerRequests::default();
        let request = ServerRequest::CommandExecutionRequestApproval {
            request_id: AppServerRequestId::Integer(41),
            params: CommandExecutionRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "call-1".to_string(),
                approval_id: Some("approval-1".to_string()),
                reason: None,
                network_approval_context: None,
                command: Some("ls".to_string()),
                cwd: None,
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        };

        assert_eq!(pending.note_server_request(&request), None);

        let resolution = pending
            .take_resolution(&Op::ExecApproval {
                id: "approval-1".to_string(),
                turn_id: None,
                decision: CommandExecutionApprovalDecision::Accept,
            })
            .expect("resolution should serialize")
            .expect("request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(41));
        assert_eq!(resolution.result, json!({ "decision": "accept" }));
    }

    #[test]
    fn resolves_permissions_and_user_input_through_app_server_request_id() {
        let mut pending = PendingAppServerRequests::default();
        let read_path = if cfg!(windows) {
            r"C:\tmp\read-only"
        } else {
            "/tmp/read-only"
        };
        let write_path = if cfg!(windows) {
            r"C:\tmp\write"
        } else {
            "/tmp/write"
        };
        let absolute_path = |path: &str| {
            AbsolutePathBuf::try_from(PathBuf::from(path)).expect("path must be absolute")
        };

        assert_eq!(
            pending.note_server_request(&ServerRequest::PermissionsRequestApproval {
                request_id: AppServerRequestId::Integer(7),
                params: PermissionsRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "perm-1".to_string(),
                    cwd: absolute_path(if cfg!(windows) { r"C:\tmp" } else { "/tmp" }),
                    reason: None,
                    permissions: serde_json::from_value(json!({
                        "network": { "enabled": null }
                    }))
                    .expect("valid permissions"),
                },
            }),
            None
        );
        assert_eq!(
            pending.note_server_request(&ServerRequest::ToolRequestUserInput {
                request_id: AppServerRequestId::Integer(8),
                params: ToolRequestUserInputParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-2".to_string(),
                    item_id: "tool-1".to_string(),
                    questions: Vec::new(),
                },
            }),
            None
        );

        let permissions = pending
            .take_resolution(&Op::RequestPermissionsResponse {
                id: "perm-1".to_string(),
                response: codex_protocol::request_permissions::RequestPermissionsResponse {
                    permissions: RequestPermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        file_system: Some(FileSystemPermissions::from_read_write_roots(
                            Some(vec![absolute_path(read_path)]),
                            Some(vec![absolute_path(write_path)]),
                        )),
                    },
                    scope: codex_protocol::request_permissions::PermissionGrantScope::Session,
                    strict_auto_review: false,
                },
            })
            .expect("permissions response should serialize")
            .expect("permissions request should be pending");
        assert_eq!(permissions.request_id, AppServerRequestId::Integer(7));
        assert_eq!(
            serde_json::from_value::<PermissionsRequestApprovalResponse>(permissions.result)
                .expect("permissions response should decode"),
            PermissionsRequestApprovalResponse {
                permissions: codex_app_server_protocol::GrantedPermissionProfile {
                    network: Some(AdditionalNetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(AdditionalFileSystemPermissions {
                        read: Some(vec![absolute_path(read_path)]),
                        write: Some(vec![absolute_path(write_path)]),
                        glob_scan_max_depth: None,
                        entries: Some(vec![
                            codex_app_server_protocol::FileSystemSandboxEntry {
                                path: codex_app_server_protocol::FileSystemPath::Path {
                                    path: absolute_path(read_path),
                                },
                                access: codex_app_server_protocol::FileSystemAccessMode::Read,
                            },
                            codex_app_server_protocol::FileSystemSandboxEntry {
                                path: codex_app_server_protocol::FileSystemPath::Path {
                                    path: absolute_path(write_path),
                                },
                                access: codex_app_server_protocol::FileSystemAccessMode::Write,
                            },
                        ]),
                    }),
                },
                scope: PermissionGrantScope::Session,
                strict_auto_review: None,
            }
        );

        let user_input = pending
            .take_resolution(&Op::UserInputAnswer {
                id: "turn-2".to_string(),
                response: ToolRequestUserInputResponse {
                    answers: std::iter::once((
                        "question".to_string(),
                        ToolRequestUserInputAnswer {
                            answers: vec!["yes".to_string()],
                        },
                    ))
                    .collect(),
                },
            })
            .expect("user input response should serialize")
            .expect("user input request should be pending");
        assert_eq!(user_input.request_id, AppServerRequestId::Integer(8));
        assert_eq!(
            serde_json::from_value::<ToolRequestUserInputResponse>(user_input.result)
                .expect("user input response should decode"),
            ToolRequestUserInputResponse {
                answers: std::iter::once((
                    "question".to_string(),
                    ToolRequestUserInputAnswer {
                        answers: vec!["yes".to_string()],
                    },
                ))
                .collect(),
            }
        );
    }

    #[test]
    fn correlates_mcp_elicitation_server_request_with_resolution() {
        let mut pending = PendingAppServerRequests::default();

        assert_eq!(
            pending.note_server_request(&ServerRequest::McpServerElicitationRequest {
                request_id: AppServerRequestId::Integer(12),
                params: McpServerElicitationRequestParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: Some("turn-1".to_string()),
                    server_name: "example".to_string(),
                    request: McpServerElicitationRequest::Form {
                        meta: None,
                        message: "Need input".to_string(),
                        requested_schema: McpElicitationSchema {
                            schema_uri: None,
                            type_: McpElicitationObjectType::Object,
                            properties: BTreeMap::new(),
                            required: None,
                        },
                    },
                },
            }),
            None
        );

        let resolution = pending
            .take_resolution(&Op::ResolveElicitation {
                server_name: "example".to_string(),
                request_id: AppServerRequestId::Integer(12),
                decision: McpServerElicitationAction::Accept,
                content: Some(json!({ "answer": "yes" })),
                meta: Some(json!({ "source": "tui" })),
            })
            .expect("elicitation response should serialize")
            .expect("elicitation request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(12));
        assert_eq!(
            resolution.result,
            json!({
                "action": "accept",
                "content": { "answer": "yes" },
                "_meta": { "source": "tui" }
            })
        );
    }

    #[test]
    fn rejects_dynamic_tool_calls_as_unsupported() {
        let mut pending = PendingAppServerRequests::default();
        let unsupported = pending
            .note_server_request(&ServerRequest::DynamicToolCall {
                request_id: AppServerRequestId::Integer(99),
                params: codex_app_server_protocol::DynamicToolCallParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "tool-1".to_string(),
                    namespace: None,
                    tool: "tool".to_string(),
                    arguments: json!({}),
                },
            })
            .expect("dynamic tool calls should be rejected");

        assert_eq!(unsupported.request_id, AppServerRequestId::Integer(99));
        assert_eq!(
            unsupported.message,
            "Dynamic tool calls are not available in TUI yet."
        );
    }

    #[test]
    fn does_not_mark_chatgpt_auth_refresh_as_unsupported() {
        let mut pending = PendingAppServerRequests::default();

        assert_eq!(
            pending.note_server_request(&ServerRequest::ChatgptAuthTokensRefresh {
                request_id: AppServerRequestId::Integer(100),
                params: codex_app_server_protocol::ChatgptAuthTokensRefreshParams {
                    reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                    previous_account_id: Some("workspace-1".to_string()),
                },
            }),
            None
        );
    }

    #[test]
    fn resolves_patch_approval_through_app_server_request_id() {
        let mut pending = PendingAppServerRequests::default();
        assert_eq!(
            pending.note_server_request(&ServerRequest::FileChangeRequestApproval {
                request_id: AppServerRequestId::Integer(13),
                params: FileChangeRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "patch-1".to_string(),
                    reason: None,
                    grant_root: None,
                },
            }),
            None
        );

        let resolution = pending
            .take_resolution(&Op::PatchApproval {
                id: "patch-1".to_string(),
                decision: FileChangeApprovalDecision::Cancel,
            })
            .expect("resolution should serialize")
            .expect("request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(13));
        assert_eq!(resolution.result, json!({ "decision": "cancel" }));
    }

    #[test]
    fn resolve_notification_returns_resolved_exec_request() {
        let mut pending = PendingAppServerRequests::default();
        assert_eq!(
            pending.note_server_request(&ServerRequest::CommandExecutionRequestApproval {
                request_id: AppServerRequestId::Integer(41),
                params: CommandExecutionRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-1".to_string(),
                    approval_id: Some("approval-1".to_string()),
                    reason: None,
                    network_approval_context: None,
                    command: Some("ls".to_string()),
                    cwd: None,
                    command_actions: None,
                    additional_permissions: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    available_decisions: None,
                },
            }),
            None
        );

        assert_eq!(
            pending.resolve_notification(&AppServerRequestId::Integer(41)),
            Some(ResolvedAppServerRequest::ExecApproval {
                id: "approval-1".to_string(),
            })
        );
        assert_eq!(
            pending.resolve_notification(&AppServerRequestId::Integer(41)),
            None
        );
    }

    #[test]
    fn resolve_notification_returns_resolved_mcp_request() {
        let mut pending = PendingAppServerRequests::default();
        assert_eq!(
            pending.note_server_request(&ServerRequest::McpServerElicitationRequest {
                request_id: AppServerRequestId::Integer(12),
                params: McpServerElicitationRequestParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: Some("turn-1".to_string()),
                    server_name: "example".to_string(),
                    request: McpServerElicitationRequest::Form {
                        meta: None,
                        message: "Need input".to_string(),
                        requested_schema: McpElicitationSchema {
                            schema_uri: None,
                            type_: McpElicitationObjectType::Object,
                            properties: BTreeMap::new(),
                            required: None,
                        },
                    },
                },
            }),
            None
        );

        assert_eq!(
            pending.resolve_notification(&AppServerRequestId::Integer(12)),
            Some(ResolvedAppServerRequest::McpElicitation {
                server_name: "example".to_string(),
                request_id: AppServerRequestId::Integer(12),
            })
        );
    }

    #[test]
    fn resolve_notification_returns_resolved_user_input_item_id() {
        let mut pending = PendingAppServerRequests::default();
        pending.note_server_request(&ServerRequest::ToolRequestUserInput {
            request_id: AppServerRequestId::Integer(8),
            params: ToolRequestUserInputParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "tool-1".to_string(),
                questions: Vec::new(),
            },
        });

        assert_eq!(
            pending.resolve_notification(&AppServerRequestId::Integer(8)),
            Some(ResolvedAppServerRequest::UserInput {
                call_id: "tool-1".to_string(),
            })
        );
    }

    #[test]
    fn same_turn_user_input_answers_resolve_app_server_requests_fifo() {
        let mut pending = PendingAppServerRequests::default();
        for (request_id, item_id) in [(8, "tool-1"), (9, "tool-2")] {
            pending.note_server_request(&ServerRequest::ToolRequestUserInput {
                request_id: AppServerRequestId::Integer(request_id),
                params: ToolRequestUserInputParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: item_id.to_string(),
                    questions: Vec::new(),
                },
            });
        }

        let response = ToolRequestUserInputResponse {
            answers: HashMap::new(),
        };
        let first_response = pending
            .take_resolution(&Op::UserInputAnswer {
                id: "turn-1".to_string(),
                response: response.clone(),
            })
            .expect("user input response should serialize")
            .expect("first user input request should be pending");
        let second_response = pending
            .take_resolution(&Op::UserInputAnswer {
                id: "turn-1".to_string(),
                response,
            })
            .expect("user input response should serialize")
            .expect("second user input request should be pending");

        assert_eq!(first_response.request_id, AppServerRequestId::Integer(8));
        assert_eq!(second_response.request_id, AppServerRequestId::Integer(9));
    }
}
