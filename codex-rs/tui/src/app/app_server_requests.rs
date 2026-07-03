use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;

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
use codex_protocol::ThreadId;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;

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
pub(crate) struct AppServerRequestIdentity {
    pub(crate) thread_id: ThreadId,
    pub(crate) request_id: AppServerRequestId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedAppServerRequest {
    ExecApproval {
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        id: String,
    },
    FileChangeApproval {
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        id: String,
    },
    PermissionsApproval {
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        id: String,
    },
    UserInput {
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        turn_id: String,
        call_id: String,
    },
    McpElicitation {
        thread_id: ThreadId,
        server_name: String,
        request_id: AppServerRequestId,
    },
}

impl ResolvedAppServerRequest {
    pub(crate) fn thread_id(&self) -> ThreadId {
        match self {
            ResolvedAppServerRequest::ExecApproval { thread_id, .. }
            | ResolvedAppServerRequest::FileChangeApproval { thread_id, .. }
            | ResolvedAppServerRequest::PermissionsApproval { thread_id, .. }
            | ResolvedAppServerRequest::UserInput { thread_id, .. }
            | ResolvedAppServerRequest::McpElicitation { thread_id, .. } => *thread_id,
        }
    }

    pub(crate) fn request_id(&self) -> &AppServerRequestId {
        match self {
            ResolvedAppServerRequest::ExecApproval { request_id, .. }
            | ResolvedAppServerRequest::FileChangeApproval { request_id, .. }
            | ResolvedAppServerRequest::PermissionsApproval { request_id, .. }
            | ResolvedAppServerRequest::UserInput { request_id, .. }
            | ResolvedAppServerRequest::McpElicitation { request_id, .. } => request_id,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct PendingAppServerRequests {
    exec_approvals: HashMap<ThreadRequestKey, AppServerRequestId>,
    file_change_approvals: HashMap<ThreadRequestKey, AppServerRequestId>,
    permissions_approvals: HashMap<ThreadRequestKey, AppServerRequestId>,
    user_inputs: HashMap<ThreadTurnKey, VecDeque<PendingUserInputRequest>>,
    mcp_requests: HashMap<McpRequestKey, ThreadId>,
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
                let thread_id = match Self::parse_thread_id(request_id, &params.thread_id) {
                    Ok(thread_id) => thread_id,
                    Err(unsupported) => return Some(unsupported),
                };
                let approval_id = params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone());
                Self::insert_approval(
                    &mut self.exec_approvals,
                    ThreadRequestKey::new(thread_id, approval_id),
                    request_id,
                    "command execution approval",
                )
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let thread_id = match Self::parse_thread_id(request_id, &params.thread_id) {
                    Ok(thread_id) => thread_id,
                    Err(unsupported) => return Some(unsupported),
                };
                Self::insert_approval(
                    &mut self.file_change_approvals,
                    ThreadRequestKey::new(thread_id, params.item_id.clone()),
                    request_id,
                    "file change approval",
                )
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                let thread_id = match Self::parse_thread_id(request_id, &params.thread_id) {
                    Ok(thread_id) => thread_id,
                    Err(unsupported) => return Some(unsupported),
                };
                // TODO(anp): Remove this duplicate validation once core permission paths remain
                // PathUri after crossing the app-server boundary. Native permission paths do not
                // yet have an ingress validation step, so validate them here before recording the
                // request as pending. Discovering an invalid path later in a UI delivery path
                // would leave the app-server RPC waiting without a clean rejection path.
                if let Err(err) = CoreRequestPermissionProfile::try_from(params.permissions.clone())
                {
                    return Some(UnsupportedAppServerRequest {
                        request_id: request_id.clone(),
                        message: format!("failed to localize requested filesystem paths: {err}"),
                    });
                }
                Self::insert_approval(
                    &mut self.permissions_approvals,
                    ThreadRequestKey::new(thread_id, params.item_id.clone()),
                    request_id,
                    "permissions approval",
                )
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                let thread_id = match Self::parse_thread_id(request_id, &params.thread_id) {
                    Ok(thread_id) => thread_id,
                    Err(unsupported) => return Some(unsupported),
                };
                let key = ThreadTurnKey {
                    thread_id,
                    turn_id: params.turn_id.clone(),
                };
                if self.user_inputs.get(&key).is_some_and(|queue| {
                    queue
                        .iter()
                        .any(|pending| pending.item_id == params.item_id)
                }) {
                    return Some(Self::duplicate_request(
                        request_id,
                        thread_id,
                        "user input",
                        &params.item_id,
                    ));
                }
                self.user_inputs
                    .entry(key)
                    .or_default()
                    .push_back(PendingUserInputRequest {
                        item_id: params.item_id.clone(),
                        request_id: request_id.clone(),
                    });
                None
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                let thread_id = match Self::parse_thread_id(request_id, &params.thread_id) {
                    Ok(thread_id) => thread_id,
                    Err(unsupported) => return Some(unsupported),
                };
                let key = McpRequestKey {
                    server_name: params.server_name.clone(),
                    request_id: request_id.clone(),
                };
                match self.mcp_requests.entry(key) {
                    Entry::Vacant(entry) => {
                        entry.insert(thread_id);
                        None
                    }
                    Entry::Occupied(_) => {
                        let correlation_id = format!("{request_id:?}");
                        Some(Self::duplicate_request(
                            request_id,
                            thread_id,
                            "MCP elicitation",
                            &correlation_id,
                        ))
                    }
                }
            }
            ServerRequest::DynamicToolCall { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Dynamic tool calls are not available in TUI yet.".to_string(),
                })
            }
            ServerRequest::ChatgptAuthTokensRefresh { .. } => None,
            ServerRequest::AttestationGenerate { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "Attestation generation is not available in TUI.".to_string(),
                })
            }
            ServerRequest::CurrentTimeRead { request_id, .. } => {
                Some(UnsupportedAppServerRequest {
                    request_id: request_id.clone(),
                    message: "External current time is not available in TUI.".to_string(),
                })
            }
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
        thread_id: ThreadId,
        expected_request_id: &AppServerRequestId,
        op: T,
    ) -> Result<Option<AppServerRequestResolution>, String>
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        let resolution = match &op {
            AppCommand::ExecApproval { id, decision, .. } => self
                .remove_matching_approval(
                    PendingApprovalKind::Exec,
                    ThreadRequestKey::new(thread_id, id.clone()),
                    expected_request_id,
                )
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
                .remove_matching_approval(
                    PendingApprovalKind::FileChange,
                    ThreadRequestKey::new(thread_id, id.clone()),
                    expected_request_id,
                )
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
                .remove_matching_approval(
                    PendingApprovalKind::Permissions,
                    ThreadRequestKey::new(thread_id, id.clone()),
                    expected_request_id,
                )
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
                .remove_user_input_request_for_turn(thread_id, id, expected_request_id)
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
                request_id: mcp_request_id,
                decision,
                content,
                meta,
            } => self
                .remove_mcp_request(
                    thread_id,
                    expected_request_id,
                    &McpRequestKey {
                        server_name: server_name.to_string(),
                        request_id: mcp_request_id.clone(),
                    },
                )
                .map(|pending| {
                    Ok::<AppServerRequestResolution, String>(AppServerRequestResolution {
                        request_id: pending,
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
        thread_id: ThreadId,
        request_id: &AppServerRequestId,
    ) -> Option<ResolvedAppServerRequest> {
        if let Some(id) = self.exec_approvals.iter().find_map(|(key, value)| {
            (key.thread_id == thread_id && value == request_id).then(|| key.clone())
        }) {
            self.exec_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::ExecApproval {
                thread_id,
                request_id: request_id.clone(),
                id: id.request_key,
            });
        }

        if let Some(id) = self.file_change_approvals.iter().find_map(|(key, value)| {
            (key.thread_id == thread_id && value == request_id).then(|| key.clone())
        }) {
            self.file_change_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::FileChangeApproval {
                thread_id,
                request_id: request_id.clone(),
                id: id.request_key,
            });
        }

        if let Some(id) = self.permissions_approvals.iter().find_map(|(key, value)| {
            (key.thread_id == thread_id && value == request_id).then(|| key.clone())
        }) {
            self.permissions_approvals.remove(&id);
            return Some(ResolvedAppServerRequest::PermissionsApproval {
                thread_id,
                request_id: request_id.clone(),
                id: id.request_key,
            });
        }

        if let Some((key, pending)) = self.remove_user_input_request(thread_id, request_id) {
            return Some(ResolvedAppServerRequest::UserInput {
                thread_id,
                request_id: request_id.clone(),
                turn_id: key.turn_id,
                call_id: pending.item_id,
            });
        }

        if let Some(key) = self
            .mcp_requests
            .iter()
            .find_map(|(key, pending_thread_id)| {
                (*pending_thread_id == thread_id && &key.request_id == request_id)
                    .then(|| key.clone())
            })
        {
            self.mcp_requests.remove(&key);
            return Some(ResolvedAppServerRequest::McpElicitation {
                thread_id,
                server_name: key.server_name,
                request_id: key.request_id,
            });
        }

        None
    }

    pub(super) fn contains_server_request(&self, request: &ServerRequest) -> bool {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let Ok(thread_id) = ThreadId::from_string(&params.thread_id) else {
                    return false;
                };
                self.exec_approvals.get(&ThreadRequestKey::new(
                    thread_id,
                    params
                        .approval_id
                        .clone()
                        .unwrap_or_else(|| params.item_id.clone()),
                )) == Some(request_id)
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let Ok(thread_id) = ThreadId::from_string(&params.thread_id) else {
                    return false;
                };
                self.file_change_approvals
                    .get(&ThreadRequestKey::new(thread_id, params.item_id.clone()))
                    == Some(request_id)
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                let Ok(thread_id) = ThreadId::from_string(&params.thread_id) else {
                    return false;
                };
                self.permissions_approvals
                    .get(&ThreadRequestKey::new(thread_id, params.item_id.clone()))
                    == Some(request_id)
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                let Ok(thread_id) = ThreadId::from_string(&params.thread_id) else {
                    return false;
                };
                self.user_inputs
                    .get(&ThreadTurnKey {
                        thread_id,
                        turn_id: params.turn_id.clone(),
                    })
                    .is_some_and(|queue| {
                        queue.iter().any(|pending| {
                            &pending.request_id == request_id && pending.item_id == params.item_id
                        })
                    })
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                let Ok(thread_id) = ThreadId::from_string(&params.thread_id) else {
                    return false;
                };
                self.mcp_requests
                    .get(&McpRequestKey {
                        server_name: params.server_name.clone(),
                        request_id: request_id.clone(),
                    })
                    .is_some_and(|pending_thread_id| *pending_thread_id == thread_id)
            }
            ServerRequest::DynamicToolCall { .. }
            | ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::AttestationGenerate { .. }
            | ServerRequest::CurrentTimeRead { .. }
            | ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. } => true,
        }
    }

    fn remove_user_input_request_for_turn(
        &mut self,
        thread_id: ThreadId,
        turn_id: &str,
        request_id: &AppServerRequestId,
    ) -> Option<PendingUserInputRequest> {
        let key = ThreadTurnKey {
            thread_id,
            turn_id: turn_id.to_string(),
        };
        let queue = self.user_inputs.get_mut(&key)?;
        let index = queue
            .iter()
            .position(|pending| &pending.request_id == request_id)?;
        let pending = queue.remove(index);
        if self.user_inputs.get(&key).is_some_and(VecDeque::is_empty) {
            self.user_inputs.remove(&key);
        }
        pending
    }

    fn remove_user_input_request(
        &mut self,
        thread_id: ThreadId,
        request_id: &AppServerRequestId,
    ) -> Option<(ThreadTurnKey, PendingUserInputRequest)> {
        let (key, index) = self.user_inputs.iter().find_map(|(key, queue)| {
            (key.thread_id == thread_id).then(|| {
                queue
                    .iter()
                    .position(|pending| &pending.request_id == request_id)
                    .map(|index| (key.clone(), index))
            })?
        })?;
        let queue = self.user_inputs.get_mut(&key)?;
        let removed = queue.remove(index);
        if queue.is_empty() {
            self.user_inputs.remove(&key);
        }
        removed.map(|pending| (key, pending))
    }

    fn remove_mcp_request(
        &mut self,
        thread_id: ThreadId,
        expected_request_id: &AppServerRequestId,
        key: &McpRequestKey,
    ) -> Option<AppServerRequestId> {
        if &key.request_id != expected_request_id || self.mcp_requests.get(key) != Some(&thread_id)
        {
            return None;
        }
        self.mcp_requests.remove(key)?;
        Some(key.request_id.clone())
    }

    fn remove_matching_approval(
        &mut self,
        kind: PendingApprovalKind,
        key: ThreadRequestKey,
        request_id: &AppServerRequestId,
    ) -> Option<AppServerRequestId> {
        let approvals = match kind {
            PendingApprovalKind::Exec => &mut self.exec_approvals,
            PendingApprovalKind::FileChange => &mut self.file_change_approvals,
            PendingApprovalKind::Permissions => &mut self.permissions_approvals,
        };
        if approvals.get(&key) != Some(request_id) {
            return None;
        }
        approvals.remove(&key)
    }

    fn parse_thread_id(
        request_id: &AppServerRequestId,
        thread_id: &str,
    ) -> Result<ThreadId, UnsupportedAppServerRequest> {
        ThreadId::from_string(thread_id).map_err(|err| UnsupportedAppServerRequest {
            request_id: request_id.clone(),
            message: format!("invalid app-server request thread_id `{thread_id}`: {err}"),
        })
    }

    fn insert_approval(
        approvals: &mut HashMap<ThreadRequestKey, AppServerRequestId>,
        key: ThreadRequestKey,
        request_id: &AppServerRequestId,
        request_kind: &str,
    ) -> Option<UnsupportedAppServerRequest> {
        match approvals.entry(key.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(request_id.clone());
                None
            }
            Entry::Occupied(_) => Some(Self::duplicate_request(
                request_id,
                key.thread_id,
                request_kind,
                &key.request_key,
            )),
        }
    }

    fn duplicate_request(
        request_id: &AppServerRequestId,
        thread_id: ThreadId,
        request_kind: &str,
        correlation_id: &str,
    ) -> UnsupportedAppServerRequest {
        UnsupportedAppServerRequest {
            request_id: request_id.clone(),
            message: format!(
                "duplicate pending {request_kind} request `{correlation_id}` for thread {thread_id}"
            ),
        }
    }
}

