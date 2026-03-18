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
                if let Some((thread_id, message)) = legacy_warning_notification(notification) {
                    let result = if self.primary_thread_id == Some(thread_id)
                        || self.primary_thread_id.is_none()
                    {
                        self.enqueue_primary_thread_legacy_warning(message).await
                    } else {
                        self.enqueue_thread_legacy_warning(thread_id, message).await
                    };
                    if let Err(err) = result {
                        tracing::warn!("failed to enqueue app-server legacy warning: {err}");
                    }
                } else {
                    tracing::warn!("ignoring legacy app-server notification in tui_app_server");
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
                self.pending_app_server_requests
                    .resolve_notification(&notification.request_id);
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

fn token_usage_from_app_server(
    value: codex_app_server_protocol::TokenUsageBreakdown,
) -> TokenUsage {
    TokenUsage {
        input_tokens: value.input_tokens,
        cached_input_tokens: value.cached_input_tokens,
        output_tokens: value.output_tokens,
        reasoning_output_tokens: value.reasoning_output_tokens,
        total_tokens: value.total_tokens,
    }
}

/// Expand a single `Turn` into the event sequence the TUI would have
/// observed if it had been connected for the turn's entire lifetime.
///
/// Snapshot replay keeps committed-item semantics for user / plan /
/// agent-message items, while replaying the legacy events that still
/// drive rendering for reasoning, web-search, image-generation, and
/// context-compaction history cells.
fn turn_snapshot_events(
    thread_id: ThreadId,
    turn: &Turn,
    show_raw_agent_reasoning: bool,
) -> Vec<Event> {
    let mut events = vec![Event {
        id: String::new(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn.id.clone(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::default(),
        }),
    }];

    for item in &turn.items {
        let Some(item) = thread_item_to_core(item) else {
            continue;
        };
        match item {
            TurnItem::UserMessage(_) | TurnItem::Plan(_) | TurnItem::AgentMessage(_) => {
                events.push(Event {
                    id: String::new(),
                    msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                        thread_id,
                        turn_id: turn.id.clone(),
                        item,
                    }),
                });
            }
            TurnItem::Reasoning(_)
            | TurnItem::WebSearch(_)
            | TurnItem::ImageGeneration(_)
            | TurnItem::ContextCompaction(_) => {
                events.extend(
                    item.as_legacy_events(show_raw_agent_reasoning)
                        .into_iter()
                        .map(|msg| Event {
                            id: String::new(),
                            msg,
                        }),
                );
            }
        }
    }

    append_terminal_turn_events(&mut events, turn, /*include_failed_error*/ true);

    events
}

/// Append the terminal event(s) for a turn based on its `TurnStatus`.
///
/// This function is shared between the live notification bridge
/// (`TurnCompleted` handling) and the snapshot replay path so that both
/// produce identical `EventMsg` sequences for the same turn status.
///
/// - `Completed` → `TurnComplete`
/// - `Interrupted` → `TurnAborted { reason: Interrupted }`
/// - `Failed` → `Error` (if present) then `TurnComplete`
/// - `InProgress` → no events (the turn is still running)
fn append_terminal_turn_events(events: &mut Vec<Event>, turn: &Turn, include_failed_error: bool) {
    match turn.status {
        TurnStatus::Completed => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn.id.clone(),
                last_agent_message: None,
            }),
        }),
        TurnStatus::Interrupted => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some(turn.id.clone()),
                reason: TurnAbortReason::Interrupted,
            }),
        }),
        TurnStatus::Failed => {
            if include_failed_error && let Some(error) = &turn.error {
                events.push(Event {
                    id: String::new(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: error.message.clone(),
                        codex_error_info: error
                            .codex_error_info
                            .clone()
                            .and_then(app_server_codex_error_info_to_core),
                    }),
                });
            }
            events.push(Event {
                id: String::new(),
                msg: EventMsg::TurnComplete(TurnCompleteEvent {
                    turn_id: turn.id.clone(),
                    last_agent_message: None,
                }),
            });
        }
        TurnStatus::InProgress => {
            // Preserve unfinished turns during snapshot replay without emitting completion events.
        }
    }
}

