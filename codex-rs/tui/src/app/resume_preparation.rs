//! Prepare saved-thread attachment before replacing the active conversation UI.
//!
//! Stored-backend preparation leaves active settings unchanged; active-backend resume retains its
//! eager preference refresh. Neither detaches the active thread or replaces its composer. The
//! prepared result retains the connection identity and any provider error for read-only history.

use super::*;
use crate::app_server_session::HistoryHydrationScope;
use crate::app_server_session::thread_blocks_direct_input;
use crate::backend_id::BackendId;
use codex_app_server_client::CloudRecoveryRelease;
use codex_app_server_client::TypedRequestError;
use std::io;
use tokio_util::task::AbortOnDropHandle;

/// Settings retained for a backend that is not currently displayed.
#[derive(Clone)]
pub(super) struct BackendSettings {
    pub(super) config: Config,
    pub(super) local_settings: crate::local_settings::LocalSettings,
    pub(super) target: AppServerTarget,
}

/// The settings source for a resume attempt.
///
/// The active source may refresh the displayed conversation's settings eagerly. A stored backend
/// is refreshed without adopting its settings until attachment succeeds.
#[derive(Clone, Copy)]
pub(super) enum ResumeSource<'a> {
    Active,
    Backend(&'a BackendSettings),
}

pub(super) struct PreparedResume {
    pub(super) backend_id: crate::backend_id::BackendId,
    pub(super) config: Config,
    pub(super) local_settings: crate::local_settings::LocalSettings,
    pub(super) thread: AppServerStartedThread,
    pub(super) restored_blank_session: bool,
    pub(super) read_only: bool,
    pub(super) history_notice: Option<&'static str>,
    pub(super) provider_error: Option<codex_app_server_protocol::ModelProviderError>,
}

const CLOUD_REQUEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 15);
const REOPEN_CLOUD_THREAD: &str = "Cloud stream ended. Reopen the conversation to reconnect.";

#[derive(Debug, thiserror::Error)]
enum CloudHistoryCheck {
    #[error("Cloud could not restore the reply stream in time. Your draft is still here.")]
    LockTimedOut,
    #[error(
        "Cloud conversation history is unavailable. Sending is paused and your draft is saved here."
    )]
    Unavailable,
}

#[derive(PartialEq)]
enum ResumePhase {
    Preparing,
    Installing,
    InstallInterruptedByNavigation,
}

pub(super) struct CloudResume {
    pub(super) backend_id: BackendId,
    pub(super) thread_id: ThreadId,
    store: Arc<Mutex<super::thread_events::ThreadEventStore>>,
    _input_hold: Arc<()>,
    phase: ResumePhase,
    pub(super) task: AbortOnDropHandle<CloudResumeOutcome>,
}

impl CloudResume {
    pub(super) fn installing(&self) -> bool {
        self.phase != ResumePhase::Preparing
    }
}

pub(super) type CloudResumeOutcome = Result<(
    Box<(
        AppServerStartedThread,
        crate::app_server_session::AttachmentMetadata,
    )>,
    Option<CloudRecoveryRelease>,
)>;