#[derive(Debug)]
struct PendingUserInputRequest {
    item_id: String,
    request_id: AppServerRequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ThreadRequestKey {
    thread_id: ThreadId,
    request_key: String,
}

impl ThreadRequestKey {
    fn new(thread_id: ThreadId, request_key: String) -> Self {
        Self {
            thread_id,
            request_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ThreadTurnKey {
    thread_id: ThreadId,
    turn_id: String,
}

#[derive(Debug, Clone, Copy)]
enum PendingApprovalKind {
    Exec,
    FileChange,
    Permissions,
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
    use super::UnsupportedAppServerRequest;
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
    use codex_protocol::ThreadId;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::request_permissions::RequestPermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::path::PathBuf;

    const THREAD_1: &str = "00000000-0000-0000-0000-000000000001";
    const THREAD_2: &str = "00000000-0000-0000-0000-000000000002";

    fn thread_id(value: &str) -> ThreadId {
        ThreadId::from_string(value).expect("test thread id should be valid")
    }

    fn exec_request(
        thread_id: &str,
        request_id: i64,
        item_id: &str,
        approval_id: Option<&str>,
    ) -> ServerRequest {
        exec_request_with_request_id(
            thread_id,
            AppServerRequestId::Integer(request_id),
            item_id,
            approval_id,
        )
    }

    fn exec_request_with_request_id(
        thread_id: &str,
        request_id: AppServerRequestId,
        item_id: &str,
        approval_id: Option<&str>,
    ) -> ServerRequest {
        ServerRequest::CommandExecutionRequestApproval {
            request_id,
            params: CommandExecutionRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: item_id.to_string(),
                started_at_ms: 0,
                approval_id: approval_id.map(str::to_string),
                approval_purpose: None,
                environment_id: None,
                reason: None,
                network_approval_context: None,
                command: Some("true".to_string()),
                cwd: None,
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        }
    }

    fn file_change_request(thread_id: &str, request_id: i64, item_id: &str) -> ServerRequest {
        ServerRequest::FileChangeRequestApproval {
            request_id: AppServerRequestId::Integer(request_id),
            params: FileChangeRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: item_id.to_string(),
                started_at_ms: 0,
                reason: None,
                grant_root: None,
            },
        }
    }

    fn permissions_request(thread_id: &str, request_id: i64, item_id: &str) -> ServerRequest {
        ServerRequest::PermissionsRequestApproval {
            request_id: AppServerRequestId::Integer(request_id),
            params: PermissionsRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: item_id.to_string(),
                environment_id: None,
                started_at_ms: 0,
                cwd: AbsolutePathBuf::current_dir().expect("current dir should be absolute"),
                reason: None,
                permissions: serde_json::from_value(json!({
                    "network": { "enabled": null }
                }))
                .expect("valid permissions"),
            },
        }
    }

    fn user_input_request(
        thread_id: &str,
        request_id: i64,
        turn_id: &str,
        item_id: &str,
    ) -> ServerRequest {
        ServerRequest::ToolRequestUserInput {
            request_id: AppServerRequestId::Integer(request_id),
            params: ToolRequestUserInputParams {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
                item_id: item_id.to_string(),
                questions: Vec::new(),
                auto_resolution_ms: None,
            },
        }
    }

    fn mcp_request(thread_id: &str, request_id: i64) -> ServerRequest {
        ServerRequest::McpServerElicitationRequest {
            request_id: AppServerRequestId::Integer(request_id),
            params: McpServerElicitationRequestParams {
                thread_id: thread_id.to_string(),
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
        }
    }

    fn empty_user_input_response() -> ToolRequestUserInputResponse {
        ToolRequestUserInputResponse {
            answers: HashMap::new(),
        }
    }

    fn permissions_response() -> codex_protocol::request_permissions::RequestPermissionsResponse {
        codex_protocol::request_permissions::RequestPermissionsResponse {
            permissions: RequestPermissionProfile::default(),
            scope: codex_protocol::request_permissions::PermissionGrantScope::Session,
            strict_auto_review: false,
        }
    }

    #[test]
    fn resolves_exec_approval_through_app_server_request_id() {
        let mut pending = PendingAppServerRequests::default();
        let request = exec_request(
            THREAD_1,
            /*request_id*/ 41,
            "call-1",
            Some("approval-1"),
        );

        assert_eq!(pending.note_server_request(&request), None);

        let resolution = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(41),
                &Op::ExecApproval {
                    id: "approval-1".to_string(),
                    turn_id: None,
                    decision: CommandExecutionApprovalDecision::Accept,
                },
            )
            .expect("resolution should serialize")
            .expect("request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(41));
        assert_eq!(resolution.result, json!({ "decision": "accept" }));
    }

    #[test]
    fn duplicate_exec_approval_does_not_overwrite_pending_request() {
        let mut pending = PendingAppServerRequests::default();

        assert_eq!(
            pending.note_server_request(&exec_request(
                THREAD_1,
                /*request_id*/ 41,
                "call-1",
                Some("approval-1"),
            )),
            None
        );
        assert!(
            pending
                .note_server_request(&exec_request(
                    THREAD_1,
                    /*request_id*/ 42,
                    "call-2",
                    Some("approval-1"),
                ))
                .is_some()
        );

        let resolution = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(41),
                &Op::ExecApproval {
                    id: "approval-1".to_string(),
                    turn_id: None,
                    decision: CommandExecutionApprovalDecision::Accept,
                },
            )
            .expect("resolution should serialize")
            .expect("original request should remain pending");
        assert_eq!(resolution.request_id, AppServerRequestId::Integer(41));
    }

    #[test]
    fn rejects_permissions_with_paths_that_cannot_be_localized() {
        let mut pending = PendingAppServerRequests::default();
        let request_id = AppServerRequestId::Integer(7);
        let permissions = codex_app_server_protocol::RequestPermissionProfile {
            network: None,
            file_system: Some(AdditionalFileSystemPermissions {
                read: Some(vec![
                    serde_json::from_value(json!("relative/path"))
                        .expect("relative API path should deserialize"),
                ]),
                write: None,
                glob_scan_max_depth: None,
                entries: None,
            }),
        };
        let localization_error =
            RequestPermissionProfile::try_from(permissions.clone()).expect_err("relative path");
        let cwd = AbsolutePathBuf::try_from(PathBuf::from(if cfg!(windows) {
            r"C:\tmp"
        } else {
            "/tmp"
        }))
        .expect("path must be absolute");

        assert_eq!(
            pending.note_server_request(&ServerRequest::PermissionsRequestApproval {
                request_id: request_id.clone(),
                params: PermissionsRequestApprovalParams {
                    thread_id: THREAD_1.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "perm-1".to_string(),
                    environment_id: None,
                    started_at_ms: 0,
                    cwd,
                    reason: None,
                    permissions,
                },
            }),
            Some(UnsupportedAppServerRequest {
                request_id,
                message: format!(
                    "failed to localize requested filesystem paths: {localization_error}"
                ),
            })
        );
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
                    thread_id: THREAD_1.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "perm-1".to_string(),
                    environment_id: None,
                    started_at_ms: 0,
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
                    thread_id: THREAD_1.to_string(),
                    turn_id: "turn-2".to_string(),
                    item_id: "tool-1".to_string(),
                    questions: Vec::new(),
                    auto_resolution_ms: None,
                },
            }),
            None
        );

        let permissions = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(7),
                &Op::RequestPermissionsResponse {
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
                },
            )
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
                        read: Some(vec![absolute_path(read_path).into()]),
                        write: Some(vec![absolute_path(write_path).into()]),
                        glob_scan_max_depth: None,
                        entries: Some(vec![
                            codex_app_server_protocol::FileSystemSandboxEntry {
                                path: codex_app_server_protocol::FileSystemPath::Path {
                                    path: absolute_path(read_path).into(),
                                },
                                access: codex_app_server_protocol::FileSystemAccessMode::Read,
                            },
                            codex_app_server_protocol::FileSystemSandboxEntry {
                                path: codex_app_server_protocol::FileSystemPath::Path {
                                    path: absolute_path(write_path).into(),
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
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(8),
                &Op::UserInputAnswer {
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
                },
            )
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
            pending.note_server_request(&mcp_request(THREAD_1, /*request_id*/ 12)),
            None
        );

        let resolution = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(12),
                &Op::ResolveElicitation {
                    server_name: "example".to_string(),
                    request_id: AppServerRequestId::Integer(12),
                    decision: McpServerElicitationAction::Accept,
                    content: Some(json!({ "answer": "yes" })),
                    meta: Some(json!({ "source": "tui" })),
                },
            )
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
                    thread_id: THREAD_1.to_string(),
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
            pending.note_server_request(&file_change_request(
                THREAD_1, /*request_id*/ 13, "patch-1",
            )),
            None
        );

        let resolution = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(13),
                &Op::PatchApproval {
                    id: "patch-1".to_string(),
                    decision: FileChangeApprovalDecision::Cancel,
                },
            )
            .expect("resolution should serialize")
            .expect("request should be pending");

        assert_eq!(resolution.request_id, AppServerRequestId::Integer(13));
        assert_eq!(resolution.result, json!({ "decision": "cancel" }));
    }

    #[test]
    fn resolve_notification_returns_resolved_exec_request() {
        let mut pending = PendingAppServerRequests::default();
        assert_eq!(
            pending.note_server_request(&exec_request(
                THREAD_1,
                /*request_id*/ 41,
                "call-1",
                Some("approval-1"),
            )),
            None
        );

        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_1), &AppServerRequestId::Integer(41)),
            Some(ResolvedAppServerRequest::ExecApproval {
                thread_id: thread_id(THREAD_1),
                request_id: AppServerRequestId::Integer(41),
                id: "approval-1".to_string(),
            })
        );
        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_1), &AppServerRequestId::Integer(41)),
            None
        );
    }

    #[test]
    fn resolve_notification_returns_resolved_mcp_request() {
        let mut pending = PendingAppServerRequests::default();
        assert_eq!(
            pending.note_server_request(&mcp_request(THREAD_1, /*request_id*/ 12)),
            None
        );

        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_1), &AppServerRequestId::Integer(12)),
            Some(ResolvedAppServerRequest::McpElicitation {
                thread_id: thread_id(THREAD_1),
                server_name: "example".to_string(),
                request_id: AppServerRequestId::Integer(12),
            })
        );
    }

    #[test]
    fn resolve_notification_returns_resolved_user_input_item_id() {
        let mut pending = PendingAppServerRequests::default();
        pending.note_server_request(&user_input_request(
            THREAD_1, /*request_id*/ 8, "turn-1", "tool-1",
        ));

        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_1), &AppServerRequestId::Integer(8)),
            Some(ResolvedAppServerRequest::UserInput {
                thread_id: thread_id(THREAD_1),
                request_id: AppServerRequestId::Integer(8),
                turn_id: "turn-1".to_string(),
                call_id: "tool-1".to_string(),
            })
        );
    }

    #[test]
    fn same_turn_user_input_answers_resolve_exact_requests_out_of_order() {
        let mut pending = PendingAppServerRequests::default();
        for (request_id, item_id) in [(8, "tool-1"), (9, "tool-2")] {
            pending
                .note_server_request(&user_input_request(THREAD_1, request_id, "turn-1", item_id));
        }

        let response = ToolRequestUserInputResponse {
            answers: HashMap::new(),
        };
        let second_response = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(9),
                &Op::UserInputAnswer {
                    id: "turn-1".to_string(),
                    response: response.clone(),
                },
            )
            .expect("user input response should serialize")
            .expect("second user input request should be pending");
        let first_response = pending
            .take_resolution(
                thread_id(THREAD_1),
                &AppServerRequestId::Integer(8),
                &Op::UserInputAnswer {
                    id: "turn-1".to_string(),
                    response,
                },
            )
            .expect("user input response should serialize")
            .expect("first user input request should be pending");

        assert_eq!(first_response.request_id, AppServerRequestId::Integer(8));
        assert_eq!(second_response.request_id, AppServerRequestId::Integer(9));
    }

    #[test]
    fn equal_domain_ids_across_threads_resolve_independently() {
        let mut pending = PendingAppServerRequests::default();

        for request in [
            exec_request(
                THREAD_1,
                /*request_id*/ 1,
                "implicit-exec",
                /*approval_id*/ None,
            ),
            exec_request(
                THREAD_2,
                /*request_id*/ 2,
                "implicit-exec",
                /*approval_id*/ None,
            ),
            exec_request(
                THREAD_1,
                /*request_id*/ 3,
                "exec-1",
                Some("explicit-exec"),
            ),
            exec_request(
                THREAD_2,
                /*request_id*/ 4,
                "exec-2",
                Some("explicit-exec"),
            ),
            file_change_request(THREAD_1, /*request_id*/ 5, "patch"),
            file_change_request(THREAD_2, /*request_id*/ 6, "patch"),
            permissions_request(THREAD_1, /*request_id*/ 7, "permissions"),
            permissions_request(THREAD_2, /*request_id*/ 8, "permissions"),
            user_input_request(THREAD_1, /*request_id*/ 9, "turn", "input"),
            user_input_request(THREAD_2, /*request_id*/ 10, "turn", "input"),
        ] {
            assert_eq!(pending.note_server_request(&request), None);
        }

        let exec_op = |id: &str| Op::ExecApproval {
            id: id.to_string(),
            turn_id: None,
            decision: CommandExecutionApprovalDecision::Accept,
        };
        let patch_op = Op::PatchApproval {
            id: "patch".to_string(),
            decision: FileChangeApprovalDecision::Accept,
        };
        let permissions_op = Op::RequestPermissionsResponse {
            id: "permissions".to_string(),
            response: permissions_response(),
        };
        let user_input_op = Op::UserInputAnswer {
            id: "turn".to_string(),
            response: empty_user_input_response(),
        };

        let cases = [
            (THREAD_2, exec_op("implicit-exec"), 2),
            (THREAD_1, exec_op("implicit-exec"), 1),
            (THREAD_2, exec_op("explicit-exec"), 4),
            (THREAD_1, exec_op("explicit-exec"), 3),
            (THREAD_2, patch_op.clone(), 6),
            (THREAD_1, patch_op, 5),
            (THREAD_2, permissions_op.clone(), 8),
            (THREAD_1, permissions_op, 7),
            (THREAD_2, user_input_op.clone(), 10),
            (THREAD_1, user_input_op, 9),
        ];
        for (thread, op, expected_request_id) in cases {
            let expected_request_id = AppServerRequestId::Integer(expected_request_id);
            let resolution = pending
                .take_resolution(thread_id(thread), &expected_request_id, &op)
                .expect("resolution should serialize")
                .expect("thread-scoped request should remain pending");
            assert_eq!(resolution.request_id, expected_request_id);
        }
    }

    #[test]
    fn remote_resolution_requires_matching_thread_even_for_equal_request_ids() {
        let mut pending = PendingAppServerRequests::default();
        let request_id = AppServerRequestId::Integer(41);
        assert_eq!(
            pending.note_server_request(&exec_request(
                THREAD_1,
                /*request_id*/ 41,
                "call",
                Some("approval"),
            )),
            None
        );
        assert_eq!(
            pending.note_server_request(&exec_request(
                THREAD_2,
                /*request_id*/ 41,
                "call",
                Some("approval"),
            )),
            None
        );

        let resolved = pending
            .resolve_notification(thread_id(THREAD_2), &request_id)
            .expect("thread 2 request should resolve");
        assert_eq!(resolved.thread_id(), thread_id(THREAD_2));
        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_2), &request_id),
            None
        );
        let resolved = pending
            .resolve_notification(thread_id(THREAD_1), &request_id)
            .expect("thread 1 request should remain pending");
        assert_eq!(resolved.thread_id(), thread_id(THREAD_1));
    }

    #[test]
    fn duplicate_scoped_approval_keys_are_rejected_without_overwriting() {
        let mut pending = PendingAppServerRequests::default();
        let cases = [
            (
                file_change_request(THREAD_1, /*request_id*/ 1, "patch"),
                file_change_request(THREAD_1, /*request_id*/ 2, "patch"),
            ),
            (
                permissions_request(THREAD_1, /*request_id*/ 3, "permissions"),
                permissions_request(THREAD_1, /*request_id*/ 4, "permissions"),
            ),
        ];
        for (original, duplicate) in cases {
            assert_eq!(pending.note_server_request(&original), None);
            assert!(pending.note_server_request(&duplicate).is_some());
            assert!(pending.contains_server_request(&original));
            assert!(!pending.contains_server_request(&duplicate));
        }
    }

    #[test]
    fn duplicate_user_input_item_is_rejected_within_thread_and_turn_only() {
        let mut pending = PendingAppServerRequests::default();
        let original = user_input_request(THREAD_1, /*request_id*/ 1, "turn-1", "input");
        let duplicate = user_input_request(THREAD_1, /*request_id*/ 2, "turn-1", "input");
        let other_turn = user_input_request(THREAD_1, /*request_id*/ 3, "turn-2", "input");
        let other_thread = user_input_request(THREAD_2, /*request_id*/ 4, "turn-1", "input");

        assert_eq!(pending.note_server_request(&original), None);
        assert!(pending.note_server_request(&duplicate).is_some());
        assert_eq!(pending.note_server_request(&other_turn), None);
        assert_eq!(pending.note_server_request(&other_thread), None);
        assert!(pending.contains_server_request(&original));
        assert!(!pending.contains_server_request(&duplicate));
        assert!(pending.contains_server_request(&other_turn));
        assert!(pending.contains_server_request(&other_thread));

        assert_eq!(
            pending.resolve_notification(thread_id(THREAD_1), &AppServerRequestId::Integer(3)),
            Some(ResolvedAppServerRequest::UserInput {
                thread_id: thread_id(THREAD_1),
                request_id: AppServerRequestId::Integer(3),
                turn_id: "turn-2".to_string(),
                call_id: "input".to_string(),
            })
        );
        assert!(pending.contains_server_request(&original));
    }

    #[test]
    fn stale_exact_resolution_does_not_consume_same_key_replacement() {
        let cases = [
            (
                "implicit exec approval",
                exec_request(
                    THREAD_1,
                    /*request_id*/ 1,
                    "implicit-exec",
                    /*approval_id*/ None,
                ),
                exec_request(
                    THREAD_1,
                    /*request_id*/ 2,
                    "implicit-exec",
                    /*approval_id*/ None,
                ),
                Op::ExecApproval {
                    id: "implicit-exec".to_string(),
                    turn_id: None,
                    decision: CommandExecutionApprovalDecision::Accept,
                },
            ),
            (
                "explicit exec approval",
                exec_request(
                    THREAD_1,
                    /*request_id*/ 1,
                    "exec-a",
                    Some("explicit-exec"),
                ),
                exec_request(
                    THREAD_1,
                    /*request_id*/ 2,
                    "exec-b",
                    Some("explicit-exec"),
                ),
                Op::ExecApproval {
                    id: "explicit-exec".to_string(),
                    turn_id: None,
                    decision: CommandExecutionApprovalDecision::Accept,
                },
            ),
            (
                "file change approval",
                file_change_request(THREAD_1, /*request_id*/ 1, "patch"),
                file_change_request(THREAD_1, /*request_id*/ 2, "patch"),
                Op::PatchApproval {
                    id: "patch".to_string(),
                    decision: FileChangeApprovalDecision::Accept,
                },
            ),
            (
                "permissions approval",
                permissions_request(THREAD_1, /*request_id*/ 1, "permissions"),
                permissions_request(THREAD_1, /*request_id*/ 2, "permissions"),
                Op::RequestPermissionsResponse {
                    id: "permissions".to_string(),
                    response: permissions_response(),
                },
            ),
            (
                "user input",
                user_input_request(THREAD_1, /*request_id*/ 1, "turn", "input"),
                user_input_request(THREAD_1, /*request_id*/ 2, "turn", "input"),
                Op::UserInputAnswer {
                    id: "turn".to_string(),
                    response: empty_user_input_response(),
                },
            ),
        ];

        for (kind, original, replacement, op) in cases {
            let mut pending = PendingAppServerRequests::default();
            let original_request_id = AppServerRequestId::Integer(1);
            let replacement_request_id = AppServerRequestId::Integer(2);

            assert_eq!(
                pending.note_server_request(&original),
                None,
                "failed to register original {kind}"
            );
            let resolved = pending
                .resolve_notification(thread_id(THREAD_1), &original_request_id)
                .unwrap_or_else(|| panic!("original {kind} should resolve remotely"));
            assert_eq!(resolved.request_id(), &original_request_id);
            assert_eq!(
                pending.note_server_request(&replacement),
                None,
                "failed to register replacement {kind}"
            );

            assert_eq!(
                pending
                    .take_resolution(thread_id(THREAD_1), &original_request_id, &op)
                    .expect("stale resolution should not fail serialization"),
                None,
                "stale {kind} resolution consumed the replacement"
            );
            assert!(
                pending.contains_server_request(&replacement),
                "replacement {kind} should remain pending"
            );

            let resolution = pending
                .take_resolution(thread_id(THREAD_1), &replacement_request_id, &op)
                .expect("replacement resolution should serialize")
                .unwrap_or_else(|| panic!("replacement {kind} should resolve exactly"));
            assert_eq!(resolution.request_id, replacement_request_id);
        }
    }

    #[test]
    fn integer_and_string_request_ids_are_distinct_generations() {
        let mut pending = PendingAppServerRequests::default();
        let integer_request_id = AppServerRequestId::Integer(1);
        let string_request_id = AppServerRequestId::String("1".to_string());
        let original = exec_request_with_request_id(
            THREAD_1,
            integer_request_id.clone(),
            "call-a",
            Some("approval"),
        );
        let replacement = exec_request_with_request_id(
            THREAD_1,
            string_request_id.clone(),
            "call-b",
            Some("approval"),
        );
        let op = Op::ExecApproval {
            id: "approval".to_string(),
            turn_id: None,
            decision: CommandExecutionApprovalDecision::Accept,
        };

        assert_eq!(pending.note_server_request(&original), None);
        assert!(
            pending
                .resolve_notification(thread_id(THREAD_1), &integer_request_id)
                .is_some()
        );
        assert_eq!(pending.note_server_request(&replacement), None);
        assert_eq!(
            pending
                .take_resolution(thread_id(THREAD_1), &integer_request_id, &op)
                .expect("stale integer resolution should not fail"),
            None
        );
        assert!(pending.contains_server_request(&replacement));
        assert_eq!(
            pending
                .take_resolution(thread_id(THREAD_1), &string_request_id, &op)
                .expect("string resolution should serialize")
                .expect("string request should resolve")
                .request_id,
            string_request_id
        );
    }

    #[test]
    fn alternate_uuid_text_encoding_correlates_by_typed_thread_id() {
        let mut pending = PendingAppServerRequests::default();
        let request_id = AppServerRequestId::Integer(17);
        let request = exec_request_with_request_id(
            "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
            request_id.clone(),
            "call",
            Some("approval"),
        );
        let normalized_thread_id = thread_id("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let op = Op::ExecApproval {
            id: "approval".to_string(),
            turn_id: None,
            decision: CommandExecutionApprovalDecision::Accept,
        };

        assert_eq!(pending.note_server_request(&request), None);
        assert_eq!(
            pending
                .take_resolution(normalized_thread_id, &request_id, &op)
                .expect("resolution should serialize")
                .expect("typed thread identity should match alternate text encoding")
                .request_id,
            request_id
        );
    }

    #[test]
    fn equal_user_input_item_ids_across_turns_resolve_independently() {
        let mut pending = PendingAppServerRequests::default();
        let first = user_input_request(THREAD_1, /*request_id*/ 8, "turn-1", "input");
        let second = user_input_request(THREAD_1, /*request_id*/ 9, "turn-2", "input");

        assert_eq!(pending.note_server_request(&first), None);
        assert_eq!(pending.note_server_request(&second), None);

        for (request_id, turn_id, request) in [(9, "turn-2", &second), (8, "turn-1", &first)] {
            let request_id = AppServerRequestId::Integer(request_id);
            let resolution = pending
                .take_resolution(
                    thread_id(THREAD_1),
                    &request_id,
                    &Op::UserInputAnswer {
                        id: turn_id.to_string(),
                        response: empty_user_input_response(),
                    },
                )
                .expect("user input resolution should serialize")
                .expect("turn-scoped user input should resolve");
            assert_eq!(resolution.request_id, request_id);
            assert!(!pending.contains_server_request(request));
        }
    }

    #[test]
    fn invalid_thread_id_is_rejected_before_pending_state_changes() {
        let mut pending = PendingAppServerRequests::default();
        let invalid = exec_request(
            "not-a-thread-id",
            /*request_id*/ 1,
            "call",
            Some("approval"),
        );
        let valid = exec_request(THREAD_1, /*request_id*/ 2, "call", Some("approval"));

        let unsupported = pending
            .note_server_request(&invalid)
            .expect("invalid thread should be rejected");
        assert_eq!(unsupported.request_id, AppServerRequestId::Integer(1));
        assert!(
            unsupported
                .message
                .contains("invalid app-server request thread_id")
        );
        assert!(!pending.contains_server_request(&invalid));
        assert_eq!(pending.note_server_request(&valid), None);
        assert!(pending.contains_server_request(&valid));
    }
}
