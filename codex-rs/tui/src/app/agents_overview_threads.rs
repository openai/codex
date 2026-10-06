//! Retains overview membership for this TUI, independently of server subscriptions.
//! A bounded recent seed runs at startup, after reconnect, and after event lag.
//! Discovery merges into retained membership without evicting rows.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::agents_overview_details::preview_agent_message;
use super::agents_overview_discovery::list_pinned_threads;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use crate::AppServerTarget;
use crate::app_event::AgentsOverviewThreadRefresh;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::chatwidget::ChatWidget;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
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
use codex_protocol::ThreadId;
use codex_protocol::protocol::SubAgentSource;
use std::collections::HashMap;
use uuid::Uuid;

const RECENT_DETAIL_LIMIT: usize = 10;

// Listing supplies membership and row metadata without replaying every historical rollout.
// Keep transcript previews fresh for recent and loaded tasks, plus notification-targeted reads.
fn detail_thread_ids<'a>(threads: impl Iterator<Item = &'a Thread>) -> Vec<ThreadId> {
    let mut threads: Vec<_> = threads.collect();
    threads.sort_by(|left, right| {
        right
            .recency_at
            .unwrap_or(right.updated_at)
            .cmp(&left.recency_at.unwrap_or(left.updated_at))
            .then_with(|| right.id.cmp(&left.id))
    });
    threads
        .into_iter()
        .enumerate()
        .filter(|(index, thread)| {
            *index < RECENT_DETAIL_LIMIT || thread.status != ThreadStatus::NotLoaded
        })
        .filter_map(|(_, thread)| ThreadId::from_string(&thread.id).ok())
        .collect()
}

impl App {
    pub(super) fn remove_agents_overview_thread(&mut self, thread_id: ThreadId) {
        self.prepare_agents_overview_removal(&std::collections::HashSet::from([thread_id]));
        self.agents_overview.removed_threads.insert(thread_id);
        if let Some(Some(thread)) = self.agents_overview.threads.remove(&thread_id)
            && !thread.ephemeral
            && thread.parent_thread_id.is_none()
            && !matches!(
                thread.source,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
            )
            && !self.agents_overview.hidden_threads.contains(&thread_id)
        {
            self.agents_overview.refill_count += 1;
        }
    }

    pub(super) fn show_more_agents_overview(&mut self, app_server: &AppServerSession) {
        if !self.agents_overview.discovery.has_more() {
            return;
        }
        self.agents_overview.show_more_requested |= self.agents_overview.refill_count == 0;
        self.start_agents_overview_refresh(app_server);
    }

