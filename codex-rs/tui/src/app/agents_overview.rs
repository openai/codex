//! Daemon-wide overview of loaded root sessions and their subagents.

use super::agents_overview_view::AgentsOverviewGroup;
use super::agents_overview_view::AgentsOverviewRow;
use super::agents_overview_view::AgentsOverviewView;
use super::*;
use crate::chatwidget::ThreadInputStateRestoreMode;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_protocol::protocol::SubAgentSource;

pub(crate) const AGENTS_OVERVIEW_VIEW_ID: &str = "agents-overview";

#[derive(Default)]
pub(super) struct AgentsOverviewState {
    pub(super) request_id: Option<Uuid>,
    pub(super) refresh_pending: bool,
    pub(super) rendered_full_screen: bool,
    pub(super) visible_thread_ids: Vec<ThreadId>,
    pub(super) view_state:
        Arc<std::sync::Mutex<super::agents_overview_view::AgentsOverviewViewState>>,
    pub(super) input_states: HashMap<ThreadId, ThreadInputState>,
    pub(super) dispatched_requests: HashMap<ThreadId, Vec<ServerRequest>>,
}

impl App {
    pub(super) fn open_agents_overview(&mut self, app_server: &AppServerSession) {
        if matches!(self.app_server_target, AppServerTarget::Embedded) {
            self.chat_widget.add_info_message(
                "The shared agents dashboard requires a background app server.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        self.agents_overview.request_id = None;
        self.agents_overview.refresh_pending = false;
        let view = self.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        self.agents_overview.visible_thread_ids = view.thread_ids();
        self.chat_widget.show_bottom_pane_view(Box::new(view));
        self.refresh_agents_overview_threads(app_server);
    }

    pub(super) fn refresh_agents_overview_threads(&mut self, app_server: &AppServerSession) {
        if self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_none()
        {
            return;
        }
        if self.agents_overview.request_id.is_some() {
            self.agents_overview.refresh_pending = true;
            return;
        }

        let request_id = Uuid::new_v4();
        self.agents_overview.request_id = Some(request_id);
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let refresh_task = tokio::spawn(async move {
            let result = async {
                let mut threads = Vec::new();
                let mut cursor = None;
                while threads.len() < 1_000 {
                    let page = request_handle
                        .request_typed::<ThreadLoadedListResponse>(ClientRequest::ThreadLoadedList {
                            request_id: RequestId::String(Uuid::new_v4().to_string()),
                            params: ThreadLoadedListParams {
                                cursor,
                                limit: Some(100),
                            },
                        })
                        .await
                        .map_err(|error| error.to_string())?;
                    let mut loaded_threads = tokio::task::JoinSet::new();
                    for thread_id in page.data.into_iter().take(1_000 - threads.len()) {
                        let request_handle = request_handle.clone();
                        loaded_threads.spawn(async move {
                            match request_handle
                                .request_typed::<ThreadReadResponse>(ClientRequest::ThreadRead {
                                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                                    params: ThreadReadParams {
                                        thread_id: thread_id.clone(),
                                        include_turns: false,
                                    },
                                })
                                .await
                            {
                                Ok(mut response) => {
                                    if let Ok(turns) = request_handle
                                        .request_typed::<ThreadTurnsListResponse>(
                                            ClientRequest::ThreadTurnsList {
                                                request_id: RequestId::String(
                                                    Uuid::new_v4().to_string(),
                                                ),
                                                params: ThreadTurnsListParams {
                                                    thread_id,
                                                    cursor: None,
                                                    limit: Some(1),
                                                    sort_direction: None,
                                                    items_view: None,
                                                },
                                            },
                                        )
                                        .await
                                        && let Some(ThreadItem::UserMessage { content, .. }) = turns
                                            .data
                                            .first()
                                            .and_then(|turn| turn.items.first())
                                    {
                                        response.thread.preview =
                                            ChatWidget::user_message_display_from_inputs(content)
                                                .message;
                                    }
                                    Some(response.thread)
                                }
                                Err(error) => {
                                    tracing::warn!(thread_id, %error, "failed to read loaded agent thread");
                                    None
                                }
                            }
                        });
                        if loaded_threads.len() >= 16
                            && let Some(Ok(Some(thread))) = loaded_threads.join_next().await
                        {
                            threads.push(thread);
                        }
                    }
                    while let Some(result) = loaded_threads.join_next().await {
                        if let Ok(Some(thread)) = result {
                            threads.push(thread);
                        }
                    }
                    let Some(next_cursor) = page.next_cursor else {
                        break;
                    };
                    cursor = Some(next_cursor);
                }
                threads.sort_by_key(|thread| std::cmp::Reverse(thread.updated_at));
                Ok(threads)
            }
            .await;

            app_event_tx.send(AppEvent::AgentsOverviewThreadsLoaded { request_id, result });
        });
        if let Ok(mut state) = self.agents_overview.view_state.lock() {
            state.refresh_task = Some(refresh_task.abort_handle());
        }
    }

    pub(super) fn apply_agents_overview_thread_refresh(
        &mut self,
        app_server: &AppServerSession,
        request_id: Uuid,
        result: Result<Vec<Thread>, String>,
    ) {
        if self.agents_overview.request_id != Some(request_id) {
            return;
        }
        self.agents_overview.request_id = None;
        let Some(selected) = self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
        else {
            self.agents_overview.refresh_pending = false;
            return;
        };
        let selected_thread_id = self
            .agents_overview
            .visible_thread_ids
            .get(selected)
            .copied();
        let threads = match result {
            Ok(threads) => threads,
            Err(error) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load shared agents: {error}"));
                if std::mem::take(&mut self.agents_overview.refresh_pending) {
                    self.refresh_agents_overview_threads(app_server);
                }
                return;
            }
        };
        let view = self.agents_overview_view(threads, selected_thread_id);
        self.agents_overview.visible_thread_ids = view.thread_ids();
        if selected_thread_id
            .is_some_and(|thread_id| !self.agents_overview.visible_thread_ids.contains(&thread_id))
            && let Ok(mut state) = self.agents_overview.view_state.lock()
            && state.renaming
        {
            state.renaming = false;
            state.input.clear();
        }
        self.chat_widget
            .replace_bottom_pane_view_if_present(AGENTS_OVERVIEW_VIEW_ID, Box::new(view));
        if std::mem::take(&mut self.agents_overview.refresh_pending) {
            self.refresh_agents_overview_threads(app_server);
        }
    }

    fn agents_overview_view(
        &self,
        mut threads: Vec<Thread>,
        selected_thread_id: Option<ThreadId>,
    ) -> AgentsOverviewView {
        threads.retain(|thread| !thread.ephemeral);
        for thread in &mut threads {
            if thread.parent_thread_id.is_none()
                && let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id, ..
                }) = &thread.source
            {
                thread.parent_thread_id = Some(parent_thread_id.to_string());
            }
        }
        let mut children: HashMap<String, Vec<&Thread>> = HashMap::new();
        for thread in &threads {
            if let Some(parent_thread_id) = &thread.parent_thread_id {
                children
                    .entry(parent_thread_id.clone())
                    .or_default()
                    .push(thread);
            }
        }