fn thread_item_to_core(item: &ThreadItem) -> Option<TurnItem> {
    match item {
        ThreadItem::UserMessage { id, content } => Some(TurnItem::UserMessage(UserMessageItem {
            id: id.clone(),
            content: content
                .iter()
                .cloned()
                .map(codex_app_server_protocol::UserInput::into_core)
                .collect(),
        })),
        ThreadItem::AgentMessage {
            id,
            text,
            phase,
            memory_citation,
        } => Some(TurnItem::AgentMessage(AgentMessageItem {
            id: id.clone(),
            content: vec![AgentMessageContent::Text { text: text.clone() }],
            phase: phase.clone(),
            memory_citation: memory_citation.clone().map(|citation| {
                codex_protocol::memory_citation::MemoryCitation {
                    entries: citation
                        .entries
                        .into_iter()
                        .map(
                            |entry| codex_protocol::memory_citation::MemoryCitationEntry {
                                path: entry.path,
                                line_start: entry.line_start,
                                line_end: entry.line_end,
                                note: entry.note,
                            },
                        )
                        .collect(),
                    rollout_ids: citation.thread_ids,
                }
            }),
        })),
        ThreadItem::Plan { id, text } => Some(TurnItem::Plan(PlanItem {
            id: id.clone(),
            text: text.clone(),
        })),
        ThreadItem::Reasoning {
            id,
            summary,
            content,
        } => Some(TurnItem::Reasoning(ReasoningItem {
            id: id.clone(),
            summary_text: summary.clone(),
            raw_content: content.clone(),
        })),
        ThreadItem::WebSearch { id, query, action } => Some(TurnItem::WebSearch(WebSearchItem {
            id: id.clone(),
            query: query.clone(),
            action: app_server_web_search_action_to_core(action.clone()?)?,
        })),
        ThreadItem::ImageGeneration {
            id,
            status,
            revised_prompt,
            result,
        } => Some(TurnItem::ImageGeneration(ImageGenerationItem {
            id: id.clone(),
            status: status.clone(),
            revised_prompt: revised_prompt.clone(),
            result: result.clone(),
            saved_path: None,
        })),
        ThreadItem::ContextCompaction { id } => {
            Some(TurnItem::ContextCompaction(ContextCompactionItem {
                id: id.clone(),
            }))
        }
        ThreadItem::CommandExecution { .. }
        | ThreadItem::FileChange { .. }
        | ThreadItem::McpToolCall { .. }
        | ThreadItem::DynamicToolCall { .. }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. } => {
            tracing::debug!("ignoring unsupported app-server thread item in TUI adapter");
            None
        }
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    use base64::Engine;
    use chrono::Utc;
    use codex_app_server_protocol::AuthMode;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_core::auth::AuthDotJson;
    use codex_core::auth::save_auth;
    use codex_core::token_data::TokenData;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;
    use tempfile::TempDir;

    fn fake_jwt(account_id: &str, plan_type: &str) -> String {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
                "chatgpt_plan_type": plan_type,
            },
        });
        let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = encode(&serde_json::to_vec(&header).expect("serialize header"));
        let payload_b64 = encode(&serde_json::to_vec(&payload).expect("serialize payload"));
        let signature_b64 = encode(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }

    fn write_chatgpt_auth(codex_home: &std::path::Path) {
        let id_token = fake_jwt("workspace-1", "business");
        let access_token = fake_jwt("workspace-1", "business");
        save_auth(
            codex_home,
            &AuthDotJson {
                auth_mode: Some(AuthMode::Chatgpt),
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: codex_core::token_data::parse_chatgpt_jwt_claims(&id_token)
                        .expect("id token should parse"),
                    access_token,
                    refresh_token: "refresh-token".to_string(),
                    account_id: Some("workspace-1".to_string()),
                }),
                last_refresh: Some(Utc::now()),
            },
            AuthCredentialsStoreMode::File,
        )
        .expect("chatgpt auth should save");
    }

    #[test]
    fn refresh_request_uses_local_chatgpt_auth() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path());

        let response = resolve_chatgpt_auth_tokens_refresh_response(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
            &ChatgptAuthTokensRefreshParams {
                reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                previous_account_id: Some("workspace-1".to_string()),
            },
        )
        .expect("refresh response should resolve");

        assert_eq!(response.chatgpt_account_id, "workspace-1");
        assert_eq!(response.chatgpt_plan_type.as_deref(), Some("business"));
        assert!(!response.access_token.is_empty());
    }

    #[test]
    fn refresh_request_rejects_account_mismatch() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path());

        let err = resolve_chatgpt_auth_tokens_refresh_response(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
            &ChatgptAuthTokensRefreshParams {
                reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                previous_account_id: Some("workspace-2".to_string()),
            },
        )
        .expect_err("mismatched account should fail");

        assert_eq!(
            err,
            "local ChatGPT auth refresh account mismatch: expected `workspace-2`, got `workspace-1`"
        );
    }
}

