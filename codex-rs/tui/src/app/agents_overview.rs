//! Daemon-wide overview of recent and locally retained sessions and their subagents.
//! Tasks owned by another app server open as frozen, read-only history snapshots.

#[path = "agents_overview_new.rs"]
mod new;
pub(crate) use new::PendingWorktree;

#[path = "agents_overview_errors.rs"]
mod errors;

#[path = "agents_overview_loading.rs"]
mod loading;

use super::agents_overview_view::AgentsOverviewGroup;
use super::agents_overview_view::AgentsOverviewRow;
use super::agents_overview_view::AgentsOverviewView;
use super::*;
use crate::app_event::AgentsOverviewThreadRefresh;
use crate::bottom_pane::SelectionDescriptionLayout;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line_for_keymap;
use crate::chatwidget::ThreadInputStateRestoreMode;
use crate::startup_draft::StartupDraftPump;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_protocol::protocol::SubAgentSource;

pub(crate) const AGENTS_OVERVIEW_VIEW_ID: &str = "agents-overview";

#[derive(Default)]
pub(super) struct AgentsOverviewState {
    /// Missing metadata records a local resume until the next metadata refresh.
    pub(super) threads: HashMap<ThreadId, Option<Thread>>,
    /// Local visibility only; activity and metadata refreshes never reveal hidden roots.
    pub(super) hidden_threads: HashSet<ThreadId>,
    pub(super) last_messages: HashMap<ThreadId, String>,
    pub(super) usage: HashMap<ThreadId, super::agents_overview_usage::AgentsOverviewUsage>,
    pub(super) pending_usage: Option<(ThreadId, Uuid)>,
    pub(super) usage_disabled: bool,
    pub(super) activity: HashMap<ThreadId, super::agents_overview_details::AgentsOverviewActivity>,
    pub(super) initialized: bool,
    pub(super) request_id: Option<Uuid>,
    pub(super) refresh_pending: bool,
    pub(super) refresh_thread_ids: HashSet<ThreadId>,
    pub(super) refresh_task: Option<tokio::task::AbortHandle>,
    pub(super) refresh_notifications: HashMap<ThreadId, Vec<ServerNotification>>,
    pub(super) rendered_full_screen: bool,
    pub(super) visible_thread_ids: Vec<ThreadId>,
    pub(super) view_state:
        Arc<std::sync::Mutex<super::agents_overview_view::AgentsOverviewViewState>>,
    /// Explicit permission-profile choices for new-session carryover, retained across navigation.
    pub(super) selected_permission_profiles: HashMap<ThreadId, String>,
    /// Keep new tasks subscribed and reusable until a first turn makes them resumable.
    pub(super) blank_sessions: HashMap<ThreadId, crate::app_server_session::AppServerStartedThread>,
    pub(super) input_states: HashMap<ThreadId, ThreadInputState>,
    pub(super) new_session_draft: Option<Box<StartupDraftPump>>,
    pub(super) dispatched_requests: HashMap<ThreadId, Vec<ServerRequest>>,
}

impl Drop for AgentsOverviewState {
    fn drop(&mut self) {
        if let Some(task) = self.refresh_task.take() {
            task.abort();
        }
    }
}