    pub(super) fn track_agents_overview_notification(&mut self, notification: &ServerNotification) {
        let ServerNotificationThreadTarget::Thread(thread_id) =
            server_notification_thread_target(notification)
        else {
            return;
        };
        if matches!(
            notification,
            ServerNotification::TurnStarted(_)
                | ServerNotification::ThreadClosed(_)
                | ServerNotification::ThreadArchived(_)
                | ServerNotification::ThreadDeleted(_)
        ) {
            self.agents_overview.blank_sessions.remove(&thread_id);
        }
        self.track_agents_overview_activity(thread_id, notification);
        let thread = self
            .agents_overview
            .threads
            .get_mut(&thread_id)
            .and_then(Option::as_mut);
        match notification {
            ServerNotification::ThreadTokenUsageUpdated(usage) => {
                if self.agents_overview.threads.contains_key(&thread_id) {
                    self.agents_overview
                        .usage
                        .entry(thread_id)
                        .or_default()
                        .tokens = Some(usage.token_usage.total.clone());
                    self.repaint_agents_overview();
                }
            }
            ServerNotification::ThreadStarted(started) => {
                if started.thread.ephemeral {
                    return;
                }
                self.agents_overview.removed_threads.remove(&thread_id);
                let mut thread = started.thread.clone();
                thread.turns.clear();
                self.agents_overview.threads.insert(thread_id, Some(thread));
            }
            ServerNotification::ThreadArchived(_) | ServerNotification::ThreadDeleted(_) => {
                self.agents_overview
                    .requested_permission_profiles
                    .remove(&thread_id);
                self.agents_overview
                    .selected_permission_profiles
                    .remove(&thread_id);
                self.agents_overview.activity.remove(&thread_id);
                self.agents_overview.last_messages.remove(&thread_id);
                self.agents_overview.usage.remove(&thread_id);
                self.remove_agents_overview_thread(thread_id);
                self.agents_overview.refresh_thread_ids.remove(&thread_id);
            }
            ServerNotification::ThreadUnarchived(_) => {
                self.agents_overview.removed_threads.remove(&thread_id);
            }
            ServerNotification::ThreadClosed(_) => {
                self.agents_overview
                    .requested_permission_profiles
                    .remove(&thread_id);
                self.agents_overview.activity.remove(&thread_id);
                if let Some(usage) = self.agents_overview.usage.get_mut(&thread_id) {
                    usage.tokens = None;
                }
                if let Some(thread) = thread {
                    thread.status = ThreadStatus::NotLoaded;
                }
            }
            ServerNotification::ThreadReverted(_) => {
                self.agents_overview.activity.remove(&thread_id);
                self.agents_overview.last_messages.remove(&thread_id);
                if let Some(usage) = self.agents_overview.usage.get_mut(&thread_id) {
                    usage.tokens = None;
                }
                self.repaint_agents_overview();
            }
            ServerNotification::ThreadStatusChanged(status) => {
                if let Some(thread) = thread {
                    thread.status = status.status.clone();
                }
            }
            ServerNotification::ThreadNameUpdated(name) => {
                if let Some(thread) = thread {
                    thread.name.clone_from(&name.thread_name);
                }
            }
            ServerNotification::ThreadSettingsUpdated(settings) => {
                if !self.pending_server_profiles.contains_key(&thread_id)
                    && !self
                        .agents_overview
                        .requested_permission_profiles
                        .contains_key(&thread_id)
                    && self
                        .agents_overview
                        .selected_permission_profiles
                        .get(&thread_id)
                        != settings
                            .thread_settings
                            .active_permission_profile
                            .as_ref()
                            .map(|profile| &profile.id)
                {
                    self.agents_overview
                        .selected_permission_profiles
                        .remove(&thread_id);
                }
                if let Some(thread) = thread {
                    thread.cwd.clone_from(&settings.thread_settings.cwd);
                    thread.model = Some(settings.thread_settings.model.clone());
                    thread
                        .model_provider
                        .clone_from(&settings.thread_settings.model_provider);
                }
            }
            _ => return,
        }
        if !matches!(
            notification,
            ServerNotification::ThreadReverted(_) | ServerNotification::ThreadTokenUsageUpdated(_)
        ) && self.agents_overview.threads.contains_key(&thread_id)
        {
            self.agents_overview.refresh_thread_ids.insert(thread_id);
        }
        if self.agents_overview.request_id.is_some() {
            // Replay the latest notification of each kind after the read, in arrival order.
            // This also protects sessions whose metadata has not arrived in the initial seed.
            let pending = self
                .agents_overview
                .refresh_notifications
                .entry(thread_id)
                .or_default();
            pending.retain(|previous| {
                std::mem::discriminant(previous) != std::mem::discriminant(notification)
            });
            pending.push(notification.clone());
        }
    }

    pub(super) fn refresh_agents_overview_threads(&mut self, app_server: &AppServerSession) {
        self.agents_overview.pin_refresh_requested = true;
        self.agents_overview.refresh_thread_ids.extend(
            self.agents_overview
                .threads
                .iter()
                .filter_map(|(id, thread)| thread.is_none().then_some(*id)),
        );
        self.agents_overview
            .refresh_thread_ids
            .extend(detail_thread_ids(
                self.agents_overview.threads.values().flatten(),
            ));
        self.start_agents_overview_refresh(app_server);
    }

    pub(super) fn refresh_changed_agents_overview_threads(
        &mut self,
        app_server: &AppServerSession,
    ) {
        if self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_none()
            || (!self.agents_overview.initialized && self.agents_overview.request_id.is_none())
        {
            return;
        }
        self.start_agents_overview_refresh(app_server);
    }

