//! App-server event stream handling for the TUI app.

use super::App;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use super::app_server_event_targets::server_request_thread_id;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_event::ConnectorsSnapshot;
use crate::app_info::app_info_from_api;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::status_account_display_from_auth_mode;
use crate::terminal_browser::TERMINAL_BROWSER_NAMESPACE;
use crate::terminal_browser::dynamic_tool_response;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_protocol::ThreadId;

impl App {
    pub(super) fn refresh_mcp_startup_expected_servers_from_config(&mut self) {
        let enabled_config_mcp_servers: Vec<String> = self
            .config
            .mcp_servers
            .get()
            .iter()
            .filter_map(|(name, server)| server.enabled.then_some(name.clone()))
            .collect();
        self.chat_widget.for_each_installed_mut(|pane| {
            pane.set_mcp_startup_expected_servers(enabled_config_mcp_servers.iter().cloned());
        });
    }

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
                self.refresh_mcp_startup_expected_servers_from_config();
                self.chat_widget
                    .for_each_installed_mut(|pane| pane.finish_mcp_startup_after_lag());
            }
            AppServerEvent::ServerNotification(notification) => {
                self.handle_server_notification_event(app_server_client, notification)
                    .await;
            }
            AppServerEvent::ServerRequest(request) => {
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
        app_server_client: &AppServerSession,
        notification: ServerNotification,
    ) {
        match &notification {
            ServerNotification::ServerRequestResolved(notification) => {
                let terminal_browser_open_resolved =
                    self.discard_pending_terminal_browser_open(&notification.request_id);
                match ThreadId::from_string(&notification.thread_id) {
                    Ok(thread_id) => {
                        if terminal_browser_open_resolved
                            && let Some(pane) = self.chat_widget.by_thread_id_mut(thread_id)
                        {
                            pane.dismiss_managed_network_restore_confirmation();
                        }
                        if let Some(request) = self
                            .pending_app_server_requests
                            .resolve_notification(thread_id, &notification.request_id)
                            && let Some(pane) = self.chat_widget.by_thread_id_mut(thread_id)
                        {
                            pane.dismiss_app_server_request(&request);
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            thread_id = notification.thread_id,
                            "ignoring resolved app-server request with invalid thread ID: {err}"
                        );
                    }
                }
            }
            ServerNotification::McpServerStatusUpdated(_) => {
                self.refresh_mcp_startup_expected_servers_from_config();
            }
            ServerNotification::AccountRateLimitsUpdated(notification) => {
                self.chat_widget.for_each_installed_mut(|pane| {
                    pane.on_rolling_rate_limit_snapshot(notification.rate_limits.clone());
                });
                return;
            }
            ServerNotification::AccountUpdated(notification) => {
                let has_codex_backend_auth = matches!(
                    notification.auth_mode,
                    Some(
                        AuthMode::Chatgpt
                            | AuthMode::ChatgptAuthTokens
                            | AuthMode::AgentIdentity
                            | AuthMode::PersonalAccessToken
                    )
                );
                let status_account_display = status_account_display_from_auth_mode(
                    notification.auth_mode,
                    notification.plan_type,
                );
                let has_chatgpt_account = notification
                    .auth_mode
                    .is_some_and(AuthMode::has_chatgpt_account);
                self.chat_widget.for_each_installed_mut(|pane| {
                    pane.update_account_state(
                        status_account_display.clone(),
                        notification.plan_type,
                        has_chatgpt_account,
                        has_codex_backend_auth,
                    );
                });
                return;
            }
            ServerNotification::ExternalAgentConfigImportCompleted(notification) => {
                let should_report_completion =
                    app_server_client.consume_external_agent_config_import_completion();
                if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                    tracing::warn!(
                        error = %err,
                        "failed to refresh config after external agent config import"
                    );
                }
                let cwd = self.chat_widget.config_ref().cwd.to_path_buf();
                self.chat_widget.for_each_installed_mut(|pane| {
                    pane.refresh_plugin_mentions();
                });
                self.chat_widget.submit_op(AppCommand::reload_user_config());
                self.fetch_plugins_list(app_server_client, cwd);
                if should_report_completion {
                    let lines = crate::external_agent_config_migration_flow::external_agent_config_migration_finished_lines(notification);
                    let (selection_text, prefix_columns) =
                        crate::external_agent_config_migration_flow::external_agent_config_migration_selection(
                            &lines,
                        );
                    self.chat_widget.add_semantic_history_lines(
                        lines,
                        selection_text,
                        prefix_columns,
                    );
                }
                return;
            }
            ServerNotification::AppListUpdated(notification) => {
                let snapshot = ConnectorsSnapshot {
                    connectors: notification
                        .data
                        .iter()
                        .cloned()
                        .map(app_info_from_api)
                        .collect(),
                };
                self.chat_widget.for_each_installed_mut(|pane| {
                    pane.on_connectors_loaded(Ok(snapshot.clone()), /*is_final*/ false);
                });
                return;
            }
            _ => {}
        }

        match server_notification_thread_target(&notification) {
            ServerNotificationThreadTarget::Thread(thread_id) => {
                if self.is_thread_retired(&thread_id) {
                    tracing::debug!(%thread_id, "ignoring notification for retired conversation");
                    return;
                }
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
            ServerNotificationThreadTarget::AppScoped => {
                tracing::debug!(
                    "ignoring app-scoped MCP startup notification without a TUI app-level target"
                );
                return;
            }
            ServerNotificationThreadTarget::Global => {}
        }

        self.chat_widget.for_each_installed_mut(|pane| {
            pane.handle_server_notification(notification.clone(), /*replay_kind*/ None);
        });
    }

    async fn handle_server_request_event(
        &mut self,
        app_server_client: &AppServerSession,
        request: ServerRequest,
    ) {
        let thread_id = server_request_thread_id(&request);
        if thread_id.is_some_and(|thread_id| self.is_thread_retired(&thread_id)) {
            let request_id = request.id().clone();
            let reason = "Conversation closed before this request could be handled.".to_string();
            tracing::debug!(
                ?request_id,
                ?thread_id,
                "rejecting request for retired conversation"
            );
            if let Err(err) = self
                .reject_app_server_request(app_server_client, request_id, reason)
                .await
            {
                tracing::warn!("{err}");
            }
            return;
        }

        if let ServerRequest::DynamicToolCall { request_id, params } = &request
            && params.namespace.as_deref() == Some(TERMINAL_BROWSER_NAMESPACE)
        {
            if !self.terminal_browser_request_matches_active_thread(&params.thread_id) {
                self.app_event_tx
                    .send(AppEvent::TerminalBrowserToolCompleted {
                        request_id: request_id.clone(),
                        response: dynamic_tool_response(Err(anyhow::anyhow!(
                            "terminal browser permission policy only allows the active TUI thread"
                        ))),
                        profile_approval: None,
                    });
                return;
            }
            if params.tool == "open"
                && let Some(selection) = self.current_auto_review_selection()
                && self
                    .managed_network_restore_available(app_server_client)
                    .await
            {
                if !self.defer_terminal_browser_open(
                    request_id.clone(),
                    params.thread_id.clone(),
                    params.arguments.clone(),
                ) {
                    self.app_event_tx
                        .send(AppEvent::TerminalBrowserToolCompleted {
                            request_id: request_id.clone(),
                            response: dynamic_tool_response(Err(anyhow::anyhow!(
                                "browser_busy: another terminal browser open is waiting for permission"
                            ))),
                            profile_approval: None,
                        });
                    return;
                }
                self.chat_widget
                    .open_managed_network_restore_confirmation(selection);
                return;
            }
            let Some(browser) = self.terminal_browser_for_active_request().await else {
                self.app_event_tx
                    .send(AppEvent::TerminalBrowserToolCompleted {
                        request_id: request_id.clone(),
                        response: dynamic_tool_response(Err(anyhow::anyhow!(
                            "terminal browser permission policy disables this TUI session"
                        ))),
                        profile_approval: None,
                    });
                return;
            };
            let request_id = request_id.clone();
            let session_key = params.thread_id.clone();
            let tool = params.tool.clone();
            let arguments = params.arguments.clone();
            let profile_approval = (tool == "profile")
                .then(|| crate::terminal_browser::requested_profile_command(&arguments))
                .flatten()
                .and_then(|command| self.terminal_browser_profile_approval(command));
            let app_event_tx = self.app_event_tx.clone();
            tokio::spawn(async move {
                let result = browser.execute(&session_key, &tool, arguments).await;
                let profile_approval = result.is_ok().then_some(profile_approval).flatten();
                let response = dynamic_tool_response(result);
                app_event_tx.send(AppEvent::TerminalBrowserToolCompleted {
                    request_id,
                    response,
                    profile_approval,
                });
            });
            return;
        }

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

        let Some(thread_id) = thread_id else {
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
}

#[cfg(test)]
#[path = "app_server_events_tests.rs"]
mod tests;
