//! Root-scoped background refresh for the agent picker.

use super::agent_navigation::AgentPickerThreadVisibility;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use super::sidebar_subagents::PendingLink;
use super::*;
use crate::app_event::AgentPickerThreadRefresh;
use crate::multi_agents::CollabHistoryCell;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use std::collections::HashSet;

pub(super) const AGENT_PICKER_VIEW_ID: &str = "agent-picker";
const AGENT_PICKER_PAGE_SIZE: u32 = 100;
const AGENT_PICKER_MAX_THREADS: usize = 1_000;

impl App {
    pub(super) fn prepare_collab_history(
        &mut self,
        app_server: &AppServerSession,
        cell: &mut dyn HistoryCell,
    ) {
        let Some(cell) = cell.as_any_mut().downcast_mut::<CollabHistoryCell>() else {
            return;
        };
        // A refresh can finish between queuing and inserting a history cell.
        if let Some(updated) = cell.refreshed(|id| self.chat_widget.collab_agent_metadata(id)) {
            *cell = updated;
        }
        if !self.reconnect.offline
            && let Some(root) = self.primary_thread_id
            && cell.needs_names()
            && (self.initial_history_replay_buffer.is_some()
                || !self.agent_navigation.queue_picker_refresh(root))
        {
            self.refresh_agent_picker_threads(app_server, root);
        }
    }