impl App {
    pub(super) fn start_cloud_resume(&mut self, app_server: &AppServerSession) {
        let backend_id = app_server.backend_id();
        let Some(thread_id) = self.current_displayed_thread_id() else {
            return;
        };
        if self.reconnect.offline || self.reconnect.cloud_resume.is_some() {
            return;
        }
        let (AppServerRequestHandle::Cloud(handle), Some(channel)) = (
            app_server.request_handle(),
            self.thread_event_channels.get(&thread_id),
        ) else {
            return;
        };
        if self.chat_widget.is_user_turn_pending_or_running() {
            return;
        }
        if self.chat_widget.needs_question_answer_history() {
            return;
        }
        let mut view = app_server.attachment_view(thread_id);
        let config = self.config.clone();
        let local_settings = self.local_settings.clone();
        let store = Arc::clone(&channel.store);
        self.reconnect.cloud_resume = Some(CloudResume {
            backend_id,
            thread_id,
            store: Arc::clone(&store),
            _input_hold: self.chat_widget.pause_input_for_resume_handoff(),
            phase: ResumePhase::Preparing,
            task: AbortOnDropHandle::new(tokio::spawn(async move {
                let (session, latest, terminal, anchor, evidence) = {
                    let store = tokio::time::timeout(CLOUD_REQUEST_TIMEOUT, store.lock())
                        .await
                        .map_err(|_| CloudHistoryCheck::LockTimedOut)?;
                    let saved = store.turns.iter().flat_map(|turn| {
                        turn.items.iter().map(move |item| {
                            (
                                item.id(),
                                Some(item),
                                item.completed_at_ms.is_some()
                                    || (item.started_at_ms.is_none()
                                        && turn.status == TurnStatus::Completed),
                            )
                        })
                    });
                    let live = store.buffer.iter().filter_map(|event| {
                        let ThreadBufferedEvent::Notification(event) = event else {
                            return None;
                        };
                        match event.as_ref() {
                            ServerNotification::ItemStarted(n) => {
                                Some((n.item.id(), Some(&n.item), false))
                            }
                            ServerNotification::ItemCompleted(n) => {
                                Some((n.item.id(), Some(&n.item), true))
                            }
                            ServerNotification::AgentMessageDelta(n) => {
                                Some((n.item_id.as_str(), None, false))
                            }
                            _ => None,
                        }
                    });
                    let mut order = HashMap::new();
                    let mut evidence = HashMap::new();
                    let mut anchor = None;
                    for (id, item, finished) in saved.chain(live) {
                        let next = order.len();
                        let rank = *order.entry(id.to_owned()).or_insert(next);
                        let Some(item) = item else { continue };
                        let reply = matches!(&item.payload, ThreadItemPayload::AgentMessage { .. });
                        if reply || rank == next {
                            anchor = anchor.max(Some((reply, rank, id)));
                        }
                        if reply && finished {
                            evidence.insert(id.to_owned(), Some(item.clone()));
                        } else if rank == next
                            && matches!(&item.payload, ThreadItemPayload::UserMessage { .. })
                        {
                            evidence.insert(id.to_owned(), None);
                        }
                    }
                    (
                        store
                            .session
                            .clone()
                            .ok_or_else(|| color_eyre::eyre::eyre!(REOPEN_CLOUD_THREAD))?,
                        store.latest_turn_id.clone(),
                        store.active_turn_id.is_none(),
                        anchor.map(|(_, _, id)| id.to_owned()),
                        evidence,
                    )
                };
                let (response, release) = handle
                    .resume_disconnected_thread(
                        thread_id.to_string(),
                        /*require_history*/ false,
                        CLOUD_REQUEST_TIMEOUT,
                    )
                    .await?;
                let Some(response) =
                    response.filter(|r| r.thread.id == session.thread_id.to_string())
                else {
                    color_eyre::eyre::bail!(REOPEN_CLOUD_THREAD);
                };
                for delay_ms in [0, 500, 1000] {
                    if delay_ms != 0 {
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                    // Each check starts at the newest saved turn; the client bounds each page.
                    let mut thread = response.thread.clone();
                    thread.turns.clear();
                    view.hydrate_initial_thread_history(
                        &mut thread,
                        /*turn_cursor*/ None,
                        /*item_cursor*/ None,
                        Some(&config),
                        Some(&local_settings),
                        HistoryHydrationScope::CloudRecovery,
                    )
                    .await?;
                    if !view.recovery_history_complete(thread_id) {
                        // A remaining item cursor means we spent the scan budget.
                        // Starting again at the newest item would repeat the same work.
                        if view.has_older_history(thread_id) {
                            break;
                        }
                        continue;
                    }
                    let turns = &mut thread.turns;
                    if latest.as_ref().is_some_and(|id| {
                        !turns.iter().any(|t| {
                            &t.id == id && (!terminal || t.status != TurnStatus::InProgress)
                        })
                    }) {
                        continue;
                    }
                    // A positively unfinished row cannot supersede a completion already shown.
                    for item in turns.iter_mut().flat_map(|turn| &mut turn.items) {
                        if item.started_at_ms.is_some()
                            && item.completed_at_ms.is_none()
                            && matches!(&item.payload, ThreadItemPayload::AgentMessage { .. })
                            && let Some(Some(known)) = evidence.get(item.id())
                        {
                            item.clone_from(known);
                        }
                    }
                    let loaded: HashMap<_, _> = turns
                        .iter()
                        .flat_map(Turn::item_payloads)
                        .map(|item| (item.id(), item))
                        .collect();
                    if anchor
                        .as_ref()
                        .is_some_and(|id| !loaded.contains_key(id.as_str()))
                        || evidence.iter().any(|(id, known)| match (known, loaded.get(id.as_str())) {
                            (None, Some(_)) => false,
                            (Some(known), Some(ThreadItemPayload::AgentMessage { text, .. })) => {
                                matches!(&known.payload, ThreadItemPayload::AgentMessage {text: prior,..}
                                    if prior != text && prior.starts_with(text.as_str()))
                            }
                            _ => true,
                        })
                    {
                        continue;
                    }
                    let started = AppServerStartedThread {
                        session,
                        blocks_direct_input: thread_blocks_direct_input(&thread),
                        turns: thread.turns,
                        task_tools_available: view.task_tools_available(thread_id),
                    };
                    let resume = (started, view.take_attachment_metadata(thread_id));
                    return Ok((Box::new(resume), Some(release)));
                }
                Err(CloudHistoryCheck::Unavailable.into())
            })),
        });
    }

    // Selecting a different attachment can close an in-progress install ACK.
    // Keep its result until the selection succeeds in case the worker already committed it.
    pub(super) fn interrupt_cloud_resume_for_navigation(&mut self) -> bool {
        if let Some(attempt) = self.reconnect.cloud_resume.as_mut()
            && attempt.installing()
        {
            attempt.phase = ResumePhase::InstallInterruptedByNavigation;
            return false;
        }
        self.reconnect.cloud_resume.take().is_some()
    }

    pub(super) async fn finish_cloud_resume(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        mut attempt: CloudResume,
        outcome: CloudResumeOutcome,
    ) -> Result<AppRunControl> {
        let backend_id = attempt.backend_id;
        let thread_id = attempt.thread_id;
        if app_server.backend_id() != backend_id
            || self.current_displayed_thread_id() != Some(thread_id)
            || self.chat_widget.thread_id() != Some(thread_id)
            || !self.chat_widget.thread_stream_disconnected
            || self.reconnect.offline
            || self
                .thread_event_channels
                .get(&thread_id)
                .is_none_or(|channel| {
                    !Arc::ptr_eq(&attempt.store, &channel.store)
                        || channel.backend_id.is_some_and(|owner| owner != backend_id)
                })
            || self
                .backend_registry
                .owner(thread_id)
                .is_some_and(|owner| owner != backend_id)
        {
            return Ok(AppRunControl::Continue);
        }

        let result = match outcome {
            Ok((resumed, Some(release))) => {
                attempt.phase = ResumePhase::Installing;
                attempt.task = AbortOnDropHandle::new(tokio::spawn(async move {
                    release.install().await?;
                    Ok((resumed, None))
                }));
                self.reconnect.cloud_resume = Some(attempt);
                return Ok(AppRunControl::Continue);
            }
            Ok((resumed, None)) => Ok(*resumed),
            Err(error) if attempt.installing() => Err(error
                .downcast::<io::Error>()
                .unwrap_or_else(|_| io::Error::other(REOPEN_CLOUD_THREAD))),
            Err(error) => Err(match error.downcast::<TypedRequestError>() {
                Ok(TypedRequestError::Transport { source, .. }) => source,
                Ok(TypedRequestError::Server { source, .. }) => io::Error::other(source.message),
                Ok(TypedRequestError::Deserialize { .. }) => io::Error::other(REOPEN_CLOUD_THREAD),
                Err(error) => match error.downcast::<CloudHistoryCheck>() {
                    Ok(reason) => io::Error::other(reason),
                    Err(_) => io::Error::other(REOPEN_CLOUD_THREAD),
                },
            }),
        };
        let channel = self.ensure_thread_channel(thread_id);
        channel.history_may_be_incomplete = true;
        if let Err(error) = result {
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionAborted | io::ErrorKind::TimedOut
            ) || (attempt.phase == ResumePhase::InstallInterruptedByNavigation
                && error.kind() == io::ErrorKind::BrokenPipe)
            {
                // The worker backs off before the next Attach. Keep the draft and queue paused.
                self.start_cloud_resume(app_server);
            } else {
                channel.mark_replay_only();
                self.chat_widget.pause_unavailable_thread();
                self.chat_widget.add_error_message(error.to_string());
            }
            return Ok(AppRunControl::Continue);
        }
        let (thread, metadata) = result?;
        app_server
            .install_attachment_metadata(thread_id, metadata, /*observed_receipt*/ false);
        let accepted_model = self.chat_widget.cloud_picker_selection.clone();
        let service_tier = self.chat_widget.configured_service_tier();
        let control = self
            .activate_prepared_session_from(
                tui,
                app_server,
                PreparedResume {
                    backend_id,
                    config: self.config.clone(),
                    local_settings: self.local_settings.clone(),
                    thread,
                    restored_blank_session: false,
                    read_only: false,
                    history_notice: None,
                    provider_error: None,
                },
                super::session_lifecycle::ResumeOrigin::CloudRecovery,
            )
            .await?;
        self.chat_widget.cloud_picker_selection = accepted_model;
        self.chat_widget.set_service_tier(service_tier);
        self.sync_active_thread_service_tier_to_cached_session()
            .await;
        Ok(control)
    }