impl App {
    pub(super) fn open_agents_overview(&mut self, app_server: &AppServerSession) {
        if matches!(self.app_server_target, AppServerTarget::Embedded) {
            let workload_identity_selected = codex_login::is_workload_identity_selected();
            self.chat_widget.show_selection_view(SelectionViewParams {
                title: Some("Shared agents unavailable".to_string()),
                subtitle: Some(
                    if workload_identity_selected {
                        "The agents dashboard is unavailable while workload identity is active."
                    } else if cfg!(any(unix, windows)) {
                        "This session isn’t connected to a shared background server."
                    } else {
                        "Connect to a remote background server to use the agents dashboard."
                    }
                    .to_string(),
                ),
                footer_note: (cfg!(any(unix, windows)) && !workload_identity_selected).then(|| {
                    Line::from(
                        "Starting a background server will not interrupt or move this session."
                            .dim(),
                    )
                }),
                footer_hint: Some(standard_popup_hint_line_for_keymap(&self.keymap.list)),
                items: [
                    #[cfg(any(unix, windows))]
                    (!workload_identity_selected).then(|| SelectionItem {
                        name: "Start background server".to_string(),
                        description: Some(
                            "Open `codex agents` in another terminal afterward.".to_string(),
                        ),
                        actions: vec![Box::new(|tx| tx.send(AppEvent::StartAgentsDaemon))],
                        dismiss_on_select: true,
                        ..Default::default()
                    }),
                    Some(SelectionItem {
                        name: "Return to this session".to_string(),
                        dismiss_on_select: true,
                        ..Default::default()
                    }),
                ]
                .into_iter()
                .flatten()
                .collect(),
                description_layout: SelectionDescriptionLayout::StackBelowWhenNarrow {
                    min_description_width: 28,
                },
                ..Default::default()
            });
            return;
        }

        let threads = self
            .agents_overview
            .threads
            .values()
            .flatten()
            .cloned()
            .collect();
        let view = self.agents_overview_view(threads, /*selected_thread_id*/ None);
        self.agents_overview.visible_thread_ids = view.thread_ids();
        self.chat_widget.show_bottom_pane_view(Box::new(view));
        self.refresh_agents_overview_threads(app_server);
    }

