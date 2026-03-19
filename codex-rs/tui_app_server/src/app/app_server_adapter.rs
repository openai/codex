use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::app_server_rate_limit_snapshot_to_core;
use crate::app_server_session::status_account_display_from_auth_mode;
use crate::local_chatgpt_auth::load_local_chatgpt_auth;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ChatgptAuthTokensRefreshParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_protocol::ThreadId;
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
enum LegacyThreadNotification {
    Warning(String),
    Rollback { num_turns: u32 },
}

impl App {
    pub(super) async fn handle_app_server_event(
        &mut self,
        app_server_client: &AppServerSession,
        event: AppServerEvent,
    ) {
        match event {
            AppServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "app-server event consumer lagged; dropping ignored events"
                );
            }
            AppServerEvent::ServerNotification(notification) => {
                self.handle_server_notification_event(app_server_client, notification)
                    .await;
            }
            AppServerEvent::LegacyNotification(notification) => {
                if let Some((thread_id, legacy_notification)) =
                    legacy_thread_notification(notification)
                {
                    let result = match legacy_notification {
                        LegacyThreadNotification::Warning(message) => {
                            if self.primary_thread_id == Some(thread_id)
                                || self.primary_thread_id.is_none()
                            {
                                self.enqueue_primary_thread_legacy_warning(message).await
                            } else {
                                self.enqueue_thread_legacy_warning(thread_id, message).await
                            }
                        }
                        LegacyThreadNotification::Rollback { num_turns } => {
                            if self.primary_thread_id == Some(thread_id)
                                || self.primary_thread_id.is_none()
                            {
                                self.enqueue_primary_thread_legacy_rollback(num_turns).await
                            } else {
                                self.enqueue_thread_legacy_rollback(thread_id, num_turns)
                                    .await
                            }
                        }
                    };
                    if let Err(err) = result {
                        tracing::warn!("failed to enqueue app-server legacy notification: {err}");
                    }
                } else {
                    tracing::debug!("ignoring legacy app-server notification in tui_app_server");
                }
            }
            AppServerEvent::ServerRequest(request) => {
                if let ServerRequest::ChatgptAuthTokensRefresh { request_id, params } = request {
                    self.handle_chatgpt_auth_tokens_refresh_request(
                        app_server_client,
                        request_id,
                        params,
                    )
                    .await;
                    return;
                }
                self.handle_server_request_event(app_server_client, request)
                    .await;
            }
            AppServerEvent::Disconnected { message } => {
                tracing::warn!("app-server event stream disconnected: {message}");
                self.chat_widget.add_error_message(message.clone());
                self.app_event_tx.send(AppEvent::FatalExitRequest(message));
            }
        }
    }

    async fn handle_server_notification_event(
        &mut self,
        _app_server_client: &AppServerSession,
        notification: ServerNotification,
    ) {
        match &notification {
            ServerNotification::ServerRequestResolved(notification) => {
                if let Some(request) = self
                    .pending_app_server_requests
                    .resolve_notification(&notification.request_id)
                {
                    self.chat_widget.dismiss_app_server_request(&request);
                }
            }
            ServerNotification::AccountRateLimitsUpdated(notification) => {
                self.chat_widget.on_rate_limit_snapshot(Some(
                    app_server_rate_limit_snapshot_to_core(notification.rate_limits.clone()),
                ));
                return;
            }
            ServerNotification::AccountUpdated(notification) => {
                self.chat_widget.update_account_state(
                    status_account_display_from_auth_mode(
                        notification.auth_mode,
                        notification.plan_type,
                    ),
                    notification.plan_type,
                    matches!(
                        notification.auth_mode,
                        Some(AuthMode::Chatgpt) | Some(AuthMode::ChatgptAuthTokens)
                    ),
                );
                return;
            }
            _ => {}
        }

        match server_notification_thread_target(&notification) {
            ServerNotificationThreadTarget::Thread(thread_id) => {
                let result = if self.primary_thread_id == Some(thread_id)
                    || self.primary_thread_id.is_none()
                {
                    self.enqueue_primary_thread_notification(notification).await
                } else {
                    self.enqueue_thread_notification(thread_id, notification)
                        .await
                };

                if let Err(err) = result {
                    tracing::warn!("failed to enqueue app-server notification: {err}");
                }
                return;
            }
            ServerNotificationThreadTarget::InvalidThreadId(thread_id) => {
                tracing::warn!(
                    thread_id,
                    "ignoring app-server notification with invalid thread_id"
                );
                return;
            }
            ServerNotificationThreadTarget::Global => {}
        }

        self.chat_widget
            .handle_server_notification(notification, /*replay_kind*/ None);
    }

    async fn handle_server_request_event(
        &mut self,
        app_server_client: &AppServerSession,
        request: ServerRequest,
    ) {
        if let Some(unsupported) = self
            .pending_app_server_requests
            .note_server_request(&request)
        {
            tracing::warn!(
                request_id = ?unsupported.request_id,
                message = unsupported.message,
                "rejecting unsupported app-server request"
            );
            self.chat_widget
                .add_error_message(unsupported.message.clone());
            if let Err(err) = self
                .reject_app_server_request(
                    app_server_client,
                    unsupported.request_id,
                    unsupported.message,
                )
                .await
            {
                tracing::warn!("{err}");
            }
            return;
        }

        let Some(thread_id) = server_request_thread_id(&request) else {
            tracing::warn!("ignoring threadless app-server request");
            return;
        };

        let result =
            if self.primary_thread_id == Some(thread_id) || self.primary_thread_id.is_none() {
                self.enqueue_primary_thread_request(request).await
            } else {
                self.enqueue_thread_request(thread_id, request).await
            };
        if let Err(err) = result {
            tracing::warn!("failed to enqueue app-server request: {err}");
        }
    }

    async fn handle_chatgpt_auth_tokens_refresh_request(
        &mut self,
        app_server_client: &AppServerSession,
        request_id: RequestId,
        params: ChatgptAuthTokensRefreshParams,
    ) {
        let config = self.config.clone();
        let result = tokio::task::spawn_blocking(move || {
            resolve_chatgpt_auth_tokens_refresh_response(
                &config.codex_home,
                config.cli_auth_credentials_store_mode,
                config.forced_chatgpt_workspace_id.as_deref(),
                &params,
            )
        })
        .await;

        match result {
            Ok(Ok(response)) => {
                let response = serde_json::to_value(response).map_err(|err| {
                    format!("failed to serialize chatgpt auth refresh response: {err}")
                });
                match response {
                    Ok(response) => {
                        if let Err(err) = app_server_client
                            .resolve_server_request(request_id, response)
                            .await
                        {
                            tracing::warn!("failed to resolve chatgpt auth refresh request: {err}");
                        }
                    }
                    Err(err) => {
                        self.chat_widget.add_error_message(err.clone());
                        if let Err(reject_err) = self
                            .reject_app_server_request(app_server_client, request_id, err)
                            .await
                        {
                            tracing::warn!("{reject_err}");
                        }
                    }
                }
            }
            Ok(Err(err)) => {
                self.chat_widget.add_error_message(err.clone());
                if let Err(reject_err) = self
                    .reject_app_server_request(app_server_client, request_id, err)
                    .await
                {
                    tracing::warn!("{reject_err}");
                }
            }
            Err(err) => {
                let message = format!("chatgpt auth refresh task failed: {err}");
                self.chat_widget.add_error_message(message.clone());
                if let Err(reject_err) = self
                    .reject_app_server_request(app_server_client, request_id, message)
                    .await
                {
                    tracing::warn!("{reject_err}");
                }
            }
        }
    }

    async fn reject_app_server_request(
        &self,
        app_server_client: &AppServerSession,
        request_id: RequestId,
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

fn resolve_chatgpt_auth_tokens_refresh_response(
    codex_home: &std::path::Path,
    auth_credentials_store_mode: codex_core::auth::AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&str>,
    params: &ChatgptAuthTokensRefreshParams,
) -> Result<codex_app_server_protocol::ChatgptAuthTokensRefreshResponse, String> {
    let auth = load_local_chatgpt_auth(
        codex_home,
        auth_credentials_store_mode,
        forced_chatgpt_workspace_id,
    )?;
    if let Some(previous_account_id) = params.previous_account_id.as_deref()
        && previous_account_id != auth.chatgpt_account_id
    {
        return Err(format!(
            "local ChatGPT auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
            auth.chatgpt_account_id
        ));
    }
    Ok(auth.to_refresh_response())
}

fn server_request_thread_id(request: &ServerRequest) -> Option<ThreadId> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::FileChangeRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ToolRequestUserInput { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::McpServerElicitationRequest { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::PermissionsRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::DynamicToolCall { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ChatgptAuthTokensRefresh { .. }
        | ServerRequest::ApplyPatchApproval { .. }
        | ServerRequest::ExecCommandApproval { .. } => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ServerNotificationThreadTarget {
    Thread(ThreadId),
    InvalidThreadId(String),
    Global,
}

fn server_notification_thread_target(
    notification: &ServerNotification,
) -> ServerNotificationThreadTarget {
    let thread_id = match notification {
        ServerNotification::Error(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadStarted(notification) => Some(notification.thread.id.as_str()),
        ServerNotification::ThreadStatusChanged(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadArchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadUnarchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadClosed(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadNameUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TurnStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnDiffUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnPlanUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::RawResponseItemCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::AgentMessageDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::PlanDelta(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::CommandExecutionOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TerminalInteraction(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::FileChangeOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ServerRequestResolved(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::McpToolCallProgress(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryPartAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ContextCompacted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ModelRerouted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadRealtimeStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeItemAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeError(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeClosed(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::SkillsChanged(_)
        | ServerNotification::McpServerOauthLoginCompleted(_)
        | ServerNotification::AccountUpdated(_)
        | ServerNotification::AccountRateLimitsUpdated(_)
        | ServerNotification::AppListUpdated(_)
        | ServerNotification::DeprecationNotice(_)
        | ServerNotification::ConfigWarning(_)
        | ServerNotification::FuzzyFileSearchSessionUpdated(_)
        | ServerNotification::FuzzyFileSearchSessionCompleted(_)
        | ServerNotification::CommandExecOutputDelta(_)
        | ServerNotification::WindowsWorldWritableWarning(_)
        | ServerNotification::WindowsSandboxSetupCompleted(_)
        | ServerNotification::AccountLoginCompleted(_) => None,
    };

    match thread_id {
        Some(thread_id) => match ThreadId::from_string(thread_id) {
            Ok(thread_id) => ServerNotificationThreadTarget::Thread(thread_id),
            Err(_) => ServerNotificationThreadTarget::InvalidThreadId(thread_id.to_string()),
        },
        None => ServerNotificationThreadTarget::Global,
    }
}

fn legacy_thread_notification(
    notification: JSONRPCNotification,
) -> Option<(ThreadId, LegacyThreadNotification)> {
    let method = notification
        .method
        .strip_prefix("codex/event/")
        .unwrap_or(&notification.method);

    let Value::Object(mut params) = notification.params? else {
        return None;
    };
    let thread_id = params
        .remove("conversationId")
        .and_then(|value| serde_json::from_value::<String>(value).ok())
        .and_then(|value| ThreadId::from_string(&value).ok())?;
    let msg = params.get("msg").and_then(Value::as_object)?;

    match method {
        "warning" => {
            let message = msg
                .get("type")
                .and_then(Value::as_str)
                .zip(msg.get("message"))
                .and_then(|(kind, message)| (kind == "warning").then_some(message))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)?;
            Some((thread_id, LegacyThreadNotification::Warning(message)))
        }
        "thread_rolled_back" => {
            let num_turns = msg
                .get("type")
                .and_then(Value::as_str)
                .zip(msg.get("num_turns"))
                .and_then(|(kind, num_turns)| (kind == "thread_rolled_back").then_some(num_turns))
                .and_then(Value::as_u64)
                .and_then(|num_turns| u32::try_from(num_turns).ok())?;
            Some((thread_id, LegacyThreadNotification::Rollback { num_turns }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::LegacyThreadNotification;
    use super::ServerNotificationThreadTarget;
    use super::legacy_thread_notification;
    use super::server_notification_thread_target;
    use codex_app_server_protocol::JSONRPCNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnStartedNotification;
    use codex_app_server_protocol::TurnStatus;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn legacy_warning_notification_extracts_thread_id_and_message() {
        let thread_id = ThreadId::new();
        let warning = legacy_thread_notification(JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(json!({
                "conversationId": thread_id.to_string(),
                "id": "event-1",
                "msg": {
                    "type": "warning",
                    "message": "legacy warning message",
                },
            })),
        });

        assert_eq!(
            warning,
            Some((
                thread_id,
                LegacyThreadNotification::Warning("legacy warning message".to_string())
            ))
        );
    }

    #[test]
    fn legacy_warning_notification_ignores_non_warning_legacy_events() {
        let notification = legacy_thread_notification(JSONRPCNotification {
            method: "codex/event/task_started".to_string(),
            params: Some(json!({
                "conversationId": ThreadId::new().to_string(),
                "id": "event-1",
                "msg": {
                    "type": "task_started",
                },
            })),
        });

        assert_eq!(notification, None);
    }

    #[test]
    fn legacy_thread_rollback_notification_extracts_thread_id_and_turn_count() {
        let thread_id = ThreadId::new();
        let rollback = legacy_thread_notification(JSONRPCNotification {
            method: "codex/event/thread_rolled_back".to_string(),
            params: Some(json!({
                "conversationId": thread_id.to_string(),
                "id": "event-1",
                "msg": {
                    "type": "thread_rolled_back",
                    "num_turns": 2,
                },
            })),
        });

        assert_eq!(
            rollback,
            Some((
                thread_id,
                LegacyThreadNotification::Rollback { num_turns: 2 }
            ))
        );
    }

    #[test]
    fn thread_scoped_notification_with_invalid_thread_id_is_not_treated_as_global() {
        let notification = ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "not-a-thread-id".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: TurnStatus::InProgress,
                error: None,
            },
        });

        assert_eq!(
            server_notification_thread_target(&notification),
            ServerNotificationThreadTarget::InvalidThreadId("not-a-thread-id".to_string())
        );
    }
}
