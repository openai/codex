//! App-server event stream handling for the TUI app.

use super::App;
use super::ThreadBufferedEvent;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use super::app_server_event_targets::server_request_thread_id;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_info::app_info_from_api;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::status_account_display_from_auth_mode;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_protocol::protocol::SubAgentSource;

impl App {
    pub(super) fn refresh_mcp_startup_expected_servers_from_config(&mut self) {
        if self
            .current_displayed_thread_id()
            .zip(self.primary_thread_id)
            .is_some_and(|(thread_id, primary_thread_id)| {
                self.agent_navigation.is_parent_owned(thread_id)
                    || (thread_id != primary_thread_id
                        && !self.side_threads.contains_key(&thread_id))
            })
        {
            // Subagents can defer cached servers indefinitely, so only servers
            // that actually report startup should keep their status running.
            self.chat_widget
                .set_mcp_startup_expected_servers(std::iter::empty());
            return;
        }

        let enabled_config_mcp_servers: Vec<String> = self
            .config
            .mcp_servers
            .get()
            .iter()
            .filter_map(|(name, server)| server.enabled.then_some(name.clone()))
            .collect();
        self.chat_widget
            .set_mcp_startup_expected_servers(enabled_config_mcp_servers);
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
                self.chat_widget.finish_mcp_startup_after_lag();
                self.refresh_agents_overview_threads(app_server_client);
            }
            AppServerEvent::ServerNotification(notification) => {
                self.handle_server_notification_event(app_server_client, *notification)
                    .await;
            }
            AppServerEvent::ServerRequest(request) => {
                self.handle_server_request_event(app_server_client, *request)
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
        if let ServerNotification::ThreadStarted(started) = &notification
            && let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) = &started.thread.source
            && self
                .agents_overview
                .dispatched_requests
                .contains_key(parent_thread_id)
            && let Ok(thread_id) = codex_protocol::ThreadId::from_string(&started.thread.id)
        {
            self.agents_overview
                .dispatched_requests
                .entry(thread_id)
                .or_default();
        }
        if matches!(
            &notification,
            ServerNotification::ThreadStarted(_)
                | ServerNotification::ThreadStatusChanged(_)
                | ServerNotification::ThreadSettingsUpdated(_)
                | ServerNotification::ThreadNameUpdated(_)
                | ServerNotification::ThreadArchived(_)
                | ServerNotification::ThreadDeleted(_)
                | ServerNotification::ThreadClosed(_)
        ) {
            self.refresh_agents_overview_threads(app_server_client);
        }
        match &notification {
            ServerNotification::ServerRequestResolved(notification) => {
                self.pending_primary_events.retain(|event| {
                    !matches!(event, ThreadBufferedEvent::Request(request)
                        if request.id() == &notification.request_id)
                });
                for requests in self.agents_overview.dispatched_requests.values_mut() {
                    requests.retain(|request| request.id() != &notification.request_id);
                }
                if let Some(request) = self
                    .pending_app_server_requests
                    .resolve_notification(&notification.request_id)
                {
                    self.chat_widget.dismiss_app_server_request(&request);
                    if self.startup_pending_protected_request {
                        self.startup_pending_protected_request =
                            self.chat_widget.has_pending_protected_request();
                    }
                }
            }
            ServerNotification::McpServerStatusUpdated(_) => {
                self.refresh_mcp_startup_expected_servers_from_config();
            }
            ServerNotification::AccountRateLimitsUpdated(notification) => {
                if matches!(
                    notification.rate_limits.rate_limit_reached_type,
                    Some(
                        RateLimitReachedType::WorkspaceOwnerCreditsDepleted
                            | RateLimitReachedType::WorkspaceMemberCreditsDepleted
                            | RateLimitReachedType::WorkspaceOwnerUsageLimitReached
                            | RateLimitReachedType::WorkspaceMemberUsageLimitReached
                    )
                ) || notification.rate_limits.spend_control_reached == Some(true)
                {
                    self.rate_limit_hard_stop_generation =
                        self.rate_limit_hard_stop_generation.wrapping_add(1);
                }
                self.chat_widget
                    .on_rolling_rate_limit_snapshot(notification.rate_limits.clone());
                return;
            }
            ServerNotification::AccountUpdated(notification) => {
                // Deferred terminal writes must never carry the previous account's billing into
                // the newly authenticated identity, even when both accounts share one thread.
                self.last_thread_usage_status_cell = None;
                self.pending_thread_usage_history_refresh = false;
                let has_codex_backend_auth = matches!(
                    notification.auth_mode,
                    Some(
                        AuthMode::Chatgpt
                            | AuthMode::ChatgptAuthTokens
                            | AuthMode::AgentIdentity
                            | AuthMode::PersonalAccessToken
                    )
                );
                self.chat_widget.update_account_state(
                    status_account_display_from_auth_mode(
                        notification.auth_mode,
                        notification.plan_type,
                    ),
                    notification.plan_type,
                    notification
                        .auth_mode
                        .is_some_and(AuthMode::has_chatgpt_account),
                    has_codex_backend_auth,
                );
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
                self.chat_widget.refresh_plugin_mentions();
                self.chat_widget.submit_op(AppCommand::reload_user_config());
                self.fetch_plugins_list(app_server_client, cwd);
                if should_report_completion {
                    self.chat_widget.add_plain_history_lines(
                        crate::external_agent_config_migration::flow::external_agent_config_migration_finished_lines(notification),
                    );
                }
                return;
            }
            ServerNotification::AppListUpdated(notification) => {
                if self.current_displayed_thread_id().is_some() {
                    self.chat_widget
                        .refresh_connector_directory_after_notification(
                            notification
                                .data
                                .iter()
                                .cloned()
                                .map(app_info_from_api)
                                .collect(),
                        );
                }
                return;
            }
            _ => {}
        }