        let mut roots = threads
            .iter()
            .filter(|thread| {
                thread.parent_thread_id.is_none()
                    && !matches!(thread.status, ThreadStatus::NotLoaded)
            })
            .map(|root| (root, agents_overview_group(root, &children)))
            .collect::<Vec<_>>();
        roots.sort_by_key(|(root, group)| (*group, std::cmp::Reverse(root.updated_at)));
        let mut rows = Vec::new();
        for (root, group) in roots {
            let Ok(thread_id) = ThreadId::from_string(&root.id) else {
                continue;
            };
            rows.push(AgentsOverviewRow {
                thread: root.clone(),
                thread_id,
                group,
                is_current: self.primary_thread_id == Some(thread_id),
            });
        }

        AgentsOverviewView::new(
            rows,
            selected_thread_id,
            self.primary_thread_id.is_none(),
            self.app_event_tx.clone(),
            self.keymap.clone(),
            Arc::clone(&self.agents_overview.view_state),
        )
    }

    pub(super) async fn select_agents_overview_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        root_thread_id: ThreadId,
    ) -> color_eyre::Result<AppRunControl> {
        if self.current_displayed_thread_id() == Some(root_thread_id) {
            return Ok(AppRunControl::Continue);
        }

        if self.primary_thread_id != Some(root_thread_id) {
            let previous_displayed_thread_id = self.current_displayed_thread_id();
            let mut previous_thread_ids =
                Vec::from_iter(self.thread_event_channels.keys().copied());
            previous_thread_ids.extend(
                self.agent_navigation
                    .tracked_thread_ids()
                    .into_iter()
                    .filter(|thread_id| !self.thread_event_channels.contains_key(thread_id)),
            );
            let mut previous_running_thread_ids = Vec::new();
            let mut previous_pending_requests = Vec::new();
            for thread_id in &previous_thread_ids {
                if self.side_threads.contains_key(thread_id) {
                    continue;
                }
                if let Some(channel) = self.thread_event_channels.get(thread_id) {
                    let store = channel.store.lock().await;
                    if store.active_turn_id().is_some() {
                        previous_running_thread_ids.push(*thread_id);
                    }
                    if let Some(input_state) = store.input_state.clone() {
                        self.agents_overview
                            .input_states
                            .insert(*thread_id, input_state);
                    }
                    let requests = store.pending_replay_requests();
                    if !requests.is_empty() {
                        previous_pending_requests.push((*thread_id, requests));
                    }
                } else if self
                    .agent_navigation
                    .get(thread_id)
                    .is_some_and(|entry| entry.is_running)
                {
                    previous_running_thread_ids.push(*thread_id);
                }
            }
            if let Some(active_thread_id) = self.current_displayed_thread_id()
                && let Some(input_state) = self.chat_widget.capture_thread_input_state()
            {
                self.agents_overview
                    .input_states
                    .insert(active_thread_id, input_state);
            }

            let target_thread = match app_server
                .thread_read(root_thread_id, /*include_turns*/ false)
                .await
            {
                Ok(thread) => thread,
                Err(error) => {
                    self.chat_widget.add_error_message(format!(
                        "Agent session {root_thread_id} is unavailable: {error}"
                    ));
                    return Ok(AppRunControl::Continue);
                }
            };
            let config_cwd = if self.app_server_target.uses_remote_workspace() {
                self.config.cwd.to_path_buf()
            } else {
                target_thread.cwd.to_path_buf()
            };
            let current_cwd = self.config.cwd.to_path_buf();
            let resume_config = match self
                .rebuild_config_for_resume_or_fallback(&current_cwd, config_cwd)
                .await
            {
                Ok(config) => config,
                Err(error) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to load task settings: {error}"));
                    return Ok(AppRunControl::Continue);
                }
            };
            let baseline_approval = resume_config.permissions.approval_policy.value();
            let baseline_permissions =
                RuntimePermissionProfileOverride::from_config(&resume_config);
            let resumed = match app_server
                .resume_thread(
                    resume_config.clone(),
                    root_thread_id,
                    crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
                )
                .await
            {
                Ok(resumed) => resumed,
                Err(error) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to attach to task: {error}"));
                    return Ok(AppRunControl::Continue);
                }
            };
            if !previous_running_thread_ids.is_empty() {
                for side_thread_id in Vec::from_iter(self.side_threads.keys().copied()) {
                    if !self.discard_side_thread(app_server, side_thread_id).await {
                        let _ = app_server.thread_unsubscribe(root_thread_id).await;
                        return Ok(AppRunControl::Continue);
                    }
                }
            }
            for (thread_id, requests) in previous_pending_requests {
                self.agents_overview
                    .dispatched_requests
                    .entry(thread_id)
                    .or_default()
                    .extend(requests);
            }
            if !previous_running_thread_ids.is_empty() {
                for thread_id in &previous_thread_ids {
                    if !self.side_threads.contains_key(thread_id) {
                        self.agents_overview
                            .dispatched_requests
                            .entry(*thread_id)
                            .or_default();
                    }
                }
            }
            if previous_running_thread_ids.is_empty() {
                self.shutdown_current_thread(app_server).await;
            }
            self.runtime_approval_policy_override = None;
            self.runtime_permission_profile_override = None;
            self.config = resume_config;
            tui.set_notification_settings(
                self.config.tui_notifications.method,
                self.config.tui_notifications.condition,
            );
            self.file_search
                .update_search_dir(self.config.cwd.to_path_buf());
            if let Err(error) = self
                .replace_chat_widget_with_app_server_thread(
                    tui,
                    resumed,
                    super::session_lifecycle::ThreadAttachPresentation::SessionLineage,
                    /*initial_user_message*/ None,
                )
                .await
            {
                self.chat_widget
                    .add_error_message(format!("Failed to attach to task: {error}"));
                return Ok(AppRunControl::Continue);
            }
            let mut destination_config = self.chat_widget.config_ref().clone();
            if self.app_server_target.uses_remote_workspace() {
                destination_config.cwd.clone_from(&self.config.cwd);
                destination_config
                    .workspace_roots
                    .clone_from(&self.config.workspace_roots);
                destination_config
                    .permissions
                    .set_workspace_roots(self.config.permissions.workspace_roots().to_vec());
            }
            self.config = destination_config;
            if self.config.permissions.approval_policy.value() != baseline_approval {
                self.runtime_approval_policy_override =
                    Some(self.config.permissions.approval_policy.value().into());
            }
            let destination_permissions =
                RuntimePermissionProfileOverride::from_config(&self.config);
            if destination_permissions != baseline_permissions {
                self.runtime_permission_profile_override = Some(destination_permissions);
            }
            if !self
                .backfill_loaded_subagent_threads(app_server)
                .await
                .completed
            {
                self.backfill_loaded_subagent_threads(app_server).await;
            }
            for thread_id in previous_thread_ids {
                if previous_running_thread_ids.is_empty()
                    && thread_id != root_thread_id
                    && Some(thread_id) != previous_displayed_thread_id
                    && let Err(error) = app_server.thread_unsubscribe(thread_id).await
                {
                    tracing::warn!(%thread_id, %error, "failed to unsubscribe previous agent thread");
                }
            }
        }

        if self.current_displayed_thread_id() != Some(root_thread_id) {
            self.select_agent_thread_and_discard_side(tui, app_server, root_thread_id)
                .await?;
        }
        self.replay_agents_overview_requests(app_server, root_thread_id)
            .await;
        if self.current_displayed_thread_id() == Some(root_thread_id)
            && let Some(input_state) = self.agents_overview.input_states.remove(&root_thread_id)
        {
            let preserve_in_flight_turn = self
                .active_turn_id_for_thread(root_thread_id)
                .await
                .is_some();
            self.chat_widget.restore_thread_input_state(
                Some(input_state),
                ThreadInputStateRestoreMode {
                    preserve_in_flight_turn,
                },
            );
            if !preserve_in_flight_turn {
                self.chat_widget.maybe_send_next_queued_input();
            }
        }
        self.maybe_prompt_resume_paused_goal_after_resume(app_server, root_thread_id)
            .await;

        Ok(AppRunControl::Continue)
    }

    pub(super) async fn replay_agents_overview_requests(
        &mut self,
        app_server: &AppServerSession,
        root_thread_id: ThreadId,
    ) {
        let requests = self
            .agents_overview
            .dispatched_requests
            .extract_if(|id, _| *id == root_thread_id || self.agent_navigation.get(id).is_some())
            .flat_map(|(_, requests)| requests)
            .collect::<Vec<_>>();
        for request in requests {
            self.handle_app_server_event(
                app_server,
                codex_app_server_client::AppServerEvent::ServerRequest(Box::new(request)),
            )
            .await;
        }
    }

    pub(super) async fn dispatch_agents_overview_task(
        &mut self,
        app_server: &mut AppServerSession,
        prompt: String,
        cwd: Option<AbsolutePathBuf>,
    ) {
        self.refresh_in_memory_config_from_disk_best_effort("starting a background task")
            .await;
        let remote_cwd = cwd
            .as_ref()
            .filter(|_| app_server.uses_remote_workspace())
            .map(AbsolutePathBuf::to_path_buf);
        let mut config = match cwd {
            Some(cwd) if app_server.uses_remote_workspace() => {
                let mut config = self.fresh_session_config();
                config.cwd = cwd;
                config
            }
            Some(cwd) => match self.rebuild_config_for_cwd(cwd.to_path_buf()).await {
                Ok(config) => config,
                Err(error) => {
                    if let Ok(mut state) = self.agents_overview.view_state.lock() {
                        state.input = prompt;
                    }
                    return self
                        .chat_widget
                        .add_error_message(format!("Failed to load project settings: {error}"));
                }
            },
            None => self.fresh_session_config(),
        };
        if let Some(profile) = self.runtime_permission_profile_override.as_ref()
            && profile.active_permission_profile.is_some()
            && (RuntimePermissionProfileOverride::from_config(&config) != *profile
                || config.permissions.profile_workspace_roots()
                    != self.config.permissions.profile_workspace_roots())
        {
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.input = prompt;
            }
            return self
                .chat_widget
                .add_error_message("Permission profile has different settings.".to_string());
        }
        self.apply_runtime_policy_overrides(&mut config);
        apply_managed_new_thread_defaults(
            &mut config,
            app_server.managed_new_thread_defaults(),
            &self.cli_kv_overrides,
            &self.harness_overrides,
        );
        match app_server
            .start_thread_with_session_start_source(
                &config,
                /*session_start_source*/ None,
                remote_cwd.as_deref(),
            )
            .await
        {
            Ok(started) => {
                let thread_id = started.session.thread_id;
                self.agents_overview
                    .dispatched_requests
                    .insert(thread_id, Vec::new());
                self.submit_agents_overview_prompt(app_server, thread_id, prompt)
                    .await;
            }
            Err(error) => {
                if let Ok(mut state) = self.agents_overview.view_state.lock() {
                    state.input = prompt;
                }
                self.chat_widget
                    .add_error_message(format!("Failed to start background task: {error}"));
            }
        }
    }

    pub(super) async fn submit_agents_overview_prompt(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        prompt: String,
    ) {
        let result = app_server
            .request_handle()
            .request_typed::<TurnStartResponse>(ClientRequest::TurnStart {
                request_id: RequestId::String(Uuid::new_v4().to_string()),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: vec![UserInput::Text {
                        text: prompt.clone(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
            })
            .await;
        if let Err(error) = result {
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.input = prompt;
            }
            self.chat_widget
                .add_error_message(format!("Failed to send task message: {error}"));
        }
    }

    pub(super) async fn stop_agents_overview_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let active_turn_id = match self.active_turn_id_for_thread(thread_id).await {
            Some(turn_id) => Some(turn_id),
            None => match app_server
                .thread_read(thread_id, /*include_turns*/ true)
                .await
            {
                Ok(thread) => thread
                    .turns
                    .into_iter()
                    .find(|turn| turn.status == TurnStatus::InProgress)
                    .map(|turn| turn.id),
                Err(error) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to stop background task: {error}"));
                    self.refresh_agents_overview_threads(app_server);
                    return;
                }
            },
        };
        let Some(turn_id) = active_turn_id else {
            return;
        };
        if let Err(error) = app_server.turn_interrupt(thread_id, turn_id).await {
            self.chat_widget
                .add_error_message(format!("Failed to stop background task: {error}"));
            self.refresh_agents_overview_threads(app_server);
        }
    }
}

#[cfg(test)]
#[path = "agents_overview_tests.rs"]
mod tests;

fn agents_overview_group(
    thread: &Thread,
    children: &HashMap<String, Vec<&Thread>>,
) -> AgentsOverviewGroup {
    children.get(&thread.id).into_iter().flatten().fold(
        AgentsOverviewGroup::for_status(&thread.status),
        |group, child| group.min(agents_overview_group(child, children)),
    )
}
