//! TUI-only out-of-process reconnection. Each attempt initializes a fresh client and rejoins existing
//! threads using ordinary resume/history semantics; no user operation is retried.
//! Offline input and old async completions are quarantined.

use super::*;
use crate::app_server_session::ResumeModelSettings;
use crate::dynamic_tools_mcp::ThreadToolTransport;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ReconnectPresentation {
    #[default]
    Conversation,
    Overview,
}

#[derive(Default)]
pub(super) struct ReconnectState {
    pub(super) offline: bool,
    pub(super) failed: bool,
    pub(super) presentation: ReconnectPresentation,
    pub(super) seen_version_notice: Option<String>,
}

pub(super) struct Reconnected {
    session: AppServerSession,
    bootstrap: AppServerBootstrap,
    thread: Option<AppServerStartedThread>,
}

pub(super) async fn reconnect(
    target: AppServerTarget,
    config: Config,
    local_settings: crate::local_settings::LocalSettings,
    thread_id: Option<ThreadId>,
    remote_cwd: Option<PathBuf>,
    task_tools: ThreadToolTransport,
    presentation: ReconnectPresentation,
) -> Result<Reconnected> {
    let mode = target.thread_params_mode();
    if matches!(target, AppServerTarget::Embedded) {
        color_eyre::eyre::bail!("in-process sessions have no connection to restore");
    }
    if let ThreadToolTransport::Mcp(server) = &task_tools {
        server.suspend();
    }
    if presentation == ReconnectPresentation::Conversation && thread_id.is_none() {
        color_eyre::eyre::bail!(
            "The initial thread may have been created, but its ID was not received. Nothing was retried. Your prompt is editable; inspect your tasks before relaunching."
        );
    }
    // Connecting already has transport deadlines. Give healthy history/inventory hydration one
    // shared budget instead of repeatedly discarding its progress on a short per-attempt timer.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(/*secs*/ 120);
    for delay in [0, 1, 2, 4, 8] {
        let attempt = async {
            tokio::time::sleep(Duration::from_secs(delay)).await;
            let client = crate::app_server_connection::connect(&target).await?;
            let mut session = AppServerSession::new(client, mode)
                .with_local_codex_home(&config.codex_home)
                .with_remote_cwd_override(remote_cwd.clone())
                .with_thread_tool_transport(task_tools.clone());
            let bootstrap = session.bootstrap(&config).await?;
            let thread = if let Some(thread_id) = thread_id {
                match session
                    .resume_thread(
                        &local_settings,
                        config.clone(),
                        thread_id,
                        ResumeModelSettings::PreserveExistingThread,
                    )
                    .await
                {
                    Ok(thread) => Some(thread),
                    Err(error)
                        if matches!(
                            error.downcast_ref::<TypedRequestError>(),
                            Some(TypedRequestError::Transport { .. })
                        ) =>
                    {
                        return Err(error);
                    }
                    // Unloading threads use the same code as unavailable conversations, but
                    // ordinary resume can reattach once the unload finishes.
                    Err(error)
                        if matches!(
                            error.downcast_ref::<TypedRequestError>(),
                            Some(TypedRequestError::Server { method, source })
                                if method == "thread/resume" && source.code == -32600
                                    && source.message.starts_with(&format!("thread {thread_id} is closing;"))
                        ) =>
                    {
                        return Err(error);
                    }
                    Err(error)
                        if matches!(
                            error.downcast_ref::<TypedRequestError>(),
                            Some(TypedRequestError::Server { source, .. }) if source.code == -32600
                        ) || presentation == ReconnectPresentation::Overview =>
                    {
                        None
                    }
                    Err(error) => return Err(error),
                }
            } else {
                None
            };
            Ok::<_, color_eyre::Report>(Reconnected {
                session,
                bootstrap,
                thread,
            })
        };
        let result = tokio::time::timeout_at(deadline, attempt).await;
        match result {
            Ok(Ok(connected)) => return Ok(connected),
            Ok(Err(_)) => {}
            Err(_) => break,
        }
        // Transport errors can contain endpoint credentials. Do not render or log them.
    }
    color_eyre::eyre::bail!("app-server session could not be restored")
}

