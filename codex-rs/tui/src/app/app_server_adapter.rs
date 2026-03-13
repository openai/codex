use super::App;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::GrantedMacOsPermissions;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_core::AuthManager;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum PendingEmbeddedAppServerRequestKey {
    Exec {
        thread_id: String,
        approval_id: String,
    },
    Patch {
        thread_id: String,
        item_id: String,
    },
    Permissions {
        thread_id: String,
        item_id: String,
    },
    UserInput {
        thread_id: String,
        turn_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PendingEmbeddedAppServerRequest {
    Exec {
        request_id: RequestId,
        kind: PendingEmbeddedExecRequestKind,
    },
    Patch {
        request_id: RequestId,
        kind: PendingEmbeddedPatchRequestKind,
    },
    Permissions {
        request_id: RequestId,
    },
    UserInput {
        request_id: RequestId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingEmbeddedExecRequestKind {
    Legacy,
    V2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingEmbeddedPatchRequestKind {
    Legacy,
    V2,
}

impl App {
    pub(super) async fn handle_embedded_app_server_event(&mut self, event: InProcessServerEvent) {
        match event {
            InProcessServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "embedded app-server event consumer lagged; dropping ignored events"
                );
            }
            InProcessServerEvent::ServerNotification(notification) => {
                self.handle_ignored_embedded_app_server_notification(notification);
            }
            InProcessServerEvent::LegacyNotification(notification) => {
                self.handle_ignored_embedded_app_server_legacy_notification(notification);
            }
            InProcessServerEvent::ServerRequest(request) => {
                self.handle_embedded_app_server_request(request).await;
            }
        }
    }

    pub(super) async fn resolve_embedded_app_server_request_for_submitted_op(
        &mut self,
        thread_id: ThreadId,
        op: &Op,
    ) -> std::result::Result<(), String> {
        let Some(client) = self.embedded_app_server_client.as_ref() else {
            return Ok(());
        };
        let Some(key) = pending_embedded_app_server_request_key_for_op(thread_id, op) else {
            return Ok(());
        };
        let Some(pending_request) = self.pending_embedded_app_server_requests.remove(&key) else {
            return Ok(());
        };
        let request_id = pending_request.request_id().clone();
        let value = embedded_app_server_response_value(&pending_request, op)?;
        client
            .resolve_server_request(request_id.clone(), value)
            .await
            .map_err(|err| {
                format!(
                    "failed to resolve embedded app-server request `{request_id:?}` for `{key:?}`: {err}"
                )
            })
    }

    async fn handle_embedded_app_server_request(&mut self, request: ServerRequest) {
        let method = Self::server_request_method_name(&request);
        match request {
            ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
                let refresh_result = tokio::task::spawn_blocking({
                    let codex_home = self.config.codex_home.clone();
                    let auth_credentials_store_mode = self.config.cli_auth_credentials_store_mode;
                    let forced_chatgpt_workspace_id =
                        self.config.forced_chatgpt_workspace_id.clone();
                    move || {
                        Self::local_external_chatgpt_tokens(
                            codex_home,
                            auth_credentials_store_mode,
                            forced_chatgpt_workspace_id,
                        )
                    }
                })
                .await;

                let handle_result = match refresh_result {
                    Err(err) => {
                        self.reject_embedded_app_server_request(
                            request_id,
                            &method,
                            format!("local chatgpt auth refresh task failed in TUI: {err}"),
                        )
                        .await
                    }
                    Ok(Err(reason)) => {
                        self.reject_embedded_app_server_request(request_id, &method, reason)
                            .await
                    }
                    Ok(Ok(response)) => {
                        if let Some(previous_account_id) = params.previous_account_id.as_deref()
                            && previous_account_id != response.chatgpt_account_id
                        {
                            tracing::warn!(
                                "local auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
                                response.chatgpt_account_id
                            );
                        }
                        match serde_json::to_value(response) {
                            Ok(value) => {
                                self.resolve_embedded_app_server_request(
                                    request_id,
                                    value,
                                    "account/chatgptAuthTokens/refresh",
                                )
                                .await
                            }
                            Err(err) => Err(format!(
                                "failed to serialize chatgpt auth refresh response: {err}"
                            )),
                        }
                    }
                };
                if let Err(err) = handle_result {
                    tracing::warn!("{err}");
                }
            }
            request => {
                if self.remember_pending_embedded_app_server_request(&request) {
                    return;
                }
                let request_id = request.id().clone();
                tracing::warn!(
                    ?request_id,
                    method,
                    "rejecting unsupported embedded app-server request while TUI still uses direct core APIs"
                );
                if let Err(err) = self
                    .reject_embedded_app_server_request(
                        request_id,
                        &method,
                        "embedded TUI client does not yet handle this app-server server request"
                            .to_string(),
                    )
                    .await
                {
                    tracing::warn!("{err}");
                }
            }
        }
    }

    fn remember_pending_embedded_app_server_request(&mut self, request: &ServerRequest) -> bool {
        let Some((key, pending_request)) = pending_embedded_app_server_request(request) else {
            return false;
        };
        self.pending_embedded_app_server_requests
            .insert(key, pending_request);
        true
    }

    async fn resolve_embedded_app_server_request(
        &self,
        request_id: RequestId,
        value: serde_json::Value,
        method: &str,
    ) -> std::result::Result<(), String> {
        let Some(client) = self.embedded_app_server_client.as_ref() else {
            return Err(format!(
                "failed to resolve `{method}` server request: embedded app-server client unavailable"
            ));
        };
        client
            .resolve_server_request(request_id, value)
            .await
            .map_err(|err| format!("failed to resolve `{method}` server request: {err}"))
    }

    async fn reject_embedded_app_server_request(
        &self,
        request_id: RequestId,
        method: &str,
        reason: String,
    ) -> std::result::Result<(), String> {
        let Some(client) = self.embedded_app_server_client.as_ref() else {
            return Err(format!(
                "failed to reject `{method}` server request: embedded app-server client unavailable"
            ));
        };
        client
            .reject_server_request(
                request_id,
                JSONRPCErrorError {
                    code: -32000,
                    message: reason,
                    data: None,
                },
            )
            .await
            .map_err(|err| format!("failed to reject `{method}` server request: {err}"))
    }

    fn handle_ignored_embedded_app_server_notification(
        &mut self,
        _notification: ServerNotification,
    ) {
    }

    fn handle_ignored_embedded_app_server_legacy_notification(
        &mut self,
        _notification: JSONRPCNotification,
    ) {
    }

    fn server_request_method_name(request: &ServerRequest) -> String {
        serde_json::to_value(request)
            .ok()
            .and_then(|value| {
                value
                    .get("method")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn local_external_chatgpt_tokens(
        codex_home: PathBuf,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        forced_chatgpt_workspace_id: Option<String>,
    ) -> Result<ChatgptAuthTokensRefreshResponse, String> {
        let auth_manager = AuthManager::shared(codex_home, false, auth_credentials_store_mode);
        auth_manager.set_forced_chatgpt_workspace_id(forced_chatgpt_workspace_id);
        auth_manager.reload();

        let auth = auth_manager
            .auth_cached()
            .ok_or_else(|| "no cached auth available for local token refresh".to_string())?;
        if !auth.is_external_chatgpt_tokens() {
            return Err("external ChatGPT token auth is not active".to_string());
        }

        let access_token = auth
            .get_token()
            .map_err(|err| format!("failed to read external access token: {err}"))?;
        let chatgpt_account_id = auth
            .get_account_id()
            .ok_or_else(|| "external token auth is missing chatgpt account id".to_string())?;
        let chatgpt_plan_type = auth.account_plan_type().map(|plan_type| match plan_type {
            AccountPlanType::Free => "free".to_string(),
            AccountPlanType::Go => "go".to_string(),
            AccountPlanType::Plus => "plus".to_string(),
            AccountPlanType::Pro => "pro".to_string(),
            AccountPlanType::Team => "team".to_string(),
            AccountPlanType::Business => "business".to_string(),
            AccountPlanType::Enterprise => "enterprise".to_string(),
            AccountPlanType::Edu => "edu".to_string(),
            AccountPlanType::Unknown => "unknown".to_string(),
        });

        Ok(ChatgptAuthTokensRefreshResponse {
            access_token,
            chatgpt_account_id,
            chatgpt_plan_type,
        })
    }
}

impl PendingEmbeddedAppServerRequest {
    fn request_id(&self) -> &RequestId {
        match self {
            PendingEmbeddedAppServerRequest::Exec { request_id, .. }
            | PendingEmbeddedAppServerRequest::Patch { request_id, .. }
            | PendingEmbeddedAppServerRequest::Permissions { request_id }
            | PendingEmbeddedAppServerRequest::UserInput { request_id } => request_id,
        }
    }
}

fn pending_embedded_app_server_request(
    request: &ServerRequest,
) -> Option<(
    PendingEmbeddedAppServerRequestKey,
    PendingEmbeddedAppServerRequest,
)> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::Exec {
                thread_id: params.thread_id.clone(),
                approval_id: params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone()),
            },
            PendingEmbeddedAppServerRequest::Exec {
                request_id: request_id.clone(),
                kind: PendingEmbeddedExecRequestKind::V2,
            },
        )),
        ServerRequest::ExecCommandApproval { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::Exec {
                thread_id: params.conversation_id.to_string(),
                approval_id: params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.call_id.clone()),
            },
            PendingEmbeddedAppServerRequest::Exec {
                request_id: request_id.clone(),
                kind: PendingEmbeddedExecRequestKind::Legacy,
            },
        )),
        ServerRequest::FileChangeRequestApproval { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::Patch {
                thread_id: params.thread_id.clone(),
                item_id: params.item_id.clone(),
            },
            PendingEmbeddedAppServerRequest::Patch {
                request_id: request_id.clone(),
                kind: PendingEmbeddedPatchRequestKind::V2,
            },
        )),
        ServerRequest::ApplyPatchApproval { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::Patch {
                thread_id: params.conversation_id.to_string(),
                item_id: params.call_id.clone(),
            },
            PendingEmbeddedAppServerRequest::Patch {
                request_id: request_id.clone(),
                kind: PendingEmbeddedPatchRequestKind::Legacy,
            },
        )),
        ServerRequest::PermissionsRequestApproval { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::Permissions {
                thread_id: params.thread_id.clone(),
                item_id: params.item_id.clone(),
            },
            PendingEmbeddedAppServerRequest::Permissions {
                request_id: request_id.clone(),
            },
        )),
        ServerRequest::ToolRequestUserInput { request_id, params } => Some((
            PendingEmbeddedAppServerRequestKey::UserInput {
                thread_id: params.thread_id.clone(),
                turn_id: params.turn_id.clone(),
            },
            PendingEmbeddedAppServerRequest::UserInput {
                request_id: request_id.clone(),
            },
        )),
        ServerRequest::ChatgptAuthTokensRefresh { .. }
        | ServerRequest::McpServerElicitationRequest { .. }
        | ServerRequest::DynamicToolCall { .. } => None,
    }
}