fn app_server_web_search_action_to_core(
    action: codex_app_server_protocol::WebSearchAction,
) -> Option<codex_protocol::models::WebSearchAction> {
    match action {
        codex_app_server_protocol::WebSearchAction::Search { query, queries } => {
            Some(codex_protocol::models::WebSearchAction::Search { query, queries })
        }
        codex_app_server_protocol::WebSearchAction::OpenPage { url } => {
            Some(codex_protocol::models::WebSearchAction::OpenPage { url })
        }
        codex_app_server_protocol::WebSearchAction::FindInPage { url, pattern } => {
            Some(codex_protocol::models::WebSearchAction::FindInPage { url, pattern })
        }
        codex_app_server_protocol::WebSearchAction::Other => {
            Some(codex_protocol::models::WebSearchAction::Other)
        }
    }
}

fn app_server_codex_error_info_to_core(
    value: codex_app_server_protocol::CodexErrorInfo,
) -> Option<codex_protocol::protocol::CodexErrorInfo> {
    serde_json::from_value(serde_json::to_value(value).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::server_notification_thread_events;
    use super::thread_snapshot_events;
    use super::turn_snapshot_events;
    use codex_app_server_protocol::AgentMessageDeltaNotification;
    use codex_app_server_protocol::CodexErrorInfo;
    use codex_app_server_protocol::ItemCompletedNotification;
    use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::Thread;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnError;
    use codex_app_server_protocol::TurnStatus;
    use codex_protocol::ThreadId;
    use codex_protocol::items::AgentMessageContent;
    use codex_protocol::items::AgentMessageItem;
    use codex_protocol::items::TurnItem;
    use codex_protocol::models::MessagePhase;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn bridges_completed_agent_messages_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();
        let item_id = "msg_123".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                item: ThreadItem::AgentMessage {
                    id: item_id,
                    text: "Hello from your coding assistant.".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        assert_eq!(event.id, String::new());
        let EventMsg::ItemCompleted(completed) = &event.msg else {
            panic!("expected item completed event");
        };
        assert_eq!(
            completed.thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        assert_eq!(completed.turn_id, turn_id);
        match &completed.item {
            TurnItem::AgentMessage(AgentMessageItem {
                id,
                content,
                phase,
                memory_citation,
            }) => {
                assert_eq!(id, "msg_123");
                let [AgentMessageContent::Text { text }] = content.as_slice() else {
                    panic!("expected a single text content item");
                };
                assert_eq!(text, "Hello from your coding assistant.");
                assert_eq!(*phase, Some(MessagePhase::FinalAnswer));
                assert_eq!(*memory_citation, None);
            }
            _ => panic!("expected bridged agent message item"),
        }
    }

    #[test]
    fn bridges_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Completed,
                    error: None,
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        assert_eq!(event.id, String::new());
        let EventMsg::TurnComplete(completed) = &event.msg else {
            panic!("expected turn complete event");
        };
        assert_eq!(completed.turn_id, turn_id);
        assert_eq!(completed.last_agent_message, None);
    }

    #[test]
    fn bridges_interrupted_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Interrupted,
                    error: None,
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        let EventMsg::TurnAborted(aborted) = &event.msg else {
            panic!("expected turn aborted event");
        };
        assert_eq!(aborted.turn_id.as_deref(), Some(turn_id.as_str()));
        assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    }

    #[test]
    fn bridges_failed_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Failed,
                    error: Some(TurnError {
                        message: "request failed".to_string(),
                        codex_error_info: Some(CodexErrorInfo::Other),
                        additional_details: None,
                    }),
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [complete_event] = events.as_slice() else {
            panic!("expected turn completion only");
        };
        let EventMsg::TurnComplete(completed) = &complete_event.msg else {
            panic!("expected turn complete event");
        };
        assert_eq!(completed.turn_id, turn_id);
        assert_eq!(completed.last_agent_message, None);
    }

    #[test]
    fn bridges_text_deltas_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();

        let (_, agent_events) = server_notification_thread_events(
            ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: thread_id.clone(),
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: "Hello".to_string(),
            }),
        )
        .expect("notification should bridge");
        let [agent_event] = agent_events.as_slice() else {
            panic!("expected one bridged agent delta event");
        };
        assert_eq!(agent_event.id, String::new());
        let EventMsg::AgentMessageDelta(delta) = &agent_event.msg else {
            panic!("expected bridged agent message delta");
        };
        assert_eq!(delta.delta, "Hello");

        let (_, reasoning_events) = server_notification_thread_events(
            ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
                thread_id,
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: "Thinking".to_string(),
                summary_index: 0,
            }),
        )
        .expect("notification should bridge");
        let [reasoning_event] = reasoning_events.as_slice() else {
            panic!("expected one bridged reasoning delta event");
        };
        assert_eq!(reasoning_event.id, String::new());
        let EventMsg::AgentReasoningDelta(delta) = &reasoning_event.msg else {
            panic!("expected bridged reasoning delta");
        };
        assert_eq!(delta.delta, "Thinking");
    }

    #[test]
    fn bridges_thread_snapshot_turns_for_resume_restore() {
        let thread_id = ThreadId::new();
        let events = thread_snapshot_events(
            &Thread {
                id: thread_id.to_string(),
                preview: "hello".to_string(),
                ephemeral: false,
                model_provider: "openai".to_string(),
                created_at: 0,
                updated_at: 0,
                status: ThreadStatus::Idle,
                path: None,
                cwd: PathBuf::from("/tmp/project"),
                cli_version: "test".to_string(),
                source: SessionSource::Cli.into(),
                agent_nickname: None,
                agent_role: None,
                git_info: None,
                name: Some("restore".to_string()),
                turns: vec![
                    Turn {
                        id: "turn-complete".to_string(),
                        items: vec![
                            ThreadItem::UserMessage {
                                id: "user-1".to_string(),
                                content: vec![codex_app_server_protocol::UserInput::Text {
                                    text: "hello".to_string(),
                                    text_elements: Vec::new(),
                                }],
                            },
                            ThreadItem::AgentMessage {
                                id: "assistant-1".to_string(),
                                text: "hi".to_string(),
                                phase: Some(MessagePhase::FinalAnswer),
                                memory_citation: None,
                            },
                        ],
                        status: TurnStatus::Completed,
                        error: None,
                    },
                    Turn {
                        id: "turn-interrupted".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Interrupted,
                        error: None,
                    },
                    Turn {
                        id: "turn-failed".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Failed,
                        error: Some(TurnError {
                            message: "request failed".to_string(),
                            codex_error_info: Some(CodexErrorInfo::Other),
                            additional_details: None,
                        }),
                    },
                ],
            },
            /*show_raw_agent_reasoning*/ false,
        );

        assert_eq!(events.len(), 9);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        assert!(matches!(events[1].msg, EventMsg::ItemCompleted(_)));
        assert!(matches!(events[2].msg, EventMsg::ItemCompleted(_)));
        assert!(matches!(events[3].msg, EventMsg::TurnComplete(_)));
        assert!(matches!(events[4].msg, EventMsg::TurnStarted(_)));
        let EventMsg::TurnAborted(TurnAbortedEvent { turn_id, reason }) = &events[5].msg else {
            panic!("expected interrupted turn replay");
        };
        assert_eq!(turn_id.as_deref(), Some("turn-interrupted"));
        assert_eq!(*reason, TurnAbortReason::Interrupted);
        assert!(matches!(events[6].msg, EventMsg::TurnStarted(_)));
        let EventMsg::Error(error) = &events[7].msg else {
            panic!("expected failed turn error replay");
        };
        assert_eq!(error.message, "request failed");
        assert_eq!(
            error.codex_error_info,
            Some(codex_protocol::protocol::CodexErrorInfo::Other)
        );
        assert!(matches!(events[8].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn bridges_non_message_snapshot_items_via_legacy_events() {
        let events = turn_snapshot_events(
            ThreadId::new(),
            &Turn {
                id: "turn-complete".to_string(),
                items: vec![
                    ThreadItem::Reasoning {
                        id: "reasoning-1".to_string(),
                        summary: vec!["Need to inspect config".to_string()],
                        content: vec!["hidden chain".to_string()],
                    },
                    ThreadItem::WebSearch {
                        id: "search-1".to_string(),
                        query: "ratatui stylize".to_string(),
                        action: Some(codex_app_server_protocol::WebSearchAction::Other),
                    },
                    ThreadItem::ImageGeneration {
                        id: "image-1".to_string(),
                        status: "completed".to_string(),
                        revised_prompt: Some("diagram".to_string()),
                        result: "image.png".to_string(),
                    },
                    ThreadItem::ContextCompaction {
                        id: "compact-1".to_string(),
                    },
                ],
                status: TurnStatus::Completed,
                error: None,
            },
            /*show_raw_agent_reasoning*/ false,
        );

        assert_eq!(events.len(), 6);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        let EventMsg::AgentReasoning(reasoning) = &events[1].msg else {
            panic!("expected reasoning replay");
        };
        assert_eq!(reasoning.text, "Need to inspect config");
        let EventMsg::WebSearchEnd(web_search) = &events[2].msg else {
            panic!("expected web search replay");
        };
        assert_eq!(web_search.call_id, "search-1");
        assert_eq!(web_search.query, "ratatui stylize");
        assert_eq!(
            web_search.action,
            codex_protocol::models::WebSearchAction::Other
        );
        let EventMsg::ImageGenerationEnd(image_generation) = &events[3].msg else {
            panic!("expected image generation replay");
        };
        assert_eq!(image_generation.call_id, "image-1");
        assert_eq!(image_generation.status, "completed");
        assert_eq!(image_generation.revised_prompt.as_deref(), Some("diagram"));
        assert_eq!(image_generation.result, "image.png");
        assert!(matches!(events[4].msg, EventMsg::ContextCompacted(_)));
        assert!(matches!(events[5].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn bridges_raw_reasoning_snapshot_items_when_enabled() {
        let events = turn_snapshot_events(
            ThreadId::new(),
            &Turn {
                id: "turn-complete".to_string(),
                items: vec![ThreadItem::Reasoning {
                    id: "reasoning-1".to_string(),
                    summary: vec!["Need to inspect config".to_string()],
                    content: vec!["hidden chain".to_string()],
                }],
                status: TurnStatus::Completed,
                error: None,
            },
            /*show_raw_agent_reasoning*/ true,
        );

        assert_eq!(events.len(), 4);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        let EventMsg::AgentReasoning(reasoning) = &events[1].msg else {
            panic!("expected reasoning replay");
        };
        assert_eq!(reasoning.text, "Need to inspect config");
        let EventMsg::AgentReasoningRawContent(raw_reasoning) = &events[2].msg else {
            panic!("expected raw reasoning replay");
        };
        assert_eq!(raw_reasoning.text, "hidden chain");
        assert!(matches!(events[3].msg, EventMsg::TurnComplete(_)));
    }
}

fn legacy_warning_notification(notification: JSONRPCNotification) -> Option<(ThreadId, String)> {
    let method = notification
        .method
        .strip_prefix("codex/event/")
        .unwrap_or(&notification.method);
    if method != "warning" {
        return None;
    }

    let Value::Object(mut params) = notification.params? else {
        return None;
    };
    let thread_id = params
        .remove("conversationId")
        .and_then(|value| serde_json::from_value::<String>(value).ok())
        .and_then(|value| ThreadId::from_string(&value).ok())?;
    let message = params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|msg| {
            msg.get("type")
                .and_then(Value::as_str)
                .zip(msg.get("message"))
        })
        .and_then(|(kind, message)| (kind == "warning").then_some(message))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)?;
    Some((thread_id, message))
}

#[cfg(test)]
mod tests {
    use super::ServerNotificationThreadTarget;
    use super::legacy_warning_notification;
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
        let warning = legacy_warning_notification(JSONRPCNotification {
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
            Some((thread_id, "legacy warning message".to_string()))
        );
    }

    #[test]
    fn legacy_warning_notification_ignores_non_warning_legacy_events() {
        let notification = legacy_warning_notification(JSONRPCNotification {
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