impl App {
    // Preserve local choices for future input, without replaying failed settings writes or
    // changing the server's authorization for work that was already admitted.
    pub(super) fn restore_runtime_permissions(
        &self,
        session: &mut ThreadSessionState,
        cached: &ThreadSessionState,
    ) {
        if self.current_displayed_thread_id() != Some(session.thread_id)
            && self.primary_thread_id != Some(session.thread_id)
        {
            return;
        }
        // Side conversations can replace the app-wide overrides. Only copy choices
        // that still match this conversation's own cached settings.
        if let Some(policy) = self
            .runtime_approval_policy_override
            .map(RuntimeApprovalPolicyOverride::policy)
            && policy == cached.approval_policy
        {
            session.approval_policy = policy;
        }
        if let Some(profile) = &self.runtime_permission_profile_override
            && profile.permission_profile == cached.permission_profile
            && profile.active_permission_profile == cached.active_permission_profile
            && self.config.approvals_reviewer == cached.approvals_reviewer
        {
            session.permission_profile = profile.permission_profile.clone();
            session.active_permission_profile = profile.active_permission_profile.clone();
            session.approvals_reviewer = self.config.approvals_reviewer;
        }
    }

    pub(super) fn thread_unavailable(&self, id: ThreadId) -> bool {
        !matches!(self.app_server_target, AppServerTarget::Embedded)
            && self
                .thread_event_channels
                .get(&id)
                .is_some_and(|channel| channel.attachment() != ThreadEventAttachment::Live)
    }

    pub(super) fn recover_transport_error(&mut self, error: &color_eyre::Report) -> bool {
        let disconnected = matches!(
            error.downcast_ref::<TypedRequestError>(),
            Some(TypedRequestError::Transport { .. })
        );
        disconnected && self.begin_reconnect()
    }