    pub(super) fn refresh_collab_history(&mut self) {
        let mut changed = false;
        let focused = (&mut self.transcript_cells, &mut self.transcript_view, true);
        let parked = self
            .side_companions
            .values_mut()
            .map(|pane| (&mut pane.cells, &mut pane.view, false));
        for (cells, view, is_focused) in std::iter::once(focused).chain(parked) {
            for index in 0..cells.len() {
                let Some(updated) = cells[index]
                    .as_any()
                    .downcast_ref::<CollabHistoryCell>()
                    .and_then(|cell| {
                        cell.refreshed(|id| self.chat_widget.collab_agent_metadata(id))
                    })
                else {
                    continue;
                };
                let replacement: Arc<dyn HistoryCell> = Arc::new(updated);
                let range = index..index + 1;
                view.replace_range(cells, range.clone(), &replacement);
                if is_focused {
                    // Update pending output without rewriting scrollback already handed to the terminal.
                    self.native_history
                        .consolidate(&cells[range.clone()], &replacement);
                    if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
                        overlay.consolidate_cells(range, Arc::clone(&replacement));
                    }
                }
                cells[index] = replacement;
                changed = true;
            }
        }
        if changed {
            self.chat_widget.request_redraw();
        }
    }

    pub(super) fn refresh_agent_picker_threads(
        &mut self,
        app_server: &AppServerSession,
        root: ThreadId,
    ) {
        let Some(request_id) = self.agent_navigation.begin_picker_refresh(root) else {
            return;
        };
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = async {
                let mut refresh = AgentPickerThreadRefresh::default();
                for archived in [false, true] {
                    let mut cursor = None;
                    let mut seen_cursors = HashSet::new();
                    let mut listed = 0;
                    while listed < AGENT_PICKER_MAX_THREADS && seen_cursors.insert(cursor.clone()) {
                        let page = match request_handle
                            .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                                request_id: RequestId::String(Uuid::new_v4().to_string()),
                                params: ThreadListParams {
                                    include_attached_threads: false,
                                    current_account_only: false,
                                    included_thread_ids: None,
                                    excluded_thread_ids: None,
                                    attached_to_thread_id: None,
                                    originators: None,
                                    cursor,
                                    limit: Some(AGENT_PICKER_PAGE_SIZE),
                                    sort_key: None,
                                    sort_direction: Some(SortDirection::Desc),
                                    model_providers: Some(vec![]),
                                    source_kinds: Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
                                    archived: Some(archived),
                                    section_id: None,
                                    project_id: None,
                                    cwd: None,
                                    use_state_db_only: true,
                                    search_term: None,
                                    parent_thread_id: None,
                                    ancestor_thread_id: Some(root.to_string()),
                                },
                            })
                            .await
                        {
                            Ok(page) => page,
                            Err(err) if !archived && listed == 0 => return Err(err.to_string()),
                            Err(err) => {
                                tracing::warn!(%err, archived, "incomplete agent picker refresh");
                                break;
                            }
                        };
                        let page_threads: Vec<_> = page
                            .data
                            .into_iter()
                            .take(AGENT_PICKER_MAX_THREADS - listed)
                            .collect();
                        listed += page_threads.len();
                        if archived {
                            for thread in page_threads {
                                if let Ok(id) = ThreadId::from_string(&thread.id) {
                                    refresh.archived_threads.insert(
                                        id,
                                        crate::multi_agents::AgentMetadata {
                                            agent_nickname: thread.agent_nickname,
                                            agent_role: thread.agent_role,
                                        },
                                    );
                                }
                            }
                        } else {
                            refresh.threads.extend(page_threads);
                        }
                        let Some(next_cursor) = page.next_cursor else {
                            break;
                        };
                        cursor = Some(next_cursor);
                    }
                }
                refresh.threads.reverse();
                Ok(refresh)
            }
            .await;

            app_event_tx.send(AppEvent::AgentPickerThreadsLoaded {
                primary_thread_id: root,
                request_id,
                result,
            });
        });
    }

    pub(super) fn apply_agent_picker_thread_refresh(
        &mut self,
        app_server: &AppServerSession,
        root: ThreadId,
        request_id: Uuid,
        result: Result<AgentPickerThreadRefresh, String>,
    ) -> Option<PendingLink> {
        let completion = self
            .agent_navigation
            .finish_picker_refresh(root, request_id)?;
        let pending_link = self
            .sidebars
            .get_mut(&Some(root))
            .and_then(|pane| pane.agents.pending_link.take());
        if self.primary_thread_id != Some(root) {
            return None;
        }
        let refresh = match result {
            Ok(refresh) => refresh,
            Err(err) => {
                tracing::warn!(%err, "failed to refresh agent picker descendants");
                self.agent_navigation.prune_untracked_picker_exclusions();
                if completion.follow_up_requested {
                    self.refresh_agent_picker_threads(app_server, root);
                }
                return None;
            }
        };
        for thread in refresh.threads {
            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
            let sidebar_agent = self.confirm_sidebar_agent(root, thread_id);
            if !completion.visibility_changed_threads.contains(&thread_id) {
                self.update_agent_picker_thread_visibility(
                    thread_id,
                    AgentPickerThreadVisibility::Visible,
                );
            }
            let live = self
                .thread_event_channels
                .get(&thread_id)
                .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::Live);
            let previous = self.agent_navigation.get(&thread_id);
            let is_running = matches!(thread.status, ThreadStatus::Active { .. });
            let update_liveness = previous.is_none() || !is_running;
            let is_closed = !live && matches!(thread.status, ThreadStatus::NotLoaded);
            if !is_closed && let Some(closed) = previous.filter(|entry| entry.is_closed) {
                // The child may have closed while discovery was in flight. Its name is
                // still useful, even though the list cannot reopen it.
                if closed.agent_nickname.is_none() && thread.agent_nickname.is_some() {
                    let role = closed.agent_role.clone().or(thread.agent_role);
                    self.upsert_agent_picker_thread(
                        thread_id,
                        thread.agent_nickname,
                        role,
                        /*is_closed*/ true,
                    );
                }
                continue;
            }
            let agent_path = crate::app_server_session::source_agent_path(&thread.source);
            let agent_nickname = thread
                .agent_nickname
                .or_else(|| previous.and_then(|entry| entry.agent_nickname.clone()));
            let agent_role = thread
                .agent_role
                .or_else(|| previous.and_then(|entry| entry.agent_role.clone()));
            if thread.can_accept_direct_input == Some(false) {
                self.agent_navigation.mark_parent_owned(thread_id);
            }
            self.upsert_agent_picker_thread(thread_id, agent_nickname, agent_role, is_closed);
            self.agent_navigation.set_agent_path(thread_id, agent_path);
            if !live && update_liveness {
                self.agent_navigation.set_running(thread_id, is_running);
            }
            if sidebar_agent && !live && (is_running || thread.status == ThreadStatus::SystemError)
            {
                self.seed_discovered_sidebar_agent(root, thread_id, &thread.status);
            }
        }
        for (thread_id, metadata) in refresh.archived_threads {
            // Archived children can label old rows without joining the picker.
            if metadata.agent_nickname.is_some()
                && self
                    .chat_widget
                    .collab_agent_metadata(thread_id)
                    .agent_nickname
                    .is_none()
            {
                self.chat_widget.set_collab_agent_metadata(
                    thread_id,
                    metadata.agent_nickname,
                    metadata.agent_role,
                );
            }
            if !completion.visibility_changed_threads.contains(&thread_id) {
                self.update_agent_picker_thread_visibility(
                    thread_id,
                    AgentPickerThreadVisibility::Hidden,
                );
            }
        }
        self.agent_navigation.prune_untracked_picker_exclusions();

        self.refresh_collab_history();
        self.refresh_agent_picker();
        if completion.follow_up_requested {
            self.refresh_agent_picker_threads(app_server, root);
        }
        pending_link.filter(|link| link.backend == app_server.backend_id())
    }

    pub(super) fn set_agent_picker_thread_visibility(
        &mut self,
        thread_id: ThreadId,
        visibility: AgentPickerThreadVisibility,
    ) {
        self.update_agent_picker_thread_visibility(thread_id, visibility);
        self.refresh_agent_picker();
    }

    fn update_agent_picker_thread_visibility(
        &mut self,
        thread_id: ThreadId,
        visibility: AgentPickerThreadVisibility,
    ) {
        if matches!(visibility, AgentPickerThreadVisibility::Hidden) {
            self.agent_navigation.mark_stopped(thread_id);
            if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
                channel.mark_replay_only();
            }
        }
        self.agent_navigation
            .set_picker_thread_visibility(thread_id, visibility);
    }

    pub(super) fn handle_agent_picker_visibility_notification(
        &mut self,
        app_server: &AppServerSession,
        notification: &ServerNotification,
    ) {
        let visibility = match notification {
            ServerNotification::ThreadArchived(_) => AgentPickerThreadVisibility::Hidden,
            ServerNotification::ThreadUnarchived(_) => AgentPickerThreadVisibility::Visible,
            _ => return,
        };
        let ServerNotificationThreadTarget::Thread(thread_id) =
            server_notification_thread_target(notification)
        else {
            return;
        };
        if matches!(notification, ServerNotification::ThreadArchived(_))
            && (self.side_threads.contains_key(&thread_id)
                || self
                    .sidebars
                    .values()
                    .any(|pane| pane.chat_tabs().choice_index(thread_id).is_some()))
        {
            self.app_event_tx.send(AppEvent::SideChatArchived {
                backend_id: app_server.backend_id(),
                target: thread_id,
            });
        }
        let refresh_uncached_thread = matches!(visibility, AgentPickerThreadVisibility::Visible)
            && self.agent_navigation.get(&thread_id).is_none();
        self.set_agent_picker_thread_visibility(thread_id, visibility);
        if refresh_uncached_thread
            && let Some(primary_thread_id) = self.primary_thread_id
            && !self
                .agent_navigation
                .queue_picker_refresh(primary_thread_id)
        {
            self.refresh_agent_picker_threads(app_server, primary_thread_id);
        }
    }

    /// Keep the approval notice and any open picker in sync with the same navigation state.
    pub(super) fn refresh_agent_picker(&mut self) -> bool {
        self.sync_active_agent_label();
        let pending = self
            .agent_navigation
            .pending_approval_threads()
            .map(|id| self.thread_label(id))
            .collect();
        self.chat_widget.set_pending_thread_approvals(pending);
        if !self.chat_widget.has_active_modal() {
            return false;
        }
        let selected = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID)
            .and_then(|index| self.agent_navigation.picker_row_ids.get(index).copied());
        let params = self.agent_picker_selection_view_params(selected);
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params)
    }
}
