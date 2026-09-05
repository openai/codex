//! Retains overview membership for this TUI, independently of server subscriptions.
//! The loaded/recent seed runs at startup and after reconnect; other reads only refresh metadata.
//! Discovery merges into retained membership without evicting rows.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::agents_overview_details::preview_text;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use crate::AppServerTarget;
use crate::app_event::AgentsOverviewThreadRefresh;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::chatwidget::ChatWidget;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SubAgentSource;
use std::collections::HashMap;
use uuid::Uuid;

impl App {
    pub(super) fn track_agents_overview_notification(&mut self, notification: &ServerNotification) {
        let ServerNotificationThreadTarget::Thread(thread_id) =
            server_notification_thread_target(notification)
        else {
            return;
        };
        self.track_agents_overview_activity(thread_id, notification);
        let thread = self
            .agents_overview
            .threads
            .get_mut(&thread_id)
            .and_then(Option::as_mut);
        match notification {
            ServerNotification::ThreadStarted(started) => {
                if started.thread.ephemeral {
                    return;
                }
                let mut thread = started.thread.clone();
                thread.turns.clear();
                self.agents_overview.threads.insert(thread_id, Some(thread));
            }
            ServerNotification::ThreadArchived(_) | ServerNotification::ThreadDeleted(_) => {
                self.agents_overview.activity.remove(&thread_id);
                self.agents_overview.last_messages.remove(&thread_id);
                self.agents_overview.threads.remove(&thread_id);
                self.agents_overview.refresh_thread_ids.remove(&thread_id);
            }
            ServerNotification::ThreadClosed(_) => {
                self.agents_overview.activity.remove(&thread_id);
                if let Some(thread) = thread {
                    thread.status = ThreadStatus::NotLoaded;
                }
            }
            ServerNotification::ThreadReverted(_) => {
                self.agents_overview.activity.remove(&thread_id);
                self.agents_overview.last_messages.remove(&thread_id);
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
                if let Some(thread) = thread {
                    thread.cwd.clone_from(&settings.thread_settings.cwd);
                    thread
                        .model_provider
                        .clone_from(&settings.thread_settings.model_provider);
                }
            }
            _ => return,
        }
        if !matches!(notification, ServerNotification::ThreadReverted(_))
            && self.agents_overview.threads.contains_key(&thread_id)
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
        self.agents_overview
            .refresh_thread_ids
            .extend(self.agents_overview.threads.keys());
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
        if self.agents_overview.initialized && self.agents_overview.refresh_thread_ids.is_empty() {
            return;
        }

        let request_id = Uuid::new_v4();
        self.agents_overview.request_id = Some(request_id);
        let initialized = self.agents_overview.initialized;
        let mut thread_ids = std::mem::take(&mut self.agents_overview.refresh_thread_ids);
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let refresh_task = tokio::spawn(async move {
            let result = async {
                let mut threads = HashMap::new();
                let mut last_messages = HashMap::new();
                let mut recent_seed_complete = true;
                if !initialized {
                    let loaded = request_handle.request_typed::<ThreadLoadedListResponse>(
                        ClientRequest::ThreadLoadedList {
                            request_id: RequestId::String(Uuid::new_v4().to_string()),
                            params: ThreadLoadedListParams {
                                cursor: None,
                                limit: None,
                            },
                        },
                    );
                    let list_recent = async |source_kinds: Vec<ThreadSourceKind>| {
                        let mut recent = Vec::new();
                        let mut cursor = None;
                        let mut sort_key = ThreadSortKey::RecencyAt;
                        while recent.len() < 20 {
                            let page = match request_handle
                                .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                                    params: ThreadListParams {
                                        originators: None,
                                        cursor,
                                        limit: Some(20),
                                        sort_key: Some(sort_key),
                                        sort_direction: None,
                                        model_providers: Some(Vec::new()),
                                        source_kinds: Some(source_kinds.clone()),
                                        archived: Some(false),
                                        section_id: None,
                                        project_id: None,
                                        parent_thread_id: None,
                                        ancestor_thread_id: None,
                                        cwd: None,
                                        use_state_db_only: true,
                                        search_term: None,
                                    },
                                })
                                .await
                            {
                                Err(TypedRequestError::Server { source, .. })
                                    if sort_key == ThreadSortKey::RecencyAt
                                        && matches!(source.code, -32600 | -32602)
                                        && source.message.contains("recency_at") =>
                                {
                                    // Older servers can still provide their activity-sorted history.
                                    sort_key = ThreadSortKey::UpdatedAt;
                                    cursor = None;
                                    recent.clear();
                                    continue;
                                }
                                result => result?,
                            };
                            recent.extend(
                                page.data
                                    .into_iter()
                                    .filter(|thread| {
                                        !thread.ephemeral
                                            && thread.parent_thread_id.is_none()
                                            && !matches!(
                                                thread.source,
                                                SessionSource::SubAgent(
                                                    SubAgentSource::ThreadSpawn { .. }
                                                )
                                            )
                                    })
                                    .take(20 - recent.len()),
                            );
                            cursor = page.next_cursor;
                            if cursor.is_none() {
                                break;
                            }
                        }
                        Ok::<_, TypedRequestError>(recent)
                    };
                    let recent = async {
                        // Default interactive sources include Atlas/ChatGPT, which have no
                        // explicit source kind. Exec/AppServer require a separate query.
                        let (interactive, non_interactive) = tokio::join!(
                            list_recent(Vec::new()),
                            list_recent(vec![ThreadSourceKind::Exec, ThreadSourceKind::AppServer]),
                        );
                        let mut recent = interactive?;
                        recent.extend(non_interactive?);
                        recent.sort_by(|left, right| {
                            right
                                .recency_at
                                .unwrap_or(right.updated_at)
                                .cmp(&left.recency_at.unwrap_or(left.updated_at))
                                .then_with(|| right.id.cmp(&left.id))
                        });
                        recent.truncate(20);
                        Ok::<_, TypedRequestError>(recent)
                    };
                    let (loaded, recent) = tokio::join!(loaded, recent);
                    let loaded = loaded.map_err(|error| error.to_string())?;
                    let recent = recent.unwrap_or_else(|error| {
                        tracing::warn!(%error, "failed to list recent agent threads");
                        recent_seed_complete = false;
                        Vec::new()
                    });
                    thread_ids.extend(
                        loaded
                            .data
                            .into_iter()
                            .filter_map(|id| ThreadId::from_string(&id).ok()),
                    );
                    // Keep list metadata even if a subsequent read fails transiently.
                    for thread in recent {
                        if let Ok(thread_id) = ThreadId::from_string(&thread.id) {
                            threads.insert(thread_id, Some(thread));
                            thread_ids.insert(thread_id);
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
                                                Some(preview_text(text))
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
                })
            }
            .await;
            app_event_tx.send(AppEvent::AgentsOverviewThreadsLoaded { request_id, result });
        });
        self.agents_overview.refresh_task = Some(refresh_task.abort_handle());
    }
}