    pub(super) fn begin_reconnect(&mut self) -> bool {
        if matches!(self.app_server_target, AppServerTarget::Embedded) {
            return false;
        }
        if !self.reconnect.offline {
            #[cfg(target_os = "windows")]
            if self.windows_sandbox_setup_is_local()
                && let Some(message) = self.chat_widget.initial_user_message.take()
            {
                self.chat_widget.restore_user_message_to_composer(message);
            }
            self.reconnect.offline = true;
            self.reconnect.failed = false;
            if self.pending_server_version_notice.take().is_some() {
                self.reconnect.seen_version_notice = None;
                self.update_server_version_overview_notice(
                    CODEX_CLI_VERSION,
                    /*older_server*/ None,
                );
            }
            self.cancel_pending_key_chord();
            self.overlay = None;
            self.commit_animation = None;
            self.clear_recap_request(crate::app_event::RecapTrigger::Manual);
            if let Some(task) = self.agents_overview.refresh_task.take() {
                task.abort();
            }
            self.agents_overview.request_id = None;
            self.agents_overview.refresh_pending = false;
            self.agents_overview.refresh_notifications.clear();
            self.agents_overview.pending_usage = None;
            self.agents_overview.usage_disabled = false;
            self.agents_overview.usage.clear();
            self.agents_overview.activity.clear();
            self.agents_overview.last_messages.clear();
            self.reconnect.presentation = if self
                .chat_widget
                .selected_index_for_active_view(agents_overview::AGENTS_OVERVIEW_VIEW_ID)
                .is_some()
            {
                if let Ok(mut state) = self.agents_overview.view_state.lock() {
                    state.connection_notice = Some("Reconnecting — agent list is stale");
                }
                ReconnectPresentation::Overview
            } else {
                self.chat_widget
                    .handle_disconnected_key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE));
                ReconnectPresentation::Conversation
            };
            self.chat_widget.pause_for_disconnect();
            self.startup_pending_protected_request = false;
            self.abort_all_thread_event_listeners();
            for (_, (_, task)) in self.dynamic_tool_tasks.drain() {
                task.abort();
            }
        }
        true
    }

    pub(super) async fn finish_reconnect(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        app_event_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
        connected: Reconnected,
        client_version: &str,
    ) -> Result<()> {
        let Reconnected {
            mut session,
            bootstrap,
            thread,
        } = connected;
        let selected = self
            .chat_widget
            .selected_index_for_present_view(agents_overview::AGENTS_OVERVIEW_VIEW_ID)
            .and_then(|index| self.agents_overview.visible_thread_ids.get(index).copied());
        let displayed = self.current_displayed_thread_id();
        let mut input = self.chat_widget.capture_thread_input_state();
        if let Some(input) = input.as_mut() {
            input.recovered_queue = true;
        }
        self.store_active_thread_receiver().await;
        // Old request handles stay attached to the dead connection. Rotating the event channel
        // prevents their late completions from submitting follow-up operations on the new one.
        let (tx, rx) = mpsc::unbounded_channel();
        self.app_event_tx = AppEventSender::new(tx);
        *app_event_rx = rx;
        self.agent_navigation.picker_refresh = None;
        self.last_subagent_backfill_attempt = None;
        self.rate_limit_refresh_state.invalidate_recovery();
        session.inherit_task_tool_capabilities(app_server);
        *app_server = session;
        #[cfg(any(target_os = "windows", test))]
        let interrupted_windows_setup = self.windows_sandbox.pending_setup.take().is_some();
        #[cfg(any(target_os = "windows", test))]
        {
            if interrupted_windows_setup {
                self.windows_sandbox.setup_started_at = None;
                self.chat_widget.clear_windows_sandbox_setup_status();
                self.chat_widget.windows_sandbox_elevated_setup_complete = false;
            }
        }
        self.chat_widget.set_local_worktree_operations(
            !crate::uses_remote_workspace_or_environment(
                &self.app_server_target,
                self.environment_manager.as_ref(),
            ),
        );
        self.chat_widget.windows_sandbox_host = self.windows_sandbox_host();
        self.chat_widget.cyber_policy_notice = Default::default();
        self.chat_widget.requires_openai_auth = bootstrap.requires_openai_auth;
        self.chat_widget.remote_connection =
            crate::status::remote_connection::remote_connection_status_value(
                &self.app_server_target,
                app_server.server_version(),
            );
        self.workspace_command_runner = Some(Arc::new(AppServerWorkspaceCommandRunner::new(
            app_server.request_handle(),
        )));
        self.file_search =
            FileSearchManager::new(self.config.cwd.to_path_buf(), self.app_event_tx.clone());
        self.model_catalog = Arc::new(
            ModelCatalog::new(bootstrap.available_models)
                .with_collaboration_modes(bootstrap.collaboration_modes),
        );
        self.pending_app_server_requests.clear();
        let pending_displayed_profile =
            displayed.is_some_and(|id| self.pending_server_profiles.contains_key(&id));
        if pending_displayed_profile {
            self.runtime_approval_policy_override = None;
            self.runtime_permission_profile_override = None;
        }
        // The displayed task was resumed above. Keep offscreen selections pending until those
        // tasks can be resumed from the server too; their old confirmations cannot arrive.
        if let Some(id) = displayed {
            self.pending_server_profiles.remove(&id);
        }
        self.pending_primary_events.clear();
        self.pending_plugin_enabled_writes.clear();
        self.pending_hook_enabled_writes.clear();
        self.temporary_structured_requests.clear();
        self.pending_thread_titles.clear();
        self.sync_thread_title_progress();
        self.agents_overview.dispatched_requests.clear();
        self.agents_overview.request_id = None;
        self.agents_overview.refresh_pending = false;
        for input in self.agents_overview.input_states.values_mut() {
            input.recovered_queue = true;
        }
        self.pending_startup_thread_start = false;
        // Move cached UI state into fresh channels. Old producers retain the old sender/store,
        // so their late requests cannot leak into recovery. Background threads attach on selection.
        for channel in self.thread_event_channels.values_mut() {
            let mut replacement = ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY);
            if channel.attachment() == ThreadEventAttachment::ExternalWriter {
                replacement.mark_external_writer();
            } else {
                replacement.mark_replay_only();
            }
            let mut store = std::mem::replace(
                &mut *channel.store.lock().await,
                ThreadEventStore::new(THREAD_EVENT_CHANNEL_CAPACITY),
            );
            store.pending_interactive_replay = Default::default();
            store.pending_interrupt_turn_id = None;
            store.active = false;
            store
                .buffer
                .retain(|event| matches!(event, ThreadBufferedEvent::Notification(_)));
            if let Some(input) = store.input_state.as_mut() {
                input.recovered_queue = true;
            }
            *replacement.store.lock().await = store;
            *channel = replacement;
        }
        if let Some(id) = self.current_displayed_thread_id()
            && let Some(mut input) = input.clone()
        {
            input.recovered_queue = true;
            self.agents_overview.input_states.insert(id, input);
        }
        if let Some(mut started) = thread {
            let id = started.session.thread_id;
            if !pending_displayed_profile
                && let Some(channel) = self.thread_event_channels.get(&id)
                && let Some(cached) = channel.store.lock().await.session.as_ref()
            {
                self.restore_runtime_permissions(&mut started.session, cached);
            }
            self.agents_overview.input_states.remove(&id);
            if started
                .turns
                .iter()
                .any(|turn| turn.status == TurnStatus::InProgress)
            {
                self.agent_navigation.mark_running(id);
            } else {
                self.agent_navigation.mark_stopped(id);
            }
            if self.primary_thread_id == Some(id) {
                self.primary_session_configured = Some(started.session.clone());
            }
            if started.blocks_direct_input {
                self.agent_navigation.mark_parent_owned(id);
            }
            let channel = ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY);
            {
                let mut store = channel.store.lock().await;
                store.set_session(started.session, started.turns);
                store.input_state = input.clone();
            }
            self.thread_event_channels.insert(id, channel);
        }
        if let Some(id) = displayed
            && let Some((receiver, snapshot)) = self.activate_thread_for_replay(id).await
        {
            self.active_thread_id = Some(id);
            self.active_thread_rx = Some(receiver);
            self.recap.seed_from_turns(&snapshot.turns, Instant::now());
            self.render_thread_snapshot(
                tui, app_server, id, snapshot, /*resume_restored_queue*/ false,
            )?;
            self.config = self.chat_widget.config_ref().clone();
            self.refresh_pending_thread_approvals().await;
            if self.thread_unavailable(id) && !self.chat_widget.is_external_writer_view() {
                self.agent_navigation.mark_stopped(id);
                self.chat_widget.pause_unavailable_thread();
                self.chat_widget.add_info_message("This conversation is unavailable. Its cached transcript and draft remain here; input is paused. Open the agent picker or return to the parent to continue.".into(), /*hint*/ None);
            } else {
                self.schedule_recap_check(id, Instant::now());
            }
        } else {
            self.active_thread_id = None;
            self.active_thread_rx = None;
            self.primary_thread_id = None;
            self.primary_session_configured = None;
            let init = self.chatwidget_init_for_forked_or_resumed_thread(
                tui,
                self.config.clone(),
                /*initial_user_message*/ None,
            );
            self.replace_chat_widget(ChatWidget::new_with_app_event(init));
            self.chat_widget.restore_reconnected_input(input);
        }
        // Discover tasks whose notifications were missed, without clearing retained rows.
        // A hidden overview performs this discovery when it is next opened.
        self.agents_overview.initialized = false;
        if self.reconnect.presentation == ReconnectPresentation::Overview {
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.connection_notice = None;
            }
            let threads = self
                .agents_overview
                .threads
                .values()
                .flatten()
                .cloned()
                .collect();
            let view = self.agents_overview_view(threads, selected);
            self.chat_widget.show_bottom_pane_view(Box::new(view));
            self.refresh_agents_overview_threads(app_server);
        }
        #[cfg(any(target_os = "windows", test))]
        if interrupted_windows_setup {
            self.chat_widget.add_error_message(
                "Windows sandbox setup was interrupted. Restart Codex before using Agent mode."
                    .to_string(),
            );
        }
        // Only accept fresh task-tool calls once this connection and its event queue are adopted.
        if let ThreadToolTransport::Mcp(server) = app_server.thread_tool_transport() {
            server.reconnect(app_server.request_handle(), self.app_event_tx.clone());
        }
        self.reconnect.offline = false;
        self.chat_widget.update_account_state(
            bootstrap.status_account_display,
            bootstrap.plan_type,
            bootstrap.has_chatgpt_account,
            matches!(bootstrap.auth_mode, Some(TelemetryAuthMode::Chatgpt)),
        );
        if self.chat_widget.has_chatgpt_account() {
            crate::daybreak::prefetch_notice(
                &self.config,
                app_server,
                self.chat_widget.cyber_policy_notice.clone(),
            );
        }
        self.feedback_audience = bootstrap.feedback_audience;
        self.chat_widget.add_info_message(
            "Reconnected. No input was resent. Review uncertain submissions before retrying; recovered queues remain paused.".into(), /*hint*/ None,
        );
        let connected_notice_key = crate::status::remote_connection::server_version_notice_key(
            &self.app_server_target,
            app_server.server_codex_home(),
            client_version,
            app_server.server_version(),
        );
        if !self.local_settings.tui.show_server_version_notice
            || self.reconnect.seen_version_notice != connected_notice_key
        {
            self.reconnect.seen_version_notice = None;
            self.update_server_version_overview_notice(client_version, /*older_server*/ None);
        }
        if let Some((notice, key)) = crate::status::remote_connection::pending_server_version_notice(
            &self.local_settings.tui,
            &self.app_server_target,
            app_server.server_codex_home(),
            client_version,
            app_server.server_version(),
            self.reconnect.seen_version_notice.as_deref(),
        ) {
            self.reconnect.seen_version_notice = Some(key);
            self.update_server_version_overview_notice(client_version, app_server.server_version());
            if self.reconnect.presentation != ReconnectPresentation::Overview {
                self.chat_widget.add_server_version_warning(notice);
            }
        }
        Ok(())
    }
}