fn pending_embedded_app_server_request_key_for_op(
    thread_id: ThreadId,
    op: &Op,
) -> Option<PendingEmbeddedAppServerRequestKey> {
    let thread_id = thread_id.to_string();
    match op {
        Op::ExecApproval { id, .. } => Some(PendingEmbeddedAppServerRequestKey::Exec {
            thread_id,
            approval_id: id.clone(),
        }),
        Op::PatchApproval { id, .. } => Some(PendingEmbeddedAppServerRequestKey::Patch {
            thread_id,
            item_id: id.clone(),
        }),
        Op::RequestPermissionsResponse { id, .. } => {
            Some(PendingEmbeddedAppServerRequestKey::Permissions {
                thread_id,
                item_id: id.clone(),
            })
        }
        Op::UserInputAnswer { id, .. } => Some(PendingEmbeddedAppServerRequestKey::UserInput {
            thread_id,
            turn_id: id.clone(),
        }),
        _ => None,
    }
}

fn embedded_app_server_response_value(
    pending_request: &PendingEmbeddedAppServerRequest,
    op: &Op,
) -> std::result::Result<serde_json::Value, String> {
    match (pending_request, op) {
        (
            PendingEmbeddedAppServerRequest::Exec {
                kind: PendingEmbeddedExecRequestKind::Legacy,
                ..
            },
            Op::ExecApproval { decision, .. },
        ) => serde_json::to_value(ExecCommandApprovalResponse {
            decision: decision.clone(),
        })
        .map_err(|err| format!("failed to serialize legacy exec approval response: {err}")),
        (
            PendingEmbeddedAppServerRequest::Exec {
                kind: PendingEmbeddedExecRequestKind::V2,
                ..
            },
            Op::ExecApproval { decision, .. },
        ) => serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: decision.clone().into(),
        })
        .map_err(|err| format!("failed to serialize v2 exec approval response: {err}")),
        (
            PendingEmbeddedAppServerRequest::Patch {
                kind: PendingEmbeddedPatchRequestKind::Legacy,
                ..
            },
            Op::PatchApproval { decision, .. },
        ) => serde_json::to_value(ApplyPatchApprovalResponse {
            decision: decision.clone(),
        })
        .map_err(|err| format!("failed to serialize legacy patch approval response: {err}")),
        (
            PendingEmbeddedAppServerRequest::Patch {
                kind: PendingEmbeddedPatchRequestKind::V2,
                ..
            },
            Op::PatchApproval { decision, .. },
        ) => serde_json::to_value(FileChangeRequestApprovalResponse {
            decision: file_change_approval_decision_from_review_decision(decision)?,
        })
        .map_err(|err| format!("failed to serialize v2 patch approval response: {err}")),
        (
            PendingEmbeddedAppServerRequest::Permissions { .. },
            Op::RequestPermissionsResponse { response, .. },
        ) => serde_json::to_value(PermissionsRequestApprovalResponse {
            permissions: granted_permission_profile_from_core(response.permissions.clone()),
            scope: response.scope.into(),
        })
        .map_err(|err| format!("failed to serialize permissions approval response: {err}")),
        (
            PendingEmbeddedAppServerRequest::UserInput { .. },
            Op::UserInputAnswer { response, .. },
        ) => {
            let answers = response
                .answers
                .iter()
                .map(|(id, answer)| {
                    (
                        id.clone(),
                        ToolRequestUserInputAnswer {
                            answers: answer.answers.clone(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            serde_json::to_value(ToolRequestUserInputResponse { answers }).map_err(|err| {
                format!("failed to serialize tool request_user_input response: {err}")
            })
        }
        _ => Err(format!(
            "cannot resolve embedded app-server request `{pending_request:?}` from op `{op:?}`"
        )),
    }
}

fn granted_permission_profile_from_core(
    permissions: codex_protocol::models::PermissionProfile,
) -> GrantedPermissionProfile {
    GrantedPermissionProfile {
        network: permissions.network.map(Into::into),
        file_system: permissions.file_system.map(Into::into),
        macos: permissions.macos.map(|macos| GrantedMacOsPermissions {
            preferences: (macos.macos_preferences
                != codex_protocol::models::MacOsPreferencesPermission::None)
                .then_some(macos.macos_preferences),
            automations: (macos.macos_automation
                != codex_protocol::models::MacOsAutomationPermission::None)
                .then_some(macos.macos_automation),
            launch_services: macos.macos_launch_services.then_some(true),
            accessibility: macos.macos_accessibility.then_some(true),
            calendar: macos.macos_calendar.then_some(true),
            reminders: macos.macos_reminders.then_some(true),
            contacts: (macos.macos_contacts
                != codex_protocol::models::MacOsContactsPermission::None)
                .then_some(macos.macos_contacts),
        }),
    }
}

fn file_change_approval_decision_from_review_decision(
    decision: &ReviewDecision,
) -> std::result::Result<FileChangeApprovalDecision, String> {
    match decision {
        ReviewDecision::Approved => Ok(FileChangeApprovalDecision::Accept),
        ReviewDecision::ApprovedForSession => Ok(FileChangeApprovalDecision::AcceptForSession),
        ReviewDecision::Denied => Ok(FileChangeApprovalDecision::Decline),
        ReviewDecision::Abort => Ok(FileChangeApprovalDecision::Cancel),
        ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => Err(format!(
            "unsupported patch approval decision for app-server response: {decision:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::CommandExecutionApprovalDecision;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::PermissionsRequestApprovalParams;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::protocol::Op;
    use codex_protocol::request_permissions::PermissionGrantScope;
    use codex_protocol::request_permissions::RequestPermissionsResponse;
    use codex_protocol::request_user_input::RequestUserInputAnswer;
    use codex_protocol::request_user_input::RequestUserInputResponse;
    use pretty_assertions::assert_eq;

    #[test]
    fn v2_exec_request_tracks_effective_approval_id_and_serializes_response() {
        let thread_id = ThreadId::new();
        let request = ServerRequest::CommandExecutionRequestApproval {
            request_id: RequestId::Integer(7),
            params: CommandExecutionRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                approval_id: Some("approval-1".to_string()),
                reason: None,
                network_approval_context: None,
                command: Some("ls".to_string()),
                cwd: Some(PathBuf::from("/tmp")),
                command_actions: None,
                additional_permissions: None,
                skill_metadata: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        };

        let (key, pending_request) = pending_embedded_app_server_request(&request).unwrap();
        assert_eq!(
            key,
            PendingEmbeddedAppServerRequestKey::Exec {
                thread_id: thread_id.to_string(),
                approval_id: "approval-1".to_string(),
            }
        );

        let response = embedded_app_server_response_value(
            &pending_request,
            &Op::ExecApproval {
                id: "approval-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                decision: ReviewDecision::ApprovedForSession,
            },
        )
        .unwrap();

        let response: CommandExecutionRequestApprovalResponse =
            serde_json::from_value(response).unwrap();
        assert_eq!(
            response,
            CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::AcceptForSession,
            }
        );
    }

    #[test]
    fn v2_patch_request_tracks_item_id_and_serializes_response() {
        let thread_id = ThreadId::new();
        let request = ServerRequest::FileChangeRequestApproval {
            request_id: RequestId::Integer(8),
            params: FileChangeRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-2".to_string(),
                item_id: "patch-1".to_string(),
                reason: None,
                grant_root: None,
            },
        };

        let (key, pending_request) = pending_embedded_app_server_request(&request).unwrap();
        assert_eq!(
            key,
            PendingEmbeddedAppServerRequestKey::Patch {
                thread_id: thread_id.to_string(),
                item_id: "patch-1".to_string(),
            }
        );

        let response = embedded_app_server_response_value(
            &pending_request,
            &Op::PatchApproval {
                id: "patch-1".to_string(),
                decision: ReviewDecision::ApprovedForSession,
            },
        )
        .unwrap();

        let response: FileChangeRequestApprovalResponse = serde_json::from_value(response).unwrap();
        assert_eq!(
            response,
            FileChangeRequestApprovalResponse {
                decision: FileChangeApprovalDecision::AcceptForSession,
            }
        );
    }

    #[test]
    fn request_permissions_response_serializes_granted_permissions() {
        let thread_id = ThreadId::new();
        let request = ServerRequest::PermissionsRequestApproval {
            request_id: RequestId::Integer(9),
            params: PermissionsRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-3".to_string(),
                item_id: "permissions-1".to_string(),
                reason: Some("Need permissions".to_string()),
                permissions: PermissionProfile::default().into(),
            },
        };

        let (_, pending_request) = pending_embedded_app_server_request(&request).unwrap();
        let response = embedded_app_server_response_value(
            &pending_request,
            &Op::RequestPermissionsResponse {
                id: "permissions-1".to_string(),
                response: RequestPermissionsResponse {
                    permissions: PermissionProfile::default(),
                    scope: PermissionGrantScope::Session,
                },
            },
        )
        .unwrap();

        let response: PermissionsRequestApprovalResponse =
            serde_json::from_value(response).unwrap();
        assert_eq!(
            codex_protocol::models::PermissionProfile::from(response.permissions.clone()),
            PermissionProfile::default()
        );
        assert_eq!(
            response.scope,
            codex_app_server_protocol::PermissionGrantScope::Session
        );
    }

    #[test]
    fn request_user_input_response_serializes_answers() {
        let thread_id = ThreadId::new();
        let request = ServerRequest::ToolRequestUserInput {
            request_id: RequestId::Integer(10),
            params: ToolRequestUserInputParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-4".to_string(),
                item_id: "tool-call-1".to_string(),
                questions: Vec::new(),
            },
        };

        let (_, pending_request) = pending_embedded_app_server_request(&request).unwrap();
        let response = embedded_app_server_response_value(
            &pending_request,
            &Op::UserInputAnswer {
                id: "turn-4".to_string(),
                response: RequestUserInputResponse {
                    answers: [(
                        "question-1".to_string(),
                        RequestUserInputAnswer {
                            answers: vec!["answer".to_string(), "notes".to_string()],
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            },
        )
        .unwrap();

        let response: ToolRequestUserInputResponse = serde_json::from_value(response).unwrap();
        assert_eq!(
            response,
            ToolRequestUserInputResponse {
                answers: [(
                    "question-1".to_string(),
                    ToolRequestUserInputAnswer {
                        answers: vec!["answer".to_string(), "notes".to_string()],
                    },
                )]
                .into_iter()
                .collect(),
            }
        );
    }
}