    pub(super) fn apply_agents_overview_thread_refresh(
        &mut self,
        app_server: &AppServerSession,
        request_id: Uuid,
        result: Result<AgentsOverviewThreadRefresh, String>,
    ) {
        if self.agents_overview.request_id != Some(request_id) {
            return;
        }
        self.agents_overview.request_id = None;
        self.agents_overview.refresh_task = None;
        self.agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .refresh_failed = !result
            .as_ref()
            .is_ok_and(|refresh| refresh.recent_seed_complete);
        match result {
            Ok(refresh) => {
                self.agents_overview.initialized = refresh.recent_seed_complete;
                self.agents_overview
                    .last_messages
                    .extend(refresh.last_messages);
                for (thread_id, thread) in refresh.threads {
                    if let Some(mut thread) = thread {
                        if thread.ephemeral {
                            self.agents_overview.threads.remove(&thread_id);
                            self.agents_overview.last_messages.remove(&thread_id);
                            self.agents_overview.activity.remove(&thread_id);
                            self.agents_overview.usage.remove(&thread_id);
                            continue;
                        }
                        thread.turns.clear();
                        self.agents_overview.threads.insert(thread_id, Some(thread));
                    } else {
                        self.agents_overview.threads.entry(thread_id).or_default();
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to refresh shared agents");
            }
        }
        for notifications in
            std::mem::take(&mut self.agents_overview.refresh_notifications).into_values()
        {
            for notification in notifications {
                if let ServerNotification::ThreadReverted(reverted) = &notification
                    && let Ok(thread_id) = ThreadId::from_string(&reverted.thread_id)
                {
                    // Discard stale read results without clearing activity received after the revert.
                    self.agents_overview.last_messages.remove(&thread_id);
                    if let Some(usage) = self.agents_overview.usage.get_mut(&thread_id) {
                        usage.tokens = None;
                    }
                    continue;
                }
                self.track_agents_overview_notification(&notification);
            }
        }
        if std::mem::take(&mut self.agents_overview.refresh_pending) {
            self.refresh_changed_agents_overview_threads(app_server);
        }
        self.repaint_agents_overview();
    }

    pub(super) fn repaint_agents_overview(&mut self) {
        let Some(selected) = self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
        else {
            return;
        };
        let selected_thread_id = self
            .agents_overview
            .visible_thread_ids
            .get(selected)
            .copied();
        let threads = self
            .agents_overview
            .threads
            .values()
            .flatten()
            .cloned()
            .collect();
        let view = self.agents_overview_view(threads, selected_thread_id);
        self.agents_overview.visible_thread_ids = view.thread_ids();
        if selected_thread_id
            .is_some_and(|thread_id| !self.agents_overview.visible_thread_ids.contains(&thread_id))
            && let Ok(mut state) = self.agents_overview.view_state.lock()
            && state.renaming
        {
            self.chat_widget.add_info_message(
                format!(
                    "The rename target disappeared. Unsubmitted title: {}",
                    state.input
                ),
                /*hint*/ None,
            );
            state.renaming = false;
            state.input.clear();
        }
        self.chat_widget
            .replace_bottom_pane_view_if_present(AGENTS_OVERVIEW_VIEW_ID, Box::new(view));
    }

    pub(super) fn agents_overview_view(
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
            .filter(|thread| thread.parent_thread_id.is_none())
            .map(|root| (root, agents_overview_group(root, &children)))
            .collect::<Vec<_>>();
        roots.sort_by(|(left, left_group), (right, right_group)| {
            left_group
                .cmp(right_group)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut rows = Vec::new();
        for (root, group) in roots {
            let Ok(thread_id) = ThreadId::from_string(&root.id) else {
                continue;
            };
            if self.agents_overview.hidden_threads.contains(&thread_id) {
                continue;
            }
            rows.push(AgentsOverviewRow {
                details: self.agents_overview_details(root, &children),
                thread: root.clone(),
                thread_id,
                group,
                is_current: self.primary_thread_id == Some(thread_id),
            });
        }

        AgentsOverviewView::new(
            rows,
            selected_thread_id,
            self.config.features.enabled(Feature::Worktrees)
                && !crate::uses_remote_workspace_or_environment(
                    &self.app_server_target,
                    self.environment_manager.as_ref(),
                ),
            self.local_settings.tui.status_line_use_colors,
            self.app_event_tx.clone(),
            self.keymap.clone(),
            Arc::clone(&self.agents_overview.view_state),
        )
    }

    pub(super) async fn select_agents_overview_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<AppRunControl> {
        Box::pin(self.attach_agents_overview_thread(
            tui, app_server, thread_id, /*started*/ None, /*startup_draft*/ None,
        ))
        .await
    }

    async fn attach_agents_overview_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        root_thread_id: ThreadId,
        started: Option<(Config, crate::app_server_session::AppServerStartedThread)>,
        mut startup_draft: Option<&mut StartupDraftPump>,
    ) -> color_eyre::Result<AppRunControl> {
        if self.windows_sandbox_blocks_thread_switch() {
            return Ok(AppRunControl::Continue);
        }
        let is_new_session = started.is_some();
        if self.current_displayed_thread_id() == Some(root_thread_id)
            && (!self.thread_unavailable(root_thread_id)
                || self.chat_widget.is_external_writer_view())
        {
            // Keep the displayed read-only snapshot frozen; R explicitly retries attachment.
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.completion = Some(crate::bottom_pane::ViewCompletion::Accepted);
            }
            self.chat_widget.pre_draw_tick();
            return Ok(AppRunControl::Continue);
        }
        if self.reject_pending_permission_root_switch() {
            return Ok(AppRunControl::Continue);
        }
        loading::draw(tui)?;
        if self.primary_thread_id != Some(root_thread_id) {
            let previous_displayed_thread_id = self.current_displayed_thread_id();
            if let Some(id) = previous_displayed_thread_id
                && let Some(blank) = self.agents_overview.blank_sessions.get_mut(&id)
                && let Some(channel) = self.thread_event_channels.get(&id)
                && let Some(session) = channel.store.lock().await.session.as_ref()
            {
                blank.session = session.clone();
            }
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

            let target_thread = match StartupDraftPump::run_with_optional_draft(
                startup_draft.as_deref_mut(),
                tui,
                app_server.thread_read(root_thread_id, /*include_turns*/ false),
            )
            .await
            {
                Ok(thread) => thread,
                Err(error) => {
                    self.add_agents_overview_error(format!(
                        "Agent session {root_thread_id} is unavailable: {error}"
                    ));
                    return Ok(AppRunControl::Continue);
                }
            };
            let unloaded = matches!(
                target_thread.status,
                codex_app_server_protocol::ThreadStatus::NotLoaded
            );
            let preserve_explicit_permissions = unloaded || started.is_some();
            let (mut resume_config, mut local_settings) = if let Some((config, _)) = &started {
                (
                    config.clone(),
                    crate::local_settings::LocalSettings::from(config),
                )
            } else if unloaded {
                let target_session = SessionTarget {
                    path: target_thread.path.clone(),
                    thread_id: root_thread_id,
                    cwd: Some(target_thread.cwd.to_path_buf()),
                    history_mode: Some(target_thread.history_mode),
                };
                match self
                    .resume_config_for_target(tui, app_server, &target_session)
                    .await
                {
                    Ok(config) => config,
                    Err(control) => return Ok(control),
                }
            } else {
                let config_cwd = if self.app_server_target.uses_remote_workspace() {
                    self.config.cwd.to_path_buf()
                } else {
                    target_thread.cwd.to_path_buf()
                };
                let current_cwd = self.config.cwd.to_path_buf();
                match self
                    .rebuild_config_for_resume_or_fallback(&current_cwd, config_cwd)
                    .await
                {
                    Ok(config) => config,
                    Err(error) => {
                        self.add_agents_overview_error(format!(
                            "Failed to load task settings: {error}"
                        ));
                        return Ok(AppRunControl::Continue);
                    }
                }
            };
            if !unloaded && started.is_none() {
                if let Err(control) = self
                    .confirm_directory_trust(
                        tui,
                        app_server,
                        &mut resume_config,
                        target_thread.cwd.as_path(),
                        Some(&target_thread),
                        /*startup_draft*/ None,
                    )
                    .await
                {
                    return Ok(control);
                }
                local_settings = crate::local_settings::LocalSettings::from(&resume_config);
            }
            // Folder selection and trust prompts can replace or clear the loading frame.
            loading::draw(tui)?;
            let baseline_approval = resume_config.permissions.approval_policy.value();
            let baseline_permissions =
                RuntimePermissionProfileOverride::from_config(&resume_config);
            let resume_model_settings = match target_thread.status {
                codex_app_server_protocol::ThreadStatus::NotLoaded => {
                    self.apply_runtime_policy_overrides(
                        &mut resume_config,
                        RuntimePolicyOverrideScope::ExplicitOnly,
                    );
                    if matches!(self.runtime_approval_policy_override,
                        Some(RuntimeApprovalPolicyOverride::Explicit(policy))
                            if policy.to_core() != resume_config.permissions.approval_policy.value())
                        || self
                            .runtime_permission_profile_override
                            .as_ref()
                            .is_some_and(|profile| {
                                profile.turn_override
                                    == RuntimePermissionProfileTurnOverride::LegacySandbox
                                    && !profile.matches_config(&resume_config)
                            })
                    {
                        self.add_agents_overview_error(
                            "Cannot resume task without preserving the selected permissions."
                                .to_string(),
                        );
                        return Ok(AppRunControl::Continue);
                    }
                    self.resume_model_settings()
                }
                codex_app_server_protocol::ThreadStatus::Idle
                | codex_app_server_protocol::ThreadStatus::Active { .. }
                | codex_app_server_protocol::ThreadStatus::SystemError => {
                    crate::app_server_session::ResumeModelSettings::PreserveExistingThread
                }
            };
            let mut history_notice = None;
            let (resumed, read_only) = if let Some((_, started)) = started {
                (started, false)
            } else if !unloaded
                && let Some(blank) = self.agents_overview.blank_sessions.get(&root_thread_id)
            {
                // An untouched task has no rollout for thread/resume yet. Its live
                // subscription and saved settings are sufficient to restore the editor.
                (blank.clone(), false)
            } else {
                match app_server
                    .resume_thread(
                        &local_settings,
                        resume_config.clone(),
                        root_thread_id,
                        resume_model_settings,
                    )
                    .await
                {
                    Ok(resumed) => (resumed, false),
                    Err(error) if crate::app_server_session::is_active_writer_error(&error) => {
                        match app_server
                            .read_thread_for_viewing(
                                &resume_config,
                                &local_settings,
                                root_thread_id,
                            )
                            .await
                        {
                            Ok((thread, notice)) => {
                                history_notice = notice;
                                (thread, true)
                            }
                            Err(_) => {
                                tracing::warn!("Failed to load read-only conversation history");
                                self.add_agents_overview_error(
                                    "Couldn't load this conversation. Please try again."
                                        .to_string(),
                                );
                                return Ok(AppRunControl::Continue);
                            }
                        }
                    }
                    Err(error) => {
                        self.add_agents_overview_error(format!(
                            "Failed to attach to task: {error}"
                        ));
                        return Ok(AppRunControl::Continue);
                    }
                }
            };
            if !previous_running_thread_ids.is_empty() {
                for side_thread_id in Vec::from_iter(self.side_threads.keys().copied()) {
                    let discarded = match startup_draft.as_deref_mut() {
                        Some(draft) => {
                            draft
                                .run_until(
                                    tui,
                                    self.discard_side_thread(app_server, side_thread_id),
                                )
                                .await?
                        }
                        None => self.discard_side_thread(app_server, side_thread_id).await,
                    };
                    if !discarded {
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
            if previous_running_thread_ids.is_empty()
                && !previous_displayed_thread_id
                    .is_some_and(|id| self.agents_overview.blank_sessions.contains_key(&id))
            {
                match startup_draft.as_deref_mut() {
                    Some(draft) => {
                        draft
                            .run_until(tui, self.shutdown_current_thread(app_server))
                            .await?
                    }
                    None => self.shutdown_current_thread(app_server).await,
                }
            }
            // Explicit choices carry across cold resumes and new sessions.
            self.runtime_approval_policy_override =
                self.runtime_approval_policy_override.filter(|policy| {
                    preserve_explicit_permissions
                        && matches!(policy, RuntimeApprovalPolicyOverride::Explicit(_))
                });
            self.runtime_permission_profile_override = self
                .runtime_permission_profile_override
                .take()
                .filter(|profile| {
                    preserve_explicit_permissions
                        && profile.turn_override
                            == RuntimePermissionProfileTurnOverride::LegacySandbox
                });
            self.local_settings = local_settings;
            self.refresh_server_version_overview_notice(CODEX_CLI_VERSION);
            self.config = resume_config;
            tui.set_notification_settings(
                self.local_settings.tui.notification_settings.method,
                self.local_settings.tui.notification_settings.condition,
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
                self.add_agents_overview_error(format!("Failed to attach to task: {error}"));
                return Ok(AppRunControl::Continue);
            }
            // Replacing the widget clears the terminal before the remaining server requests.
            loading::draw(tui)?;
            if read_only {
                self.ensure_thread_channel(root_thread_id)
                    .mark_external_writer();
                self.chat_widget.show_external_writer_thread();
                if let Some(notice) = history_notice {
                    self.chat_widget
                        .add_info_message(notice.to_string(), /*hint*/ None);
                }
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
            let approval = self.config.permissions.approval_policy.value();
            if self
                .runtime_approval_policy_override
                .is_none_or(|policy| policy.policy().to_core() != approval)
            {
                self.runtime_approval_policy_override = (approval != baseline_approval)
                    .then_some(RuntimeApprovalPolicyOverride::Restored(approval.into()));
            }
            if self
                .runtime_permission_profile_override
                .as_ref()
                .is_none_or(|profile| !profile.matches_config(&self.config))
            {
                self.runtime_permission_profile_override = (!baseline_permissions
                    .matches_config(&self.config))
                .then(|| RuntimePermissionProfileOverride::from_restored_config(&self.config));
            }
            // A new session has no descendants. Scanning every loaded thread here
            // adds a serial round trip per agent before the composer can render.
            if !is_new_session
                && !self
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
                    && !self.agents_overview.blank_sessions.contains_key(&thread_id)
                    && let Err(error) = StartupDraftPump::run_with_optional_draft(
                        startup_draft.as_deref_mut(),
                        tui,
                        app_server.thread_unsubscribe(thread_id),
                    )
                    .await
                {
                    tracing::warn!(%thread_id, %error, "failed to unsubscribe previous agent thread");
                }
            }
        }

        if self.current_displayed_thread_id() != Some(root_thread_id)
            || (self.thread_unavailable(root_thread_id)
                && !self.chat_widget.is_external_writer_view())
        {
            self.select_agent_thread_and_discard_side(tui, app_server, root_thread_id)
                .await?;
        }
        let read_only = self.chat_widget.is_external_writer_view();
        if !read_only {
            self.replay_agents_overview_requests(app_server, root_thread_id)
                .await;
        }
        if self.current_displayed_thread_id() == Some(root_thread_id)
            && let Some(input_state) = self.agents_overview.input_states.remove(&root_thread_id)
        {
            let preserve_in_flight_turn = !read_only
                && self
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
        if !read_only && !is_new_session {
            self.maybe_prompt_resume_paused_goal_after_resume(app_server, root_thread_id)
                .await;
        }

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

    async fn agents_overview_session_config(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: Option<AbsolutePathBuf>,
        mut startup_draft: Option<&mut StartupDraftPump>,
    ) -> Option<(Config, Option<PathBuf>)> {
        if self
            .chat_widget
            .thread_id()
            .is_some_and(|thread_id| self.pending_server_profiles.contains_key(&thread_id))
        {
            self.add_agents_overview_error(
                "Wait for permissions to update before starting a session.".into(),
            );
            return None;
        }
        let remote = app_server.uses_remote_workspace();
        let remote_cwd = cwd
            .as_ref()
            .filter(|_| remote)
            .map(AbsolutePathBuf::to_path_buf);
        let preserve_service_tier = remote || cwd.is_none();
        let local_cwd = if remote {
            self.config.cwd.to_path_buf()
        } else {
            cwd.map_or_else(
                || self.chat_widget.config_ref().cwd.to_path_buf(),
                |cwd| cwd.to_path_buf(),
            )
        };
        let mut config = match StartupDraftPump::run_with_optional_draft(
            startup_draft.as_deref_mut(),
            tui,
            self.rebuild_config_for_cwd(local_cwd),
        )
        .await
        {
            Ok(config) => config,
            Err(error) => {
                self.add_agents_overview_error(format!("Failed to load project settings: {error}"));
                return None;
            }
        };
        if preserve_service_tier {
            config.service_tier = self.chat_widget.configured_service_tier();
        }
        let trust_cwd = config.cwd.to_path_buf();
        if self
            .confirm_directory_trust(
                tui,
                app_server,
                &mut config,
                &trust_cwd,
                /*resumed_thread*/ None,
                startup_draft.as_deref_mut(),
            )
            .await
            .is_err()
        {
            return None;
        }
        if let Some(profile) = self.runtime_permission_profile_override.as_ref()
            && profile.turn_override == RuntimePermissionProfileTurnOverride::LegacySandbox
            && profile
                .active_permission_profile
                .as_ref()
                .is_some_and(|active| !active.id.starts_with(':'))
            && (!profile.matches_config(&config)
                || config.permissions.profile_workspace_roots()
                    != self.config.permissions.profile_workspace_roots())
        {
            self.add_agents_overview_error(
                "Permission profile has different settings.".to_string(),
            );
            return None;
        }
        // New sessions use the destination settings plus explicit user choices, not
        // a permission snapshot inherited when attaching to another task.
        self.apply_runtime_policy_overrides(&mut config, RuntimePolicyOverrideScope::ExplicitOnly);
        let defaults_cwd = match app_server.thread_params_mode() {
            crate::app_server_session::ThreadParamsMode::Embedded => config.cwd.as_path(),
            crate::app_server_session::ThreadParamsMode::Remote => remote_cwd
                .as_deref()
                .or_else(|| app_server.remote_cwd_override())
                .unwrap_or(Path::new(".")),
        };
        if let Some(draft) = startup_draft.as_deref_mut() {
            draft.apply_config(&config);
        }
        let mut server_model_cleared = false;
        match StartupDraftPump::run_with_optional_draft(
            startup_draft,
            tui,
            crate::config_update::read_effective_config_if_supported(
                app_server.request_handle(),
                defaults_cwd,
            ),
        )
        .await
        {
            Ok(Some(defaults)) => {
                server_model_cleared = defaults.model.is_none();
                let use_server_provider = matches!(
                    app_server.thread_params_mode(),
                    crate::app_server_session::ThreadParamsMode::Embedded
                ) && self.harness_overrides.model.is_none()
                    && !super::new_session::has_launch_setting(
                        &config,
                        &self.cli_kv_overrides,
                        "model",
                    )
                    && self.harness_overrides.model_provider.is_none()
                    && !super::new_session::has_launch_setting(
                        &config,
                        &self.cli_kv_overrides,
                        "model_provider",
                    );
                super::new_session::overlay_new_session_defaults(
                    &mut config,
                    &defaults,
                    &self.cli_kv_overrides,
                    &self.harness_overrides,
                );
                // Embedded thread/start sends a provider ID alongside the selected model.
                if use_server_provider {
                    config.model_provider_id = defaults
                        .model_provider
                        .unwrap_or_else(|| "openai".to_string());
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.add_agents_overview_error(format!(
                    "Failed to load new session settings: {error}"
                ));
                return None;
            }
        }
        apply_managed_new_thread_defaults(
            &mut config,
            app_server.managed_new_thread_defaults(),
            &self.cli_kv_overrides,
            &self.harness_overrides,
        );
        if server_model_cleared
            && config.model.is_none()
            && config.features.enabled(Feature::FastMode)
        {
            // Bootstrap's fallback model may be seeded from the client. Resolve tiers
            // against the server catalog when config/read cleared the model.
            config.model = self
                .model_catalog
                .models
                .iter()
                .find(|model| model.is_default)
                .or_else(|| self.model_catalog.models.first())
                .map(|model| model.model.clone());
        }
        Some((config, remote_cwd))
    }

    pub(super) async fn stop_agents_overview_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let active_turn_id = match self.active_turn_id_for_thread(thread_id).await {
            Some(turn_id) => Some(turn_id),
            None => match async {
                let thread = app_server
                    .thread_read(thread_id, /*include_turns*/ false)
                    .await?;
                let turns = match thread.history_mode {
                    ThreadHistoryMode::Paginated if app_server.supports_paginated_history() => {
                        app_server
                            .thread_turns_page(thread_id, /*cursor*/ None)
                            .await?
                            .data
                    }
                    ThreadHistoryMode::Legacy | ThreadHistoryMode::Paginated => {
                        app_server
                            .thread_read(thread_id, /*include_turns*/ true)
                            .await?
                            .turns
                    }
                };
                Ok::<_, color_eyre::Report>(
                    turns
                        .into_iter()
                        .find(|turn| turn.status == TurnStatus::InProgress)
                        .map(|turn| turn.id),
                )
            }
            .await
            {
                Ok(turn_id) => turn_id,
                Err(error) => {
                    self.add_agents_overview_error(format!(
                        "Failed to stop background task: {error}"
                    ));
                    self.refresh_agents_overview_threads(app_server);
                    return;
                }
            },
        };
        let Some(turn_id) = active_turn_id else {
            return;
        };
        if let Err(error) = app_server.turn_interrupt(thread_id, turn_id).await {
            self.add_agents_overview_error(format!("Failed to stop background task: {error}"));
            self.refresh_agents_overview_threads(app_server);
        }
    }

    #[cfg(any(unix, windows))]
    pub(super) fn start_agents_daemon(&self) {
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = async {
                let current_executable =
                    std::env::current_exe().map_err(|error| error.to_string())?;
                let executable = if current_executable
                    .file_stem()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|name| {
                        if cfg!(windows) {
                            name.eq_ignore_ascii_case("codex-tui")
                        } else {
                            name == "codex-tui"
                        }
                    }) {
                    current_executable.with_file_name(if cfg!(windows) {
                        "codex.exe"
                    } else {
                        "codex"
                    })
                } else {
                    current_executable
                };
                let output = tokio::process::Command::new(executable)
                    .args(["app-server", "daemon", "start"])
                    .output()
                    .await
                    .map_err(|error| error.to_string())?;
                if output.status.success() {
                    Ok(())
                } else {
                    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(if message.is_empty() {
                        format!("daemon process exited with {}", output.status)
                    } else {
                        message
                    })
                }
            }
            .await;

            app_event_tx.send(AppEvent::AgentsDaemonStarted { result });
        });
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