    fn start_agents_overview_refresh(&mut self, app_server: &AppServerSession) {
        let visible = self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some();
        if !visible
            && (self.agents_overview.initialized
                || matches!(self.app_server_target, AppServerTarget::Embedded))
        {
            return;
        }
        if self.agents_overview.request_id.is_some() {
            self.agents_overview.refresh_pending = true;
            return;
        }
        if self.agents_overview.initialized
            && self.agents_overview.refresh_thread_ids.is_empty()
            && !self.agents_overview.show_more_requested
            && !(self.agents_overview.refill_count > 0 && self.agents_overview.discovery.has_more())
            && !self.agents_overview.pin_refresh_requested
        {
            return;
        }

        self.agents_overview.pin_refresh_requested = false;
        let request_id = Uuid::new_v4();
        self.agents_overview.request_id = Some(request_id);
        let initialized = self.agents_overview.initialized;
        let refill = self.agents_overview.refill_count;
        let show_more =
            refill == 0 && std::mem::take(&mut self.agents_overview.show_more_requested);
        self.agents_overview.refresh_show_more = show_more;
        let discover =
            !initialized || show_more || (refill > 0 && self.agents_overview.discovery.has_more());
        let limit = if initialized && refill > 0 {
            refill.min(10)
        } else {
            10
        };
        let mut discovery = discover.then(|| {
            if initialized {
                self.agents_overview.discovery.clone()
            } else {
                self.agents_overview.removed_threads.clear();
                Default::default()
            }
        });
        let mut thread_ids = std::mem::take(&mut self.agents_overview.refresh_thread_ids);
        self.agents_overview.active_refresh_thread_ids = thread_ids.clone();
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        self.agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .loading = discover;
        let refresh_task = tokio::spawn(async move {
            let result = async {
                let mut threads = HashMap::new();
                let mut last_messages = HashMap::new();
                let mut recent_seed_complete = true;
                let pinned_thread_ids = match list_pinned_threads(&request_handle).await {
                    Ok(Some(pinned_threads)) => {
                        let mut ids = Vec::with_capacity(pinned_threads.len());
                        for thread in pinned_threads {
                            if let Ok(thread_id) = ThreadId::from_string(&thread.id) {
                                ids.push(thread_id);
                                threads.insert(thread_id, Some(thread));
                            }
                        }
                        Some(Some(ids))
                    }
                    Ok(None) => Some(None),
                    Err(error) => {
                        tracing::warn!(%error, "failed to list pinned agent threads");
                        None
                    }
                };
                if let Some(discovery) = &mut discovery {
                    let loaded = async {
                        if initialized {
                            return Ok(ThreadLoadedListResponse {
                                data: Vec::new(),
                                next_cursor: None,
                            });
                        }
                        request_handle
                            .request_typed::<ThreadLoadedListResponse>(
                                ClientRequest::ThreadLoadedList {
                                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                                    params: ThreadLoadedListParams {
                                        cursor: None,
                                        limit: None,
                                    },
                                },
                            )
                            .await
                    };
                    let (loaded, (recent, complete)) =
                        tokio::join!(loaded, discovery.next_batch(&request_handle, limit));
                    let loaded = loaded.map_err(|error| error.to_string())?;
                    recent_seed_complete = complete;
                    thread_ids.extend(
                        loaded
                            .data
                            .into_iter()
                            .filter_map(|id| ThreadId::from_string(&id).ok()),
                    );
                    thread_ids.extend(detail_thread_ids(recent.iter()));
                    for thread in recent {
                        if let Ok(thread_id) = ThreadId::from_string(&thread.id) {
                            threads.insert(thread_id, Some(thread));
                        }
                    }
                }

                let mut reads = tokio::task::JoinSet::new();
                for thread_id in thread_ids {
                    threads.entry(thread_id).or_default();
                    let request_handle = request_handle.clone();
                    reads.spawn(async move {
                        match request_handle
                            .request_typed::<ThreadReadResponse>(ClientRequest::ThreadRead {
                                request_id: RequestId::String(Uuid::new_v4().to_string()),
                                params: ThreadReadParams {
                                    thread_id: thread_id.to_string(),
                                    include_turns: false,
                                },
                            })
                            .await
                        {
                            Ok(mut response) => {
                                let mut last_message = None;
                                if let Ok(turns) = request_handle
                                    .request_typed::<ThreadTurnsListResponse>(
                                        ClientRequest::ThreadTurnsList {
                                            request_id: RequestId::String(
                                                Uuid::new_v4().to_string(),
                                            ),
                                            params: ThreadTurnsListParams {
                                                thread_id: thread_id.to_string(),
                                                cursor: None,
                                                limit: Some(1),
                                                sort_direction: None,
                                                items_view: None,
                                            },
                                        },
                                    )
                                    .await
                                    && let Some(turn) = turns.data.first()
                                {
                                    if let Some(ThreadItem::UserMessage { content, .. }) =
                                        turn.items.first()
                                    {
                                        response.thread.preview =
                                            ChatWidget::user_message_display_from_inputs(content)
                                                .message;
                                    }
                                    last_message =
                                        turn.items.iter().rev().find_map(|item| match item {
                                            ThreadItem::AgentMessage { text, .. } => {
                                                Some(preview_agent_message(text))
                                            }
                                            _ => None,
                                        });
                                }
                                Some((thread_id, response.thread, last_message))
                            }
                            Err(error) => {
                                tracing::warn!(%thread_id, %error, "failed to read agent thread");
                                None
                            }
                        }
                    });
                    if reads.len() >= 16
                        && let Some(Ok(Some((thread_id, thread, last_message)))) =
                            reads.join_next().await
                    {
                        threads.insert(thread_id, Some(thread));
                        if let Some(message) = last_message {
                            last_messages.insert(thread_id, message);
                        }
                    }
                }
                while let Some(result) = reads.join_next().await {
                    if let Ok(Some((thread_id, thread, last_message))) = result {
                        threads.insert(thread_id, Some(thread));
                        if let Some(message) = last_message {
                            last_messages.insert(thread_id, message);
                        }
                    }
                }
                Ok(AgentsOverviewThreadRefresh {
                    threads,
                    last_messages,
                    recent_seed_complete,
                    discovery,
                    pinned_thread_ids,
                })
            }
            .await;
            app_event_tx.send(AppEvent::AgentsOverviewThreadsLoaded { request_id, result });
        });
        self.agents_overview.refresh_task = Some(refresh_task.abort_handle());
    }
}