        match server_notification_thread_target(&notification) {
            ServerNotificationThreadTarget::Thread(thread_id) => {
                if self.primary_thread_id.is_none() && !self.pending_startup_thread_start {
                    return;
                }
                if self.primary_thread_id.is_some()
                    && self.primary_thread_id != Some(thread_id)
                    && !self.thread_event_channels.contains_key(&thread_id)
                    && self.agent_navigation.get(&thread_id).is_none()
                    && !self.side_threads.contains_key(&thread_id)
                    && !matches!(&notification, ServerNotification::McpServerStatusUpdated(_))
                    && !matches!(
                        &notification,
                        ServerNotification::ThreadStarted(started)
                            if matches!(
                                &started.thread.source,
                                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                                    parent_thread_id,
                                    ..
                                }) if self.primary_thread_id == Some(*parent_thread_id)
                                    || self.thread_event_channels.contains_key(parent_thread_id)
                                    || self.agent_navigation.get(parent_thread_id).is_some()
                            )
                    )
                {
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

        self.chat_widget
            .handle_server_notification(notification, /*replay_kind*/ None);
    }

    async fn handle_server_request_event(
        &mut self,
        app_server_client: &AppServerSession,
        request: ServerRequest,
    ) {
        let thread_id = server_request_thread_id(&request);
        if thread_id.is_some_and(|thread_id| self.abandoned_side_threads.contains(&thread_id)) {
            if let Err(err) = self
                .reject_app_server_request(
                    app_server_client,
                    request.id().clone(),
                    "side conversation was closed".to_string(),
                )
                .await
            {
                tracing::warn!("{err}");
            }
            return;
        }
        if thread_id.is_some()
            && self.primary_thread_id.is_none()
            && self.pending_startup_thread_start
        {
            self.pending_primary_events
                .push_back(ThreadBufferedEvent::Request(Box::new(request)));
            return;
        }
        let unsupported_request = matches!(
            &request,
            ServerRequest::DynamicToolCall { .. }
                | ServerRequest::AttestationGenerate { .. }
                | ServerRequest::CurrentTimeRead { .. }
                | ServerRequest::ApplyPatchApproval { .. }
                | ServerRequest::ExecCommandApproval { .. }
        );
        if self
            .pending_app_server_requests
            .contains_server_request(&request)
        {
            return;
        }
        if let Some(thread_id) = thread_id
            && self.primary_thread_id != Some(thread_id)
            && !unsupported_request
            && let Some(requests) = self.agents_overview.dispatched_requests.get_mut(&thread_id)
        {
            requests.push(request);
            return;
        }
        if thread_id.is_some()
            && self.primary_thread_id.is_none()
            && !self.pending_startup_thread_start
            && !unsupported_request
        {
            return;
        }
        if let Some(thread_id) = thread_id
            && self.primary_thread_id.is_some()
            && self.primary_thread_id != Some(thread_id)
            && !self.thread_event_channels.contains_key(&thread_id)
            && self.agent_navigation.get(&thread_id).is_none()
            && !self.side_threads.contains_key(&thread_id)
            && !unsupported_request
        {
            let thread = app_server_client
                .request_handle()
                .request_typed::<ThreadReadResponse>(ClientRequest::ThreadRead {
                    request_id: RequestId::String(format!("subagent-approval-{thread_id}")),
                    params: ThreadReadParams {
                        thread_id: thread_id.to_string(),
                        include_turns: false,
                    },
                })
                .await;
            let Ok(ThreadReadResponse { thread }) = thread else {
                return;
            };
            let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) = thread.source
            else {
                return;
            };
            if self.primary_thread_id != Some(parent_thread_id)
                && !self.thread_event_channels.contains_key(&parent_thread_id)
                && self.agent_navigation.get(&parent_thread_id).is_none()
            {
                if self
                    .agents_overview
                    .dispatched_requests
                    .contains_key(&parent_thread_id)
                {
                    self.agents_overview
                        .dispatched_requests
                        .entry(thread_id)
                        .or_default()
                        .push(request);
                }
                return;
            }
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
