use std::collections::HashMap;

use crate::app_command::AppCommand;
use crate::app_command::AppCommandView;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::protocol::ReviewDecision;

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

#[derive(Debug, Default)]
pub(super) struct PendingAppServerRequests {
    exec_approvals: HashMap<String, AppServerRequestId>,
    file_change_approvals: HashMap<String, AppServerRequestId>,
    permissions_approvals: HashMap<String, AppServerRequestId>,
    user_inputs: HashMap<String, AppServerRequestId>,
    mcp_requests: HashMap<McpLegacyRequestKey, AppServerRequestId>,
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
                    .insert(params.turn_id.clone(), request_id.clone());
                None
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                self.mcp_requests.insert(
                    McpLegacyRequestKey {
                        server_name: params.server_name.clone(),
                        request_id: app_server_request_id_to_mcp_request_id(request_id),
                    },
                    request_id.clone(),
                );
                None
            }
            ServerRequest::DynamicToolCall { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Dynamic tool calls are not available in app-server TUI yet."
                        .to_string(),
                })
            }
            ServerRequest::ChatgptAuthTokensRefresh { .. } => None,
            ServerRequest::ApplyPatchApproval { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message:
                        "Legacy patch approval requests are not available in app-server TUI yet."
                            .to_string(),
                })
            }
            ServerRequest::ExecCommandApproval { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message:
                        "Legacy command approval requests are not available in app-server TUI yet."
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
        let resolution = match op.view() {
            AppCommandView::ExecApproval { id, decision, .. } => self
                .exec_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(CommandExecutionRequestApprovalResponse {
                            decision: decision.clone().into(),
                        })
                        .map_err(|err| {
                            format!("failed to serialize command execution approval response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommandView::PatchApproval { id, decision } => self
                .file_change_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(FileChangeRequestApprovalResponse {
                            decision: file_change_decision(decision)?,
                        })
                        .map_err(|err| {
                            format!("failed to serialize file change approval response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommandView::RequestPermissionsResponse { id, response } => self
                .permissions_approvals
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(PermissionsRequestApprovalResponse {
                            permissions: serde_json::from_value::<GrantedPermissionProfile>(
                                serde_json::to_value(&response.permissions).map_err(|err| {
                                    format!("failed to encode granted permissions: {err}")
                                })?,
                            )
                            .map_err(|err| {
                                format!("failed to decode granted permissions for app-server: {err}")
                            })?,
                            scope: response.scope.into(),
                        })
                        .map_err(|err| {
                            format!("failed to serialize permissions approval response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommandView::UserInputAnswer { id, response } => self
                .user_inputs
                .remove(id)
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(
                            serde_json::from_value::<ToolRequestUserInputResponse>(
                                serde_json::to_value(response).map_err(|err| {
                                    format!("failed to encode request_user_input response: {err}")
                                })?,
                            )
                            .map_err(|err| {
                                format!(
                                    "failed to decode request_user_input response for app-server: {err}"
                                )
                            })?,
                        )
                        .map_err(|err| {
                            format!("failed to serialize request_user_input response: {err}")
                        })?,
                    })
                })
                .transpose()?,
            AppCommandView::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => self
                .mcp_requests
                .remove(&McpLegacyRequestKey {
                    server_name: server_name.to_string(),
                    request_id: request_id.clone(),
                })
                .map(|request_id| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id,
                        result: serde_json::to_value(McpServerElicitationRequestResponse {
                            action: match decision {
                                codex_protocol::approvals::ElicitationAction::Accept => {
                                    McpServerElicitationAction::Accept
                                }
                                codex_protocol::approvals::ElicitationAction::Decline => {
                                    McpServerElicitationAction::Decline
                                }
                                codex_protocol::approvals::ElicitationAction::Cancel => {
                                    McpServerElicitationAction::Cancel
                                }
                            },
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

    pub(super) fn resolve_notification(&mut self, request_id: &AppServerRequestId) {
        self.exec_approvals.retain(|_, value| value != request_id);
        self.file_change_approvals
            .retain(|_, value| value != request_id);
        self.permissions_approvals
            .retain(|_, value| value != request_id);
        self.user_inputs.retain(|_, value| value != request_id);
        self.mcp_requests.retain(|_, value| value != request_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct McpLegacyRequestKey {
    server_name: String,
    request_id: McpRequestId,
}

fn app_server_request_id_to_mcp_request_id(request_id: &AppServerRequestId) -> McpRequestId {
    match request_id {
        AppServerRequestId::String(value) => McpRequestId::String(value.clone()),
        AppServerRequestId::Integer(value) => McpRequestId::Integer(*value),
    }
}

fn file_change_decision(decision: &ReviewDecision) -> Result<FileChangeApprovalDecision, String> {
    match decision {
        ReviewDecision::Approved => Ok(FileChangeApprovalDecision::Accept),
        ReviewDecision::ApprovedForSession => Ok(FileChangeApprovalDecision::AcceptForSession),
        ReviewDecision::ApprovedPersistToProfile => Ok(FileChangeApprovalDecision::Accept),
        ReviewDecision::Denied => Ok(FileChangeApprovalDecision::Decline),
        ReviewDecision::Abort => Ok(FileChangeApprovalDecision::Cancel),
        ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
            Err("execpolicy amendment is not a valid file change approval decision".to_string())
        }
        ReviewDecision::NetworkPolicyAmendment { .. } => {
            Err("network policy amendment is not a valid file change approval decision".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PendingAppServerRequests;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::McpElicitationObjectType;
    use codex_app_server_protocol::McpElicitationSchema;
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
    use codex_protocol::approvals::ElicitationAction;
    use codex_protocol::approvals::ExecPolicyAmendment;
    use codex_protocol::mcp::RequestId as McpRequestId;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::ReviewDecision;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;

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
                skill_metadata: None,
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
                decision: ReviewDecision::Approved,
            })
            .expect("resolution should serialize")
            .expect("request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(41));
        assert_eq!(resolution.result, json!({ "decision": "accept" }));
    }

    #[test]
    fn resolves_permissions_and_user_input_through_app_server_request_id() {
        let mut pending = PendingAppServerRequests::default();

        assert_eq!(
            pending.note_server_request(&ServerRequest::PermissionsRequestApproval {
                request_id: AppServerRequestId::Integer(7),
                params: PermissionsRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "perm-1".to_string(),
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
                    permissions: serde_json::from_value(json!({
                        "network": { "enabled": null }
                    }))
                    .expect("valid permissions"),
                    scope: codex_protocol::request_permissions::PermissionGrantScope::Session,
                },
                persist_permissions: None,
            })
            .expect("permissions response should serialize")
            .expect("permissions request should be pending");
        assert_eq!(permissions.request_id, AppServerRequestId::Integer(7));
        assert_eq!(
            serde_json::from_value::<PermissionsRequestApprovalResponse>(permissions.result)
                .expect("permissions response should decode"),
            PermissionsRequestApprovalResponse {
                permissions: serde_json::from_value(json!({
                    "network": { "enabled": null }
                }))
                .expect("valid permissions"),
                scope: PermissionGrantScope::Session,
            }
        );

        let user_input = pending
            .take_resolution(&Op::UserInputAnswer {
                id: "turn-2".to_string(),
                response: codex_protocol::request_user_input::RequestUserInputResponse {
                    answers: std::iter::once((
                        "question".to_string(),
                        codex_protocol::request_user_input::RequestUserInputAnswer {
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
                request_id: McpRequestId::Integer(12),
                decision: ElicitationAction::Accept,
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
                    tool: "tool".to_string(),
                    arguments: json!({}),
                },
            })
            .expect("dynamic tool calls should be rejected");

        assert_eq!(unsupported.request_id, AppServerRequestId::Integer(99));
        assert_eq!(
            unsupported.message,
            "Dynamic tool calls are not available in app-server TUI yet."
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
    fn rejects_invalid_patch_decisions_for_file_change_requests() {
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

        let error = pending
            .take_resolution(&Op::PatchApproval {
                id: "patch-1".to_string(),
                decision: ReviewDecision::ApprovedExecpolicyAmendment {
                    proposed_execpolicy_amendment: ExecPolicyAmendment::new(vec![
                        "echo".to_string(),
                        "hi".to_string(),
                    ]),
                },
            })
            .expect_err("invalid patch decision should fail");

        assert_eq!(
            error,
            "execpolicy amendment is not a valid file change approval decision"
        );
    }
}
