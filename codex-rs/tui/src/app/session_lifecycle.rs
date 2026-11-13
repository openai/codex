//! Session, resume, fork, and subagent selection lifecycle for the TUI app.
//!
//! This module owns the high-level transitions between app-server threads: starting fresh sessions,
//! resuming/forking saved sessions, replacing ChatWidget instances, and maintaining the agent picker
//! cache used for multi-agent navigation.

use super::agent_picker::AGENT_PICKER_VIEW_ID;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use super::app_server_event_targets::server_request_thread_id;
use super::thread_attachment::ChannelMode;
use super::thread_attachment::PreparedAttachment;
use super::thread_attachment::Selection;
use super::thread_attachment::SelectionMode;
use super::*;
use crate::app_server_session::SessionAccount;
use crate::app_server_session::SideChatStorage;
use crate::app_server_session::is_server_shutting_down_error;
use crate::app_server_session::source_agent_path;
use crate::app_server_session::thread_blocks_direct_input;
use crate::chatwidget::ThreadInputStateRestoreMode;
use crate::streaming::commit_tick::CommitTickScope;
use codex_app_server_protocol::ThreadPrepareApprovedPlanParams;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub(super) enum ThreadAttachPresentation {
    /// A primary thread created by thread/start, without inherited history.
    Fresh,
    /// A fresh thread whose terminal clear succeeded before its backend was adopted.
    FreshAfterClear,
    /// A fresh thread whose startup composer is already visible.
    FreshWithDraft,
    SessionLineage,
    /// A resume whose outgoing events were drained and terminal cleared before capture.
    SessionLineageAfterClear,
}

pub(super) struct PreparedFreshSession {
    pub(super) started: AppServerStartedThread,
    pub(super) config: Config,
    pub(super) local_settings: crate::local_settings::LocalSettings,
}

/// Reports whether a loaded-thread backfill completed and which descendants already had their
/// liveness metadata refreshed, allowing the picker to skip duplicate `thread/read` requests.
#[derive(Default)]
pub(super) struct LoadedSubagentBackfill {
    pub(super) completed: bool,
    pub(super) refreshed_thread_ids: HashSet<ThreadId>,
}

impl App {
    pub(super) async fn open_agent_picker(&mut self, app_server: &mut AppServerSession) {
        let backfill = if self.primary_thread_id.is_none() {
            self.backfill_loaded_subagent_threads(app_server).await
        } else {
            LoadedSubagentBackfill::default()
        };
        // V2 subagents are identified by canonical paths observed from activity events or loaded
        // thread metadata. A buffered active turn is positive liveness evidence; a completed
        // snapshot is terminal evidence. An empty store does not clear a successful spawn hint.
        let path_backed_thread_ids: Vec<_> = self
            .agent_navigation
            .ordered_path_backed_subagent_threads(self.primary_thread_id)
            .into_iter()
            .map(|(thread_id, _)| thread_id)
            .collect();
        for thread_id in path_backed_thread_ids.iter().copied() {
            if let Some(channel) = self.thread_event_channels.get(&thread_id)
                && channel.attachment() == ThreadEventAttachment::Live
            {
                let Ok(store) = channel.store.try_lock() else {
                    continue;
                };
                let has_active_turn = store.active_turn_id().is_some();
                let has_terminal_snapshot = store
                    .turns
                    .last()
                    .is_some_and(|turn| !matches!(turn.status, TurnStatus::InProgress));
                drop(store);
                if has_active_turn {
                    self.agent_navigation.mark_running(thread_id);
                } else if has_terminal_snapshot {
                    self.agent_navigation.mark_stopped(thread_id);
                }
            } else if self.primary_thread_id.is_none()
                && !backfill.refreshed_thread_ids.contains(&thread_id)
            {
                self.refresh_agent_picker_thread_liveness(app_server, thread_id)
                    .await;
            }
        }
        let path_backed_threads = self
            .agent_navigation
            .ordered_path_backed_subagent_threads(self.primary_thread_id);
        if !path_backed_threads.is_empty() {
            let running_threads: Vec<_> = path_backed_threads
                .into_iter()
                .filter_map(|(thread_id, entry)| {
                    if !entry.is_running || entry.is_closed {
                        return None;
                    }
                    Some((thread_id, entry.agent_path.as_deref()?.trim().to_string()))
                })
                .collect();
            let mut entries = Vec::new();
            for (thread_id, agent_path) in running_threads {
                let preview = if let Some(channel) = self.thread_event_channels.get(&thread_id) {
                    match channel.store.try_lock() {
                        Ok(store) => {
                            super::agent_status_feed::AgentStatusThreadPreview::from_store(
                                agent_path, &store,
                            )
                        }
                        Err(_) => {
                            super::agent_status_feed::AgentStatusThreadPreview::empty(agent_path)
                        }
                    }
                } else {
                    super::agent_status_feed::AgentStatusThreadPreview::empty(agent_path)
                };
                entries.push(preview);
            }

            self.chat_widget
                .add_to_history(super::agent_status_feed::AgentStatusHistoryCell::new(
                    entries,
                ));
        }

        let mut thread_ids = self.agent_navigation.tracked_thread_ids();
        for thread_id in self.thread_event_channels.keys().copied() {
            if !thread_ids.contains(&thread_id) {
                thread_ids.push(thread_id);
            }
        }
        for thread_id in thread_ids {
            if path_backed_thread_ids.contains(&thread_id)
                || self.side_threads.contains_key(&thread_id)
                || backfill.refreshed_thread_ids.contains(&thread_id)
            {
                continue;
            }
            if self.primary_thread_id.is_none() {
                self.refresh_agent_picker_thread_liveness(app_server, thread_id)
                    .await;
            } else if self.agent_navigation.get(&thread_id).is_none() {
                self.upsert_agent_picker_thread(
                    thread_id, /*agent_nickname*/ None, /*agent_role*/ None,
                    /*is_closed*/ false,
                );
            }
        }

        self.sync_active_agent_label();
        let has_non_primary_agent_thread = self
            .agent_navigation
            .has_non_primary_thread(self.primary_thread_id);
        if !self.config.features.enabled(Feature::Collab) && !has_non_primary_agent_thread {
            if let Some(primary_thread_id) = self.primary_thread_id {
                self.refresh_agent_picker_threads(app_server, primary_thread_id);
            }
            self.chat_widget.open_feature_enable_prompt(Feature::Collab);
            return;
        }

        if self.agent_navigation.is_empty() {
            self.chat_widget
                .add_info_message("No agents available yet.".to_string(), /*hint*/ None);
            return;
        }

        if !self.refresh_agent_picker() {
            let params = self.agent_picker_selection_view_params(/*selected*/ None);
            self.chat_widget.show_selection_view(params);
        }
        if let Some(primary_thread_id) = self.primary_thread_id {
            self.refresh_agent_picker_threads(app_server, primary_thread_id);
        }
    }