    pub(super) fn backend_settings(&self) -> BackendSettings {
        BackendSettings {
            config: self.config.clone(),
            local_settings: self.local_settings.clone(),
            target: self.app_server_target.clone(),
        }
    }

    pub(super) async fn live_blank_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        thread: &codex_app_server_protocol::Thread,
    ) -> Option<AppServerStartedThread> {
        if !matches!(thread.status, codex_app_server_protocol::ThreadStatus::Idle) {
            return None;
        }
        let mut blank = self.agents_overview.blank_sessions.get(&thread_id)?.clone();

        // A turn can start while this backend is parked, without the TUI seeing it.
        let paginated = matches!(
            thread.history_mode,
            codex_app_server_protocol::ThreadHistoryMode::Paginated
        ) && app_server.supports_paginated_history();
        let untouched = if paginated {
            let page =
                app_server.thread_turns_page(thread_id, /*cursor*/ None, /*limit*/ 1);
            match self.wait_for_opening_task(tui, page).await {
                Ok(page) => Some(page.data.is_empty() && page.next_cursor.is_none()),
                Err(error) if Self::can_fallback_from_include_turns_error(&error) => Some(true),
                Err(error) if error.chain().any(|cause| matches!(
                    cause.downcast_ref::<TypedRequestError>(),
                    Some(TypedRequestError::Server { source, .. })
                        if crate::app_server_session::is_history_pagination_unsupported(source)
                )) => None,
                Err(_) => Some(false),
            }
        } else {
            None
        };
        let untouched = match untouched {
            Some(untouched) => untouched,
            None => match self
                .wait_for_opening_task(
                    tui,
                    app_server.thread_read(thread_id, /*include_turns*/ true),
                )
                .await
            {
                Ok(thread) => {
                    thread.turns.is_empty()
                        && matches!(thread.status, codex_app_server_protocol::ThreadStatus::Idle)
                }
                Err(error) => Self::can_fallback_from_include_turns_error(&error),
            },
        };
        if !untouched {
            return None;
        }
        // An untouched task may have no rollout to resume yet.
        blank.session.thread_name.clone_from(&thread.name);
        Some(blank)
    }

    pub(super) async fn prepare_target_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target_session: &SessionTarget,
        source: ResumeSource<'_>,
    ) -> std::result::Result<PreparedResume, AppRunControl> {
        if self
            .agents_overview
            .pending_archives
            .contains_key(&target_session.thread_id)
        {
            self.add_session_picker_error("This task is still being archived.".into());
            return Err(AppRunControl::Continue);
        }
        if let Some(owner) = self.backend_registry.owner(target_session.thread_id)
            && let Err(error) = target_session.ensure_backend(owner)
        {
            self.add_session_picker_error(error.to_string());
            return Err(AppRunControl::Continue);
        }
        let (mut resume_config, local_settings) = self
            .resume_config_for_target(tui, app_server, target_session, source)
            .await?;
        if let Err(error) = self.apply_runtime_policy_overrides(
            &mut resume_config,
            RuntimePolicyOverrideScope::ExplicitOnly,
        ) {
            self.add_session_picker_error(format!("{error:#}"));
            return Err(AppRunControl::Continue);
        }
        let permission_overrides = self.resume_permission_overrides(&resume_config);
        let model_settings = match source {
            ResumeSource::Active => self.resume_model_settings(),
            ResumeSource::Backend(_) => {
                super::config_persistence::resume_model_settings_for_overrides(
                    &resume_config,
                    &self.harness_overrides,
                )
            }
        };

        if let Some(history_mode) = target_session.history_mode {
            app_server.remember_thread_history_mode(target_session.thread_id, history_mode);
        }
        let thread_id = target_session.thread_id;
        let live_blank = if matches!(source, ResumeSource::Backend(_))
            && self.backend_registry.owner(thread_id) == Some(app_server.backend_id())
            && self.agents_overview.blank_sessions.contains_key(&thread_id)
            && let Ok(thread) = self
                .wait_for_opening_task(
                    tui,
                    app_server.thread_read(thread_id, /*include_turns*/ false),
                )
                .await
        {
            self.live_blank_session(tui, app_server, thread_id, &thread)
                .await
        } else {
            None
        };
        let restored_blank_session = live_blank.is_some();
        let resumed = match live_blank {
            Some(thread) => Ok(thread),
            None => {
                let resume = crate::boxed_future!(app_server.resume_thread_for_navigation(
                    &local_settings,
                    resume_config.clone(),
                    thread_id,
                    model_settings,
                    permission_overrides,
                ));
                self.wait_for_opening_task(tui, resume).await
            }
        };
        let mut history_notice = None;
        let mut provider_error = None;
        let (resumed, read_only) = match resumed {
            Ok(resumed) => (resumed, false),
            Err(err)
                if crate::app_server_session::is_active_writer_error(&err)
                    || crate::app_server_session::model_provider_error(&err).is_some() =>
            {
                provider_error = crate::app_server_session::model_provider_error(&err);
                let history = crate::boxed_future!(app_server.read_thread_for_viewing(
                    &resume_config,
                    &local_settings,
                    target_session.thread_id,
                ));
                match self.wait_for_opening_task(tui, history).await {
                    Ok((thread, notice)) => {
                        history_notice = notice;
                        (thread, true)
                    }
                    Err(read_err) => {
                        self.add_session_picker_error(format!(
                            "Failed to load chat history: {read_err}"
                        ));
                        return Err(AppRunControl::Continue);
                    }
                }
            }
            Err(err) => {
                let path_display = target_session.display_label();
                self.add_session_picker_error(format!(
                    "Failed to resume session from {path_display}: {err}"
                ));
                return Err(AppRunControl::Continue);
            }
        };
        Ok(PreparedResume {
            backend_id: app_server.backend_id(),
            config: resume_config,
            local_settings,
            thread: resumed,
            restored_blank_session,
            read_only,
            history_notice,
            provider_error,
        })
    }
}