    pub(super) fn agent_picker_selection_view_params(
        &mut self,
        selected: Option<ThreadId>,
    ) -> SelectionViewParams {
        let mut threads = self.agent_navigation.visible_threads();
        let pending = &self.agent_navigation.pending_approvals;
        threads.sort_by_key(|(id, _)| !pending.contains(id));
        let mut row_ids = Vec::with_capacity(threads.len());
        let mut initial_selected_idx = None;
        let items: Vec<SelectionItem> = threads
            .into_iter()
            .enumerate()
            .map(|(idx, (thread_id, entry))| {
                row_ids.push(thread_id);
                let is_pending = pending.contains(&thread_id);
                let is_current = self.active_thread_id == Some(thread_id);
                if selected == Some(thread_id)
                    || initial_selected_idx.is_none() && (is_pending || is_current)
                {
                    initial_selected_idx = Some(idx);
                }
                let name = self.thread_label(thread_id);
                let uuid = thread_id.to_string();
                SelectionItem {
                    name: if is_pending {
                        format!("{name} (needs approval)")
                    } else {
                        name.clone()
                    },
                    name_prefix_spans: agent_picker_status_dot_spans(entry.is_closed),
                    description: Some(uuid.clone()),
                    is_current,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SelectAgentThread(thread_id));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!("{name} {uuid}")),
                    ..Default::default()
                }
            })
            .collect();
        self.agent_navigation.picker_row_ids = row_ids;

        SelectionViewParams {
            view_id: Some(AGENT_PICKER_VIEW_ID),
            title: Some("Subagents".to_string()),
            subtitle: Some(AgentNavigationState::picker_subtitle()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..SelectionViewParams::picker()
        }
    }

    pub(super) fn is_terminal_thread_read_error(err: &color_eyre::Report) -> bool {
        err.chain()
            .any(|cause| cause.to_string().contains("thread not loaded:"))
    }

    pub(super) fn closed_state_for_thread_read_error(
        err: &color_eyre::Report,
        existing_is_closed: Option<bool>,
    ) -> bool {
        Self::is_terminal_thread_read_error(err) || existing_is_closed.unwrap_or(false)
    }

    pub(super) fn can_fallback_from_include_turns_error(err: &color_eyre::Report) -> bool {
        err.chain().any(|cause| {
            let message = cause.to_string();
            message.contains("includeTurns is unavailable before first user message")
                || message.contains("thread/turns/list is unavailable before first user message")
                || message.contains("ephemeral threads do not support includeTurns")
        })
    }

    /// Cache metadata in both the picker and transcript. Callers refresh the UI after a batch.
    pub(super) fn upsert_agent_picker_thread(
        &mut self,
        thread_id: ThreadId,
        agent_nickname: Option<String>,
        agent_role: Option<String>,
        is_closed: bool,
    ) {
        self.chat_widget.set_collab_agent_metadata(
            thread_id,
            agent_nickname.clone(),
            agent_role.clone(),
        );
        self.agent_navigation
            .upsert(thread_id, agent_nickname, agent_role, is_closed);
    }

    pub(super) fn sync_agent_picker_thread(&mut self, thread_id: ThreadId) {
        if self.agent_navigation.pending_approvals.contains(&thread_id) {
            self.refresh_agent_picker();
        } else {
            self.sync_active_agent_label();
        }
    }

    /// Persists the app-server's authoritative ownership flag and updates the active composer.
    pub(super) fn mark_primary_thread_parent_owned(&mut self, thread_id: ThreadId) {
        self.agent_navigation.mark_parent_owned(thread_id);
        self.chat_widget.set_parent_owned_thread();
    }

    /// Marks a cached picker thread closed and recomputes the contextual footer label.
    ///
    /// Closing a thread is not the same as removing it: users can still inspect finished agent
    /// transcripts, and the stable next/previous traversal order should not collapse around them.
    pub(super) fn mark_agent_picker_thread_closed(&mut self, thread_id: ThreadId) {
        self.agent_navigation.mark_closed(thread_id);
        self.sync_active_agent_label();
    }

    pub(super) async fn refresh_agent_picker_thread_liveness(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> bool {
        let existing_entry = self.agent_navigation.get(&thread_id).cloned();
        let has_replay_channel = self.thread_event_channels.contains_key(&thread_id);
        match app_server
            .thread_read(thread_id, /*include_turns*/ false)
            .await
        {
            Ok(thread) => {
                let is_parent_owned = thread_blocks_direct_input(&thread);
                let agent_path = source_agent_path(&thread.source);
                let is_running = matches!(
                    thread.status,
                    codex_app_server_protocol::ThreadStatus::Active { .. }
                );
                let is_closed = matches!(
                    thread.status,
                    codex_app_server_protocol::ThreadStatus::NotLoaded
                );
                self.upsert_agent_picker_thread(
                    thread_id,
                    thread.agent_nickname.or_else(|| {
                        existing_entry
                            .as_ref()
                            .and_then(|entry| entry.agent_nickname.clone())
                    }),
                    thread.agent_role.or_else(|| {
                        existing_entry
                            .as_ref()
                            .and_then(|entry| entry.agent_role.clone())
                    }),
                    is_closed,
                );
                if is_parent_owned {
                    self.agent_navigation.mark_parent_owned(thread_id);
                }
                self.agent_navigation.set_agent_path(thread_id, agent_path);
                self.sync_agent_picker_thread(thread_id);
                if is_running {
                    self.agent_navigation.mark_running(thread_id);
                } else {
                    self.agent_navigation
                        .set_running(thread_id, /*is_running*/ false);
                }
                true
            }
            Err(err) => {
                if Self::is_terminal_thread_read_error(&err) && !has_replay_channel {
                    self.agent_navigation.remove(thread_id);
                    return false;
                }
                let is_closed = Self::closed_state_for_thread_read_error(
                    &err,
                    existing_entry.as_ref().map(|entry| entry.is_closed),
                );
                if let Some(entry) = existing_entry {
                    self.upsert_agent_picker_thread(
                        thread_id,
                        entry.agent_nickname,
                        entry.agent_role,
                        is_closed,
                    );
                } else {
                    self.upsert_agent_picker_thread(
                        thread_id, /*agent_nickname*/ None, /*agent_role*/ None,
                        is_closed,
                    );
                }
                self.agent_navigation
                    .set_running(thread_id, /*is_running*/ false);
                self.sync_agent_picker_thread(thread_id);
                true
            }
        }
    }

    /// Materializes a live thread into local replay state when the picker knows about it but the
    /// TUI has not cached a local event channel yet.
    ///
    /// Resume-time backfill intentionally avoids creating empty placeholder channels, because those
    /// placeholders make stale `/subagents` entries open blank transcripts. When a user later
    /// selects a still-live discovered thread, attach it on demand with a real resumed snapshot.
    pub(super) async fn attach_live_thread_for_selection(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<bool> {
        if !self.should_attach_live_thread_for_selection(thread_id) {
            return Ok(true);
        }

        use crate::app_server_session::ResumeModelSettings;
        let side = self.side_threads.get(&thread_id);
        let resume_settings = if side.is_some_and(|side| side.storage == SideChatStorage::Attached)
            && self
                .agent_navigation
                .get(&thread_id)
                .is_some_and(|entry| entry.is_closed)
        {
            ResumeModelSettings::PreserveSideConversation
        } else {
            ResumeModelSettings::PreserveExistingThread
        };
        let prepared = super::thread_attachment::prepare(
            app_server,
            &self.local_settings,
            &self.config,
            thread_id,
            resume_settings,
        )
        .await?;
        Ok(self
            .install_thread_attachment(app_server, thread_id, prepared, ChannelMode::ReplaceLive)
            .await)
    }

    pub(super) async fn install_thread_attachment(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        prepared: PreparedAttachment,
        channel_mode: ChannelMode<'_>,
    ) -> bool {
        let incoming = match channel_mode {
            ChannelMode::ReplaceLive => None,
            ChannelMode::Reuse(updates) => Some(updates),
        };
        let closed = incoming
            .is_some_and(|updates| updates.close == super::thread_attachment::CloseState::Closed);
        let was_external_writer = self
            .thread_event_channels
            .get(&thread_id)
            .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::ExternalWriter);
        let (mut session, turns, live_attached) = match prepared {
            PreparedAttachment::Live(started) => {
                if let Some(entry) = self.agent_navigation.get(&thread_id).cloned() {
                    self.upsert_agent_picker_thread(
                        thread_id,
                        entry.agent_nickname,
                        entry.agent_role,
                        closed,
                    );
                    self.sync_agent_picker_thread(thread_id);
                }
                if started.blocks_direct_input {
                    self.agent_navigation.mark_parent_owned(thread_id);
                }
                (started.session, started.turns, true)
            }
            PreparedAttachment::ReplayOnly(thread) => {
                let mut session = self.session_state_for_thread_read(thread_id, &thread).await;
                // Reads have no settings. Keep this cached thread's permissions rather than
                // inferring them from the conversation that is currently displayed.
                if let Some(channel) = self.thread_event_channels.get(&thread_id)
                    && let Some(cached) = channel.store.lock().await.session.as_ref()
                {
                    session.approval_policy = cached.approval_policy;
                    session.permission_profile = cached.permission_profile.clone();
                    session.active_permission_profile = cached.active_permission_profile.clone();
                    session.approvals_reviewer = cached.approvals_reviewer;
                }
                // `thread/read` can seed replay state, but it does not attach the app-server
                // listener that `thread/resume` establishes, so treat this path as replay-only.
                session.model.clear();
                if was_external_writer {
                    app_server.freeze_viewed_receipt(thread_id);
                }
                (session, thread.turns, false)
            }
        };
        let live_attached = live_attached && !closed;
        if self.side_threads.contains_key(&thread_id) {
            session.forked_from_id = None;
            session.fork_parent_title = None;
            session.daybreak_enabled = false;
        }
        // A reattach can discover output missed while the server was disconnected.
        let replay_may_have_output = if self.side_companions.contains_key(&thread_id) {
            match self.thread_event_channels.get(&thread_id) {
                Some(channel) => {
                    let store = channel.store.lock().await;
                    store.replay_may_have_output || store.session.is_none() || store.turns != turns
                }
                None => true,
            }
        } else {
            false
        };
        self.agents_overview.activity.remove(&thread_id);
        let recap_progress = if incoming.is_none()
            && live_attached
            && let Some(channel) = self.thread_event_channels.remove(&thread_id)
        {
            let store = channel.store.lock().await;
            if let Some(input) = store.input_state.clone() {
                self.agents_overview.input_states.insert(thread_id, input);
            }
            store.recap_progress()
        } else {
            Default::default()
        };
        let channel = self.ensure_thread_channel(thread_id);
        if live_attached {
            channel.mark_live();
        } else {
            if was_external_writer {
                channel.mark_external_writer();
            } else {
                channel.mark_replay_only();
            }
        }
        let mut store = channel.store.lock().await;
        if let Some(settings) = incoming.and_then(|updates| updates.settings.as_ref()) {
            super::thread_settings::apply_thread_settings_to_session(&mut session, settings);
        }
        let live_turn = incoming.filter(|updates| updates.turn_changed).map(|_| {
            (
                store.active_turn_id.clone(),
                store.latest_turn_id.clone(),
                store.evicted_active_turn.take(),
                store.active_reasoning_item.clone(),
            )
        });
        store.set_session(session, turns);
        if let Some((active, latest, evicted, reasoning)) = live_turn {
            store.active_turn_id = active;
            store.latest_turn_id = latest;
            store.evicted_active_turn = evicted;
            store.active_reasoning_item = reasoning;
        }
        if closed {
            store.clear_active_turn_id();
            store.active_reasoning_item = None;
            store.pending_interrupt_turn_id = None;
        }
        store.replay_may_have_output |= replay_may_have_output;
        store.merge_recap_progress(recap_progress);
        store.rebase_buffer_after_session_refresh();
        live_attached
    }

    /// Replaces the chat widget, carrying composer preferences and the editor yank, and
    /// re-seeding collab metadata from the navigation cache.
    ///
    /// Thread switches reconstruct the `ChatWidget`, which loses the `collab_agent_metadata` map.
    /// This helper copies every known nickname/role from `AgentNavigationState` into the
    /// replacement widget so that replayed collab items render agent names immediately.
    pub(super) fn replace_chat_widget(&mut self, mut chat_widget: ChatWidget) {
        chat_widget.inherit_view_transition_key_guard(&mut self.chat_widget);
        chat_widget.fork_in_progress = self.chat_widget.fork_in_progress;
        self.pending_right_click_paste = None;
        self.config_notification_test_pending = None;
        if !self.chat_widget.realtime_conversation_is_running() {
            self.retain_realtime_replay_state_before_replace();
        }
        self.commit_animation = None;
        // Transfer the last-written terminal title to the replacement widget
        // so it knows what OSC title is currently displayed. Without this, the
        // new widget would redundantly clear and rewrite the same title, causing
        // a visible flicker in some terminals.
        let previous_terminal_title = self.chat_widget.last_terminal_title.take();
        if chat_widget.last_terminal_title.is_none() {
            chat_widget.last_terminal_title = previous_terminal_title;
        }
        chat_widget.config_panel_tui = self.chat_widget.config_panel_tui.clone();
        chat_widget.remote_connection = self.chat_widget.remote_connection.clone();
        chat_widget.snapshot_local_images = self.app_server_target.uses_remote_workspace();
        chat_widget.thread_settings_owner = self.chat_widget.thread_settings_owner;
        chat_widget.pending_cloud_turn_settings =
            self.chat_widget.pending_cloud_turn_settings.clone();
        chat_widget.set_local_worktree_operations(self.chat_widget.local_worktree_operations);
        chat_widget.windows_sandbox_local_server = self.chat_widget.windows_sandbox_local_server;
        chat_widget.windows_sandbox_host = WindowsSandboxHost::Unknown;
        #[cfg(any(target_os = "windows", test))]
        {
            chat_widget.windows_sandbox_elevated_setup_complete =
                self.chat_widget.windows_sandbox_elevated_setup_complete;
        }
        if chat_widget.status_account_display() == self.chat_widget.status_account_display()
            && chat_widget.has_chatgpt_account() == self.chat_widget.has_chatgpt_account()
        {
            chat_widget.inherit_backend_banner_state(&mut self.chat_widget);
            chat_widget.inherit_security_setup(&mut self.chat_widget);
        }
        chat_widget
            .mention_usage_mut()
            .inherit(self.chat_widget.mention_usage_mut());
        for (thread_id, entry) in self.agent_navigation.ordered_threads() {
            chat_widget.set_collab_agent_metadata(
                thread_id,
                entry.agent_nickname.clone(),
                entry.agent_role.clone(),
            );
        }
        chat_widget.restore_kill_buffer_snapshot(self.chat_widget.take_kill_buffer_snapshot());
        crate::markdown_render::preferences::init(chat_widget.local_settings.tui.rendering);
        let mut previous = std::mem::replace(&mut self.chat_widget, chat_widget);
        if previous.realtime_conversation_is_running() {
            previous.park_voice();
            self.background_voice = Some(Box::new(previous));
        }
        self.refresh_agent_picker();
    }

    pub(super) async fn select_agent_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<()> {
        if self
            .side_threads
            .get(&thread_id)
            .is_some_and(|side| side.close_request.is_some())
        {
            self.chat_widget.add_info_message(
                "Side chat is still closing. Try again in a moment.".to_string(),
                /*hint*/ None,
            );
            return Ok(());
        }

        let pending_profile_on_unavailable_thread = self.thread_unavailable(thread_id)
            && self.pending_server_profiles.contains_key(&thread_id);
        let cached_session = if self.thread_unavailable(thread_id) {
            self.thread_event_channels[&thread_id]
                .store
                .lock()
                .await
                .session
                .clone()
        } else {
            None
        };
        if self.active_thread_id == Some(thread_id) && !self.thread_unavailable(thread_id) {
            return Ok(());
        }
        if !self.can_select_thread() {
            return Ok(());
        }
        if self.active_thread_id == Some(thread_id) {
            // Detach the cached receiver before a successful attachment replaces its channel.
            self.store_active_thread_receiver().await;
        }

        let known_side = self.side_threads.contains_key(&thread_id)
            && self.thread_event_channels.contains_key(&thread_id);
        // The live parent already receives lifecycle notifications while its child has focus.
        let live_parent = self.side_retention_enabled(tui)
            && !self.reconnect.offline
            && self.primary_thread_id == Some(thread_id)
            && self.active_side_parent_thread_id() == Some(thread_id)
            && !self.should_attach_live_thread_for_selection(thread_id);
        if !(known_side
            || live_parent
            || self
                .refresh_agent_picker_thread_liveness(app_server, thread_id)
                .await)
        {
            self.add_agents_overview_error(format!(
                "Agent thread {thread_id} is no longer available."
            ));
            return Ok(());
        }
        let mode = if self.should_attach_live_thread_for_selection(thread_id) {
            match self
                .attach_live_thread_for_selection(app_server, thread_id)
                .await
            {
                Ok(true) => SelectionMode::Live,
                Ok(false) => SelectionMode::ReplayOnly,
                Err(_) if self.thread_event_channels.contains_key(&thread_id) => {
                    SelectionMode::ReplayOnly
                }
                Err(err) => {
                    self.add_agents_overview_error(format!(
                        "Failed to attach to agent thread {thread_id}: {err}"
                    ));
                    return Ok(());
                }
            }
        } else if self
            .agent_navigation
            .get(&thread_id)
            .is_some_and(|entry| entry.is_closed)
        {
            SelectionMode::Closed
        } else {
            SelectionMode::Live
        };
        self.commit_thread_selection(
            tui,
            app_server,
            thread_id,
            Selection {
                mode,
                cached_session,
                pending_profile_on_unavailable_thread,
            },
        )
        .await
    }

    pub(super) fn can_select_thread(&mut self) -> bool {
        if self.reject_pending_permission_root_switch() {
            return false;
        }
        if self.windows_sandbox_blocks_thread_switch() {
            self.chat_widget.add_info_message(
                "Finish Windows sandbox setup before switching threads.".to_string(),
                /*hint*/ None,
            );
            return false;
        }
        true
    }

    pub(super) async fn commit_thread_selection(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        selection: Selection,
    ) -> Result<()> {
        let Selection {
            mode,
            cached_session,
            pending_profile_on_unavailable_thread,
        } = selection;
        let mut is_replay_only = mode != SelectionMode::Live;
        let mut attached_replay_only = mode == SelectionMode::ReplayOnly;
        let previous_thread_id = self.active_thread_id;
        // Notifications already routed to the active channel must reach the old
        // widget before its transcript state is transferred. Events routed after
        // deactivation are retained separately for that thread.
        if let Some(mut receiver) = self.active_thread_rx.take() {
            while let Ok(event) = receiver.try_recv() {
                self.handle_thread_event_now_recovering_file_changes(event)
                    .await;
            }
            self.active_thread_rx = Some(receiver);
        }
        self.store_active_thread_receiver().await;
        self.active_thread_id = None;
        let Some((receiver, mut snapshot)) = self.activate_thread_for_replay(thread_id).await
        else {
            self.add_agents_overview_error(format!("Agent thread {thread_id} is already active."));
            if let Some(previous_thread_id) = previous_thread_id {
                self.activate_thread_channel(previous_thread_id).await;
            }
            return Ok(());
        };

        let session_verified = self
            .refresh_snapshot_session_if_needed(
                app_server,
                thread_id,
                is_replay_only,
                &mut snapshot,
            )
            .await;
        if !is_replay_only
            && !self.side_threads.contains_key(&thread_id)
            && snapshot.session.as_ref().is_some_and(|session| {
                session
                    .active_permission_profile
                    .as_ref()
                    .is_some_and(|profile| !profile.id.starts_with(':'))
            })
            && !session_verified
        {
            is_replay_only = true;
            attached_replay_only = true;
        }
        if pending_profile_on_unavailable_thread && !is_replay_only && session_verified {
            self.pending_server_profiles.remove(&thread_id);
        }
        // Refreshing can merge restored turns into the store, so recap progress must be read only
        // after the refresh while the activated thread channel is still retained.
        let Some(channel) = self.thread_event_channels.get(&thread_id) else {
            self.add_agents_overview_error(format!(
                "Agent thread {thread_id} is no longer available."
            ));
            return Ok(());
        };
        let recap_progress = {
            let mut store = channel.store.lock().await;
            if let (Some(cached), Some(session)) =
                (cached_session.as_ref(), snapshot.session.as_mut())
            {
                if !pending_profile_on_unavailable_thread {
                    self.restore_runtime_permissions(session, cached);
                }
                store.session = Some(session.clone());
                if !is_replay_only && self.primary_thread_id == Some(thread_id) {
                    self.primary_session_configured = Some(session.clone());
                }
            }
            store.recap_progress()
        };
        snapshot.input_state = snapshot
            .input_state
            .or(self.agents_overview.input_states.remove(&thread_id));

        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = Some(receiver);

        self.recap.note_focus_gained();
        self.recap = recap::RecapState::default();

        if !tui.is_terminal_focused() {
            self.recap.note_focus_lost(Instant::now());
        }
        let now = Instant::now();
        self.recap.seed_from_progress(recap_progress, now);
        self.schedule_recap_check(thread_id, now);

        // Draining the old receiver can queue history insertions and stream consolidation.
        // Park the transcript after those events and before the destination's reset and replay.
        if self.side_retention_enabled(tui)
            && self.pending_thread_switch_resets == 0
            && let Some(old) = previous_thread_id
            && old != thread_id
            && let Some(pair) = self.side_pair_key(old)
        {
            let conversation_width = self
                .conversation_area(tui.terminal.last_known_screen_size.into())
                .width;
            self.sync_owned_transcript(conversation_width);
            let width = self.transcript_view.remaining_area().width.max(1);
            let (stream, display) = self.chat_widget.take_side_display(width);
            self.app_event_tx.send(AppEvent::ParkSideCompanion {
                pair,
                thread_id: old,
                stream,
                display,
            });
        }
        if is_replay_only && let Some(session) = snapshot.session.as_mut() {
            session.read_only_history = true;
        }
        self.render_thread_snapshot(tui, app_server, thread_id, snapshot, !is_replay_only)?;
        if !is_replay_only
            && let Some(message) = self
                .side_threads
                .get_mut(&thread_id)
                .and_then(|side| side.opening_message.take())
        {
            self.chat_widget
                .submit_user_message_as_plain_user_turn(message);
        }
        if self.side_retention_enabled(tui) {
            if self.side_threads.contains_key(&thread_id) {
                self.sidebar_mut().show(
                    sidebar::SidebarContent::Chat(thread_id),
                    sidebar::PaneFocus::Sidebar,
                );
            } else if let Some(sidebar::SidebarContent::Chat(child)) = self
                .sidebar()
                .and_then(sidebar::SidebarPane::visible_content)
            {
                self.sidebar_mut().show(
                    sidebar::SidebarContent::Chat(child),
                    sidebar::PaneFocus::Main,
                );
            }
        }
        if is_replay_only
            && self
                .thread_event_channels
                .get(&thread_id)
                .is_none_or(|channel| channel.attachment() != ThreadEventAttachment::ExternalWriter)
        {
            self.chat_widget.pause_unavailable_thread();
            let message = if attached_replay_only {
                format!(
                    "Agent thread {thread_id} could not be resumed live. Replaying saved transcript."
                )
            } else {
                format!("Agent thread {thread_id} is closed. Replaying saved transcript.")
            };
            self.chat_widget.add_info_message(message, /*hint*/ None);
        }
        self.refresh_pending_thread_approvals().await;

        Ok(())
    }

    pub(super) fn render_thread_snapshot(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        snapshot: ThreadEventSnapshot,
        resume_restored_queue: bool,
    ) -> Result<()> {
        let retain_side_view = self.side_retention_enabled(tui);
        let side_parent = |id| {
            self.side_threads
                .get(&self.side_pair_key(id)?)
                .map(|side| side.parent_thread_id)
        };
        let preserve_screen = retain_side_view
            && side_parent(thread_id).is_some_and(|parent| {
                self.chat_widget.thread_id().and_then(side_parent) == Some(parent)
            });
        let init = self.chatwidget_init_for_forked_or_resumed_thread(
            tui,
            self.config.clone(),
            /*initial_user_message*/ None,
        );
        self.replace_chat_widget(ChatWidget::new_with_app_event(init));
        self.chat_widget
            .set_task_mentions_enabled(app_server.task_tools_available(thread_id));
        self.chat_widget
            .note_rendered_width(tui.terminal.last_known_screen_size.width);
        let external_writer = self
            .thread_event_channels
            .get(&thread_id)
            .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::ExternalWriter);
        if external_writer {
            self.chat_widget.show_external_writer_thread();
        }
        if self.agent_navigation.is_parent_owned(thread_id) {
            self.chat_widget.set_parent_owned_thread();
        }
        if retain_side_view {
            // Even a superseding selection must leave queued history its original cells and view.
            tui.clear_pending_history_lines();
            if !preserve_screen {
                tui.clear_for_thread_switch()?;
            }
        } else {
            self.reset_for_thread_switch(tui)?;
        }
        self.pending_thread_switch_resets += 1;
        self.app_event_tx.send(if preserve_screen {
            AppEvent::ResetTranscriptForThreadSwitchPreservingScreen
        } else {
            AppEvent::ResetTranscriptForThreadSwitch
        });
        self.replay_thread_snapshot(snapshot, resume_restored_queue);
        self.app_event_tx
            .send(AppEvent::RestoreBackgroundTerminals(thread_id));
        if let Some(thread_id) = self.chat_widget.thread_id()
            && let Some(active) = self
                .chat_widget
                .config_ref()
                .permissions
                .active_permission_profile()
            && self
                .agents_overview
                .selected_permission_profiles
                .get(&thread_id)
                == Some(&active.id)
        {
            self.adopt_server_permissions();
        }
        if external_writer {
            self.chat_widget.show_external_writer_thread();
        }
        if retain_side_view {
            // Hold drawing and input through the history events emitted by replay.
            self.chat_widget
                .run_commit_tick_with_scope(CommitTickScope::Replay);
            self.pending_thread_switch_resets += 1;
            self.app_event_tx
                .send(AppEvent::SideReplayFinished(thread_id));
        }
        Ok(())
    }

    pub(super) fn should_attach_live_thread_for_selection(&self, thread_id: ThreadId) -> bool {
        self.agent_navigation
            .get(&thread_id)
            .is_some_and(|entry| entry.is_closed)
            || self
                .thread_event_channels
                .get(&thread_id)
                .is_none_or(|channel| channel.attachment() != ThreadEventAttachment::Live)
    }

    pub(super) fn reset_for_thread_switch(&mut self, tui: &mut tui::Tui) -> Result<()> {
        self.reset_transcript_state_after_clear();
        tui.clear_pending_history_lines();
        Ok(tui.clear_for_thread_switch()?)
    }

    pub(super) async fn reset_thread_event_state(&mut self) {
        let voice_owner = self.voice_owner_thread_id();
        let mut previous_pending = std::mem::take(&mut self.pending_app_server_requests);
        // Move retained tasks' approvals to background routing before clearing request bookkeeping.
        for (thread_id, requests) in &mut self.agents_overview.dispatched_requests {
            if let Some(channel) = self.thread_event_channels.get(thread_id) {
                for request in channel.store.lock().await.pending_replay_requests() {
                    if !requests
                        .iter()
                        .any(|pending| pending.request.id() == request.id())
                        && let Some(request) = previous_pending.buffered_request(request)
                    {
                        requests.push(request);
                    }
                }
            }
        }
        if voice_owner.is_some() {
            for (thread_id, channel) in &self.thread_event_channels {
                if Some(*thread_id) != voice_owner && !self.side_threads.contains_key(thread_id) {
                    for request in channel.store.lock().await.pending_replay_requests() {
                        previous_pending.resolve_notification(&thread_id.to_string(), request.id());
                    }
                }
            }
        }
        self.thread_event_listener_tasks.retain(|id, task| {
            if Some(*id) == voice_owner
                || self.side_threads.contains_key(id)
                || self.agents_overview.dispatched_requests.contains_key(id)
            {
                true
            } else {
                task.abort();
                false
            }
        });
        self.thread_event_channels
            .retain(|id, _| Some(*id) == voice_owner || self.side_threads.contains_key(id));
        // A retained side can still display its parent even after the parent channel is detached.
        self.side_companions.retain(|id, _| {
            self.side_threads.contains_key(id)
                || self
                    .side_threads
                    .values()
                    .any(|side| side.parent_thread_id == *id)
        });
        self.sidebars.remove(&None);
        self.pending_realtime_speech_replay.clear();
        self.pending_realtime_transcript_replay.clear();
        self.realtime_replay_order.clear();
        self.pending_server_profiles.clear();
        self.agents_overview.activity.clear();
        self.agent_navigation.clear();
        self.active_thread_id = None;
        self.active_thread_rx = None;
        self.primary_thread_id = None;
        self.last_subagent_backfill_attempt = None;
        self.primary_session_configured = None;
        self.pending_primary_events.clear();
        if voice_owner.is_none() {
            let side_thread_ids: Vec<_> = self.side_threads.keys().copied().collect();
            for thread_id in side_thread_ids {
                if let Some(channel) = self.thread_event_channels.get(&thread_id) {
                    for request in channel.store.lock().await.pending_replay_requests() {
                        if let Some(request) = previous_pending.buffered_request(request) {
                            let _ = self
                                .pending_app_server_requests
                                .note_buffered_request(&request);
                        }
                    }
                }
            }
        } else {
            self.pending_app_server_requests = previous_pending;
        }
        self.pending_startup_thread_start = false;
        self.pending_server_version_notice = None;
        self.chat_widget.set_pending_thread_approvals(Vec::new());
        self.sync_active_agent_label();
    }

    pub(super) async fn handle_startup_thread_started(
        &mut self,
        app_server: &mut AppServerSession,
        result: Result<AppServerStartedThread>,
    ) -> Result<()> {
        if !self.pending_startup_thread_start {
            if let Ok(started) = result {
                let thread_id = started.session.thread_id;
                if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
                    tracing::warn!(
                        thread_id = %thread_id,
                        "failed to unsubscribe stale startup thread: {err}"
                    );
                }
                self.discard_thread_local_state(thread_id).await;
            }
            return Ok(());
        }

        // A refusal proves no thread was created.
        if let Err(error) = &result
            && matches!(
                error.downcast_ref::<TypedRequestError>(),
                Some(TypedRequestError::Server { method, source })
                    if method == "thread/start" && is_server_shutting_down_error(source)
            )
            && self.begin_reconnect_for_rejected_startup()
        {
            return Ok(());
        }

        self.pending_startup_thread_start = false;
        self.chat_widget
            .set_queue_submissions_until_session_configured(/*queue*/ false);
        match result {
            Ok(started) => {
                self.chat_widget.mark_fresh_task_for_sparkle(&started);
                let thread_id = started.session.thread_id;
                if started.task_tools_available {
                    app_server.remember_task_tool_thread(thread_id);
                    self.chat_widget.set_task_mentions_enabled(/*enabled*/ true);
                }
                self.pending_primary_events.retain(|event| match event {
                    ThreadBufferedEvent::Notification(notification) => matches!(
                        server_notification_thread_target(notification),
                        ServerNotificationThreadTarget::Thread(event_thread_id)
                            if event_thread_id == thread_id
                    ),
                    ThreadBufferedEvent::Request(request) => {
                        server_request_thread_id(request) == Some(thread_id)
                    }
                    ThreadBufferedEvent::HistoryEntryResponse(_)
                    | ThreadBufferedEvent::FeedbackSubmission(_) => true,
                });
                let previous_pending = std::mem::take(&mut self.pending_app_server_requests);
                for event in &self.pending_primary_events {
                    if let ThreadBufferedEvent::Request(request) = event {
                        let retained = previous_pending.buffered_request((**request).clone());
                        #[cfg(test)]
                        let retained = retained.or_else(|| {
                            Some(super::app_server_requests::BufferedServerRequest::new(
                                (**request).clone(),
                                &app_server.request_handle(),
                            ))
                        });
                        if let Some(request) = retained {
                            let _ = self
                                .pending_app_server_requests
                                .note_buffered_request(&request);
                        }
                    }
                }
                if started.blocks_direct_input {
                    self.mark_primary_thread_parent_owned(thread_id);
                }
                // Lifecycle notifications may arrive before the thread/start response.
                if !self.config.ephemeral
                    && !self.pending_primary_events.iter().any(|event| {
                        matches!(event, ThreadBufferedEvent::Notification(notification)
                            if matches!(notification.as_ref(),
                                ServerNotification::TurnStarted(_)
                                    | ServerNotification::ThreadClosed(_)
                                    | ServerNotification::ThreadArchived(_)
                                    | ServerNotification::ThreadDeleted(_)))
                    })
                {
                    self.agents_overview
                        .blank_sessions
                        .insert(thread_id, started.clone());
                }
                // A full usage read can finish before thread/start. Apply its cached fallback
                // after attachment but before the initial prompt or queued draft is submitted.
                let recovery_was_pending = self.chat_widget.hold_rate_limit_recovery();
                if self.loader_overrides.user_config_profile.is_none() {
                    self.chat_widget.accept_startup_approval_policy(
                        &mut self.config.permissions.approval_policy,
                        started.session.approval_policy,
                    )?;
                }
                self.enqueue_primary_thread_session(started.session, started.turns)
                    .await?;
                self.apply_backend_banner_fallback(app_server).await;
                if let Some(model) = self.harness_overrides.model.as_deref()
                    && model == self.chat_widget.current_model()
                    && self.chat_widget.has_chatgpt_account()
                    && self.chat_widget.config_ref().model_provider_id == OPENAI_PROVIDER_ID
                    && !self
                        .model_catalog
                        .models
                        .iter()
                        .any(|preset| preset.model == model)
                {
                    self.chat_widget.add_to_history(history_cell::new_visible_warning_event(format!(
                        "Your ChatGPT model list doesn't include `{model}`. Check the spelling or use /model to choose another model."
                    )));
                }
                if let Some(notice) = self.pending_server_version_notice.take() {
                    self.chat_widget.add_server_version_warning(notice);
                }
                if !recovery_was_pending {
                    self.chat_widget.finish_rate_limit_recovery();
                }
                self.chat_widget.maybe_send_next_queued_input();
            }
            Err(err) if self.recover_transport_error(&err) => {}
            Err(err) => {
                let warnings = self
                    .transcript_cells
                    .iter()
                    .find_map(|cell| {
                        cell.as_any()
                            .downcast_ref::<history_cell::StartupWarningsCell>()
                    })
                    .filter(|cell| cell.pending_header)
                    .map(|cell| format!("\n\nStartup warnings:\n{}", cell.messages.join("\n")))
                    .unwrap_or_default();
                return Err(color_eyre::eyre::eyre!(
                    "Failed to start a fresh session through the app server: {err}{warnings}"
                ));
            }
        }
        Ok(())
    }

    pub(super) async fn start_fresh_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        session_start_source: Option<ThreadStartSource>,
        initial_user_message: Option<crate::chatwidget::UserMessage>,
        new_thread_name: Option<String>,
    ) {
        if self.reject_pending_permission_root_switch() {
            if let Some(message) = initial_user_message {
                self.chat_widget.restore_user_message_to_composer(message);
            }
            return;
        }
        // Resolve trust before loading defaults and applying explicit permission choices.
        if self.current_displayed_thread_id().is_none() {
            let mut config = self.chat_widget.config_ref().clone();
            let cwd = config.cwd.to_path_buf();
            if let Err(control) = self
                .confirm_directory_trust(
                    tui,
                    app_server,
                    &mut config,
                    &cwd,
                    crate::onboarding::DirectoryTrustOptions::default(),
                    /*startup_draft*/ None,
                )
                .await
            {
                if matches!(control, AppRunControl::Exit(_)) {
                    self.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
                }
                return;
            }
        }
        // Start a fresh in-memory session while preserving resumability via persisted rollout
        // history. If an initial message is provided, `enqueue_primary_thread_session` suppresses it
        // until the new session is configured and any replayed turns have been rendered.
        match self
            .prepare_fresh_session(app_server, session_start_source)
            .await
        {
            Ok(prepared) => {
                self.adopt_fresh_session(
                    tui,
                    app_server,
                    prepared,
                    ThreadAttachPresentation::Fresh,
                    initial_user_message,
                    new_thread_name,
                )
                .await;
            }
            Err(err) => {
                self.chat_widget.add_error_message(err.to_string());
                if let Some(message) = initial_user_message {
                    self.chat_widget.restore_user_message_to_composer(message);
                }
            }
        }
        tui.frame_requester().schedule_frame();
    }

    async fn prepare_fresh_session(
        &mut self,
        app_server: &mut AppServerSession,
        session_start_source: Option<ThreadStartSource>,
    ) -> Result<PreparedFreshSession> {
        let (config, local_settings) =
            crate::boxed_future!(self.load_new_session_config(app_server))
                .await
                .map_err(|err| {
                    color_eyre::eyre::eyre!("Failed to read new session defaults: {err}")
                })?;
        let started = crate::boxed_future!(app_server.start_thread_with_session_start_source(
            &self.local_settings,
            &config,
            session_start_source,
            /*remote_cwd_override*/ None,
            /*selected_profile*/ None,
            self.backend_preference_context.local_cwd.as_deref(),
        ))
        .await
        .map_err(|err| {
            color_eyre::eyre::eyre!("Failed to start a fresh session through the app server: {err}")
        })?;
        Ok(PreparedFreshSession {
            started,
            config,
            local_settings,
        })
    }

    pub(super) async fn implement_plan_in_fresh_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        plan: String,
    ) -> Result<()> {
        if self.reject_pending_permission_root_switch() {
            return Ok(());
        }
        let prepared = match self
            .prepare_fresh_session(app_server, Some(ThreadStartSource::Clear))
            .await
        {
            Ok(prepared) => prepared,
            Err(err) => {
                self.chat_widget.add_error_message(err.to_string());
                tui.frame_requester().schedule_frame();
                return Ok(());
            }
        };
        let thread_id = prepared.started.session.thread_id;
        let codex_home = app_server.codex_home_path(&prepared.config.codex_home);
        let attachment_path = crate::attachment_files::write_approved_plan(
            app_server,
            codex_home.as_ref(),
            plan.clone(),
        )
        .await
        .map(|path| path.to_string())
        .inspect_err(|err| tracing::debug!(%err, "approved plan attachment unavailable"))
        .ok();
        match app_server
            .thread_prepare_approved_plan(ThreadPrepareApprovedPlanParams {
                thread_id: thread_id.to_string(),
                markdown: plan,
                attachment_path,
            })
            .await
        {
            Ok(prompt) => {
                self.clear_terminal_ui(tui, /*redraw_header*/ false)?;
                self.reset_app_ui_state_after_clear();
                self.adopt_fresh_session(
                    tui,
                    app_server,
                    prepared,
                    ThreadAttachPresentation::Fresh,
                    Some(prompt.into()),
                    /*new_thread_name*/ None,
                )
                .await;
            }
            Err(err) => {
                let _ = app_server.thread_unsubscribe(thread_id).await;
                self.chat_widget
                    .add_error_message(format!("Failed to hand off the approved plan: {err:#}"));
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) async fn adopt_fresh_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        prepared: PreparedFreshSession,
        presentation: ThreadAttachPresentation,
        initial_user_message: Option<crate::chatwidget::UserMessage>,
        new_thread_name: Option<String>,
    ) {
        let PreparedFreshSession {
            mut started,
            config,
            local_settings,
        } = prepared;
        if let Err(error) = self
            .backend_registry
            .register_started(app_server.backend_id(), &started)
        {
            self.chat_widget.add_error_message(error.to_string());
            return;
        }
        if let Some(thread_id) = self.current_displayed_thread_id()
            && let Some(blank) = self.agents_overview.blank_sessions.get_mut(&thread_id)
        {
            if let Some(channel) = self.thread_event_channels.get(&thread_id)
                && let Some(session) = channel.store.lock().await.session.as_ref()
            {
                blank.session = session.clone();
            }
            if let Some(input) = self.chat_widget.capture_thread_input_state() {
                self.agents_overview.input_states.insert(thread_id, input);
            }
        }
        self.detach_current_thread_for_navigation(app_server, Some(started.session.thread_id))
            .await;
        self.local_settings = local_settings;
        self.refresh_server_version_overview_notice(CODEX_CLI_VERSION);
        self.config = config;
        self.remember_launch_permissions();

        let name_error = if let Some(name) = new_thread_name {
            match app_server
                .thread_set_name(started.session.thread_id, name.clone())
                .await
            {
                Ok(()) => {
                    started.session.thread_name = Some(name);
                    None
                }
                Err(err) => Some(format!("Failed to name the new session: {err}")),
            }
        } else {
            None
        };
        let thread_id = started.session.thread_id;
        let (account, account_error) = if self.model_catalog.provider_may_have_changed() {
            match app_server.read_thread_account(&started.session).await {
                Ok(account) => (Some(account), None),
                Err(err) => (Some(SessionAccount::default()), Some(err)),
            }
        } else {
            (None, None)
        };
        if !self.config.ephemeral {
            self.agents_overview
                .blank_sessions
                .insert(thread_id, started.clone());
        }
        if let Err(err) = self
            .replace_chat_widget_with_account(
                tui,
                started,
                presentation,
                initial_user_message,
                account,
            )
            .await
        {
            self.agents_overview.blank_sessions.remove(&thread_id);
            self.chat_widget.add_error_message(format!(
                "Failed to attach to fresh app-server thread: {err}"
            ));
        } else {
            if let Some(err) = name_error {
                self.chat_widget.add_error_message(err);
            }
            if let Some(err) = account_error {
                self.chat_widget.add_error_message(format!(
                    "Could not load account details for this chat: {err}"
                ));
            }
        }
    }

    pub(super) async fn replace_chat_widget_with_app_server_thread(
        &mut self,
        tui: &mut tui::Tui,
        started: AppServerStartedThread,
        presentation: ThreadAttachPresentation,
        initial_user_message: Option<crate::chatwidget::UserMessage>,
    ) -> Result<()> {
        self.replace_chat_widget_with_account(
            tui,
            started,
            presentation,
            initial_user_message,
            /*account*/ None,
        )
        .await
    }

    async fn drain_outgoing_thread_events(&mut self) {
        if let Some(mut receiver) = self.active_thread_rx.take() {
            while let Ok(event) = receiver.try_recv() {
                self.handle_thread_event_now_recovering_file_changes(event)
                    .await;
            }
            self.active_thread_rx = Some(receiver);
        }
    }

    async fn replace_chat_widget_with_account(
        &mut self,
        tui: &mut tui::Tui,
        started: AppServerStartedThread,
        presentation: ThreadAttachPresentation,
        initial_user_message: Option<crate::chatwidget::UserMessage>,
        account: Option<SessionAccount>,
    ) -> Result<()> {
        // Initial messages are for freshly attached primary threads only. Thread switches and
        // resume/fork flows pass `None` so they cannot replay old history and then auto-submit a new
        // user turn by accident.
        if !matches!(
            presentation,
            ThreadAttachPresentation::SessionLineageAfterClear
        ) {
            self.drain_outgoing_thread_events().await;
        }
        self.store_active_thread_receiver().await;
        self.reset_transcript_state_after_clear();
        tui.clear_pending_history_lines();
        let presentation = match presentation {
            ThreadAttachPresentation::FreshAfterClear => ThreadAttachPresentation::Fresh,
            ThreadAttachPresentation::SessionLineageAfterClear => {
                ThreadAttachPresentation::SessionLineage
            }
            ThreadAttachPresentation::FreshWithDraft => {
                tui.defer_thread_switch_clear();
                presentation
            }
            ThreadAttachPresentation::Fresh | ThreadAttachPresentation::SessionLineage => {
                tui.clear_for_thread_switch()?;
                presentation
            }
        };
        self.pending_thread_switch_resets += 1;
        self.app_event_tx.send(
            if matches!(presentation, ThreadAttachPresentation::FreshWithDraft) {
                AppEvent::ResetTranscriptForThreadSwitchPreservingScreen
            } else {
                AppEvent::ResetTranscriptForThreadSwitch
            },
        );
        self.reset_thread_event_state().await;
        let mut init = self.chatwidget_init_for_forked_or_resumed_thread(
            tui,
            self.config.clone(),
            initial_user_message,
        );
        if let Some(account) = account {
            self.account_email_request_id = None;
            self.feedback_audience = account.feedback_audience();
            init.has_chatgpt_account = account.has_chatgpt_account();
            init.has_codex_backend_auth = init.has_chatgpt_account;
            init.requires_openai_auth = account.requires_openai_auth;
            init.status_account_display = account.display;
            init.initial_plan_type = account.plan;
        }
        self.replace_chat_widget(ChatWidget::new_with_app_event(init));
        if matches!(
            presentation,
            ThreadAttachPresentation::Fresh | ThreadAttachPresentation::FreshWithDraft
        ) {
            self.chat_widget.mark_fresh_task_for_sparkle(&started);
            // FreshWithDraft inherits its provisional replay at the handoff.
            if matches!(presentation, ThreadAttachPresentation::Fresh) {
                self.chat_widget
                    .empty_state_animation
                    .borrow_mut()
                    .start_fresh();
            }
        }
        self.chat_widget
            .set_task_mentions_enabled(started.task_tools_available);
        self.chat_widget
            .note_rendered_width(tui.terminal.last_known_screen_size.width);
        if started.blocks_direct_input {
            self.mark_primary_thread_parent_owned(started.session.thread_id);
        }
        self.enqueue_primary_thread_session_with_presentation(
            started.session,
            started.turns,
            presentation,
        )
        .await?;
        Ok(())
    }

    /// Fetches all loaded threads from the app server and registers descendants of the primary
    /// thread in the navigation cache and chat widget metadata.
    ///
    /// Called when opening the `/subagents` picker and after resuming a thread so that the picker and
    /// keyboard navigation are pre-populated even if the TUI did not witness the original spawn
    /// events. Fresh and forked threads cannot have pre-existing descendants.
    ///
    /// The loaded-thread list is fetched in full (no pagination) and the spawn tree is walked
    /// by `find_loaded_subagent_threads_for_primary`. Each discovered subagent is registered via
    /// `upsert_agent_picker_thread`, which writes to both `AgentNavigationState` and the
    /// `ChatWidget` metadata map.
    pub(super) async fn backfill_loaded_subagent_threads(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> LoadedSubagentBackfill {
        let Some(primary_thread_id) = self.primary_thread_id else {
            return LoadedSubagentBackfill::default();
        };

        let loaded_thread_ids = match app_server
            .thread_loaded_list(ThreadLoadedListParams {
                cursor: None,
                limit: None,
            })
            .await
        {
            Ok(response) => response.data,
            Err(err) => {
                tracing::warn!(%err, "failed to list loaded threads for subagent backfill");
                return LoadedSubagentBackfill::default();
            }
        };

        let mut threads = Vec::new();
        let mut had_read_error = false;
        for thread_id in loaded_thread_ids {
            let Ok(thread_id) = ThreadId::from_string(&thread_id) else {
                tracing::warn!("ignoring loaded thread with invalid id during subagent backfill");
                continue;
            };

            if thread_id == primary_thread_id {
                continue;
            }

            match app_server
                .thread_read(thread_id, /*include_turns*/ false)
                .await
            {
                Ok(thread) => threads.push(thread),
                Err(err) => {
                    had_read_error = true;
                    tracing::warn!(thread_id = %thread_id, %err, "failed to read loaded thread");
                }
            }
        }

        let mut refreshed_thread_ids = HashSet::new();
        for thread in find_loaded_subagent_threads_for_primary(threads, primary_thread_id) {
            let agent_path = thread.agent_path;
            let has_live_channel = self
                .thread_event_channels
                .get(&thread.thread_id)
                .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::Live);
            let is_closed = !has_live_channel && thread.is_closed;
            if thread.blocks_direct_input {
                self.agent_navigation.mark_parent_owned(thread.thread_id);
            }
            self.upsert_agent_picker_thread(
                thread.thread_id,
                thread.agent_nickname,
                thread.agent_role,
                is_closed,
            );
            self.agent_navigation
                .set_agent_path(thread.thread_id, agent_path);
            // A live channel can have an empty store after a successful spawn. Only apply server
            // status for channels that would otherwise need another liveness read.
            if !has_live_channel {
                if thread.is_running {
                    self.agent_navigation.mark_running(thread.thread_id);
                } else {
                    self.agent_navigation
                        .set_running(thread.thread_id, /*is_running*/ false);
                }
                refreshed_thread_ids.insert(thread.thread_id);
            }
        }
        self.refresh_agent_picker();

        LoadedSubagentBackfill {
            completed: !had_read_error,
            refreshed_thread_ids,
        }
    }

    /// Returns the adjacent thread id for keyboard navigation, backfilling from the server if the
    /// local cache has no neighbor.
    ///
    /// Tries the fast path first: ask `AgentNavigationState` directly. If it returns `None` (no
    /// adjacent entry exists, typically because the cache was never populated with remote
    /// subagents), performs a full `backfill_loaded_subagent_threads` and retries. This ensures the
    /// first next/previous keypress in a resumed remote session discovers subagents on demand
    /// without requiring the user to wait for a proactive fetch.
    pub(super) async fn adjacent_thread_id_with_backfill(
        &mut self,
        app_server: &mut AppServerSession,
        direction: AgentNavigationDirection,
    ) -> Option<ThreadId> {
        let current_thread = self.current_displayed_thread_id();
        if let Some(thread_id) = self
            .agent_navigation
            .adjacent_thread_id(current_thread, direction)
        {
            return Some(thread_id);
        }

        let primary_thread_id = self.primary_thread_id?;
        if self.last_subagent_backfill_attempt == Some(primary_thread_id) {
            return None;
        }

        if self
            .backfill_loaded_subagent_threads(app_server)
            .await
            .completed
        {
            self.last_subagent_backfill_attempt = Some(primary_thread_id);
        }
        self.agent_navigation
            .adjacent_thread_id(self.current_displayed_thread_id(), direction)
    }

    pub(super) async fn resume_target_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target_session: SessionTarget,
    ) -> Result<AppRunControl> {
        if let Err(error) = target_session.ensure_backend(app_server.backend_id()) {
            self.add_session_picker_error(error.to_string());
            return Ok(AppRunControl::Continue);
        }
        if self.ignore_same_thread_resume(&target_session) {
            self.agents_overview
                .hidden_threads
                .remove(&target_session.thread_id);
            self.repaint_agents_overview();
            tui.frame_requester().schedule_frame();
            return Ok(AppRunControl::Continue);
        }
        if self.windows_sandbox_blocks_thread_switch() {
            self.chat_widget.add_info_message(
                "Finish Windows sandbox setup before switching threads.".to_string(),
                /*hint*/ None,
            );
            return Ok(AppRunControl::Continue);
        }

        if self.reject_pending_permission_root_switch() {
            return Ok(AppRunControl::Continue);
        }
        let prepared = match self
            .prepare_target_session(
                tui,
                app_server,
                &target_session,
                super::resume_preparation::ResumeSource::Active,
            )
            .await
        {
            Ok(prepared) => prepared,
            Err(control) => return Ok(control),
        };
        self.activate_prepared_session(tui, app_server, prepared)
            .await
    }

    pub(super) async fn activate_prepared_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        prepared: super::resume_preparation::PreparedResume,
    ) -> Result<AppRunControl> {
        if prepared.backend_id != app_server.backend_id() {
            self.add_session_picker_error(
                "This session belongs to a different backend. Open the session picker again."
                    .to_string(),
            );
            return Ok(AppRunControl::Continue);
        }
        let handoff = self.chat_widget.pause_input_for_resume_handoff();
        let _handoff = self.prepare_resume_handoff(tui, handoff).await?;
        self.activate_prepared_session_after_clear(
            tui, app_server, prepared, /*switched_backend*/ false,
        )
        .await
    }

    pub(super) async fn prepare_resume_handoff(
        &mut self,
        tui: &mut tui::Tui,
        handoff: std::sync::Arc<()>,
    ) -> Result<std::sync::Arc<()>> {
        self.drain_outgoing_thread_events().await;
        // Keep the outgoing receiver and session in place if the terminal clear fails.
        if let Err(error) = tui.clear_for_thread_switch() {
            drop(handoff);
            self.chat_widget.maybe_send_next_queued_input();
            return Err(error.into());
        }
        Ok(handoff)
    }

    pub(super) async fn activate_prepared_session_after_clear(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        prepared: super::resume_preparation::PreparedResume,
        switched_backend: bool,
    ) -> Result<AppRunControl> {
        let super::resume_preparation::PreparedResume {
            backend_id,
            config: resume_config,
            local_settings,
            thread: resumed,
            read_only,
            history_notice,
            provider_error,
            restored_blank_session,
        } = prepared;
        debug_assert_eq!(backend_id, app_server.backend_id());
        let resumed_thread_id = resumed.session.thread_id;
        self.backend_registry.observe(backend_id, resumed_thread_id);
        let has_resumed_mode = resumed.session.collaboration_mode.is_some();
        let was_reconnecting = !switched_backend && self.chat_widget.thread_stream_disconnected;
        let owned_channel = self
            .thread_event_channels
            .get(&resumed_thread_id)
            .filter(|channel| channel.backend_id.is_none_or(|owner| owner == backend_id));
        let same_thread =
            !switched_backend && self.chat_widget.thread_id() == Some(resumed_thread_id);
        let refreshing_history = same_thread
            && !was_reconnecting
            && owned_channel.is_some_and(|channel| channel.history_may_be_incomplete);
        let retained_input = (same_thread
            && owned_channel.is_some()
            && (refreshing_history
                || was_reconnecting
                || self.chat_widget.is_read_only_view()
                || self.chat_widget.has_provider_error()))
        .then(|| self.chat_widget.capture_thread_input_state())
        .flatten();
        let mut confirmed_message_ids =
            super::thread_events::confirmed_user_message_ids(&resumed.turns, std::iter::empty());
        if let Some(channel) = owned_channel {
            let store = channel.store.lock().await;
            confirmed_message_ids.extend(super::thread_events::confirmed_user_message_ids(
                &store.turns,
                store.buffer.iter(),
            ));
        }
        // A backend switch has already parked the outgoing inputs.
        if !switched_backend {
            let displayed_thread_id = self.current_displayed_thread_id();
            for (id, channel) in &self.thread_event_channels {
                if Some(*id) != displayed_thread_id
                    && channel.backend_id.is_none_or(|owner| owner == backend_id)
                    && let Some(input) = channel.store.lock().await.input_state.clone()
                {
                    self.agents_overview.input_states.insert(*id, input);
                }
            }
            if displayed_thread_id != Some(resumed_thread_id)
                && let Some(id) = displayed_thread_id
                && self
                    .backend_registry
                    .owner(id)
                    .is_none_or(|owner| owner == backend_id)
                && let Some(input) = self.chat_widget.capture_thread_input_state()
            {
                self.agents_overview.input_states.insert(id, input);
            }
        }
        self.detach_current_thread_for_navigation(app_server, Some(resumed_thread_id))
            .await;
        self.local_settings = local_settings;
        self.refresh_server_version_overview_notice(CODEX_CLI_VERSION);
        self.config = resume_config;
        tui.set_notification_settings(
            self.local_settings.tui.notification_settings.method,
            self.local_settings.tui.notification_settings.condition,
        );
        self.file_search
            .update_search_dir(self.config.cwd.to_path_buf());
        match self
            .replace_chat_widget_with_app_server_thread(
                tui,
                resumed,
                ThreadAttachPresentation::SessionLineageAfterClear,
                /*initial_user_message*/ None,
            )
            .await
        {
            Ok(()) => {
                self.ensure_thread_channel(resumed_thread_id)
                    .history_may_be_incomplete = false;
                self.remember_attached_host_project(app_server);
                let resumed_turn_running = !read_only
                    && self
                        .active_turn_id_for_thread(resumed_thread_id)
                        .await
                        .is_some();
                let preserve_in_flight_turn = resumed_turn_running;
                if switched_backend && !restored_blank_session {
                    self.agents_overview
                        .blank_sessions
                        .remove(&resumed_thread_id);
                }
                let keep_blank_settings = self
                    .agents_overview
                    .blank_sessions
                    .contains_key(&resumed_thread_id);
                if let Some(mut input) =
                    retained_input.or(self.agents_overview.input_states.remove(&resumed_thread_id))
                {
                    // A successful resume is the authority for clearing the old rejection.
                    input.provider_error = None;
                    if !restored_blank_session {
                        self.chat_widget
                            .update_input_collaboration_mode(&mut input, has_resumed_mode);
                    }
                    // A saved draft includes model settings, so apply newer server settings after
                    // restoring input for retained blank sessions.
                    let pending_settings = keep_blank_settings
                        .then(|| input.pending_thread_settings.take())
                        .flatten();
                    let reconcile_input = refreshing_history
                        || !input.pending_steers.is_empty()
                        || input.has_unconfirmed_messages();
                    if reconcile_input {
                        input.reconcile_confirmed_messages(&confirmed_message_ids);
                        input.reconnect_pending |= refreshing_history || !resumed_turn_running;
                    }
                    if refreshing_history
                        || (reconcile_input && !read_only && !resumed_turn_running)
                    {
                        self.chat_widget
                            .restore_reconnected_input(Some(input), &confirmed_message_ids);
                    } else {
                        self.chat_widget.restore_thread_input_state(
                            Some(input),
                            ThreadInputStateRestoreMode {
                                preserve_in_flight_turn,
                            },
                        );
                    }
                    if let Some(settings) = pending_settings {
                        self.chat_widget.on_thread_settings_updated(settings);
                    }
                    if was_reconnecting && !read_only && provider_error.is_none() {
                        self.chat_widget.maybe_send_next_queued_input();
                    }
                }
                if let Some(error) = provider_error {
                    self.chat_widget
                        .show_provider_error(error, /*rejected_message_id*/ None);
                } else if read_only {
                    self.ensure_thread_channel(resumed_thread_id)
                        .mark_external_writer();
                    self.chat_widget.show_external_writer_thread();
                }
                if let Some(notice) = history_notice {
                    self.chat_widget
                        .add_info_message(notice.to_string(), /*hint*/ None);
                }
                self.backfill_loaded_subagent_threads(app_server).await;
                if matches!(
                    self.runtime_approval_policy_override,
                    Some(RuntimeApprovalPolicyOverride::Restored(_))
                ) {
                    self.runtime_approval_policy_override = None;
                }
                if self
                    .runtime_permission_profile_override
                    .as_ref()
                    .is_some_and(|profile| {
                        profile.turn_override == RuntimePermissionProfileTurnOverride::Preserve
                    })
                {
                    self.runtime_permission_profile_override = None;
                }
                if !read_only {
                    self.replay_agents_overview_requests(app_server, resumed_thread_id)
                        .await;
                }
                if !read_only {
                    self.maybe_prompt_resume_paused_goal_after_resume(
                        app_server,
                        resumed_thread_id,
                    )
                    .await;
                    if self
                        .active_turn_id_for_thread(resumed_thread_id)
                        .await
                        .is_none()
                    {
                        self.chat_widget.maybe_send_next_queued_input();
                    }
                }
            }
            Err(err) => return Err(err),
        }

        Ok(AppRunControl::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_thread_read_error_detection_matches_not_loaded_errors() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread not loaded: thr_123"
        );

        assert!(App::is_terminal_thread_read_error(&err));
    }

    #[test]
    fn terminal_thread_read_error_detection_ignores_transient_failures() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read transport error: broken pipe"
        );

        assert!(!App::is_terminal_thread_read_error(&err));
    }

    #[test]
    fn closed_state_for_thread_read_error_preserves_live_state_without_cache_on_transient_error() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read transport error: broken pipe"
        );

        assert!(!App::closed_state_for_thread_read_error(
            &err, /*existing_is_closed*/ None
        ));
    }

    #[test]
    fn closed_state_for_thread_read_error_marks_terminal_uncached_threads_closed() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread not loaded: thr_123"
        );

        assert!(App::closed_state_for_thread_read_error(
            &err, /*existing_is_closed*/ None
        ));
    }

    #[test]
    fn include_turns_fallback_detection_handles_unmaterialized_and_ephemeral_threads() {
        let unmaterialized = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread thr_123 is not materialized yet; includeTurns is unavailable before first user message"
        );
        let ephemeral = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: ephemeral threads do not support includeTurns"
        );

        assert!(App::can_fallback_from_include_turns_error(&unmaterialized));
        assert!(App::can_fallback_from_include_turns_error(&ephemeral));
    }
}
