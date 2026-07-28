//! Root-scoped background refresh for the agent picker.

use super::*;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use std::collections::HashSet;

pub(super) const AGENT_PICKER_VIEW_ID: &str = "agent-picker";
const AGENT_PICKER_PAGE_SIZE: u32 = 100;
const AGENT_PICKER_MAX_THREADS: usize = 1_000;

impl App {
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
                let mut threads = Vec::new();
                let mut cursor = None;
                let mut seen_cursors = HashSet::new();
                while threads.len() < AGENT_PICKER_MAX_THREADS
                    && seen_cursors.insert(cursor.clone())
                {
                    let page = match request_handle
                        .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                            request_id: RequestId::String(Uuid::new_v4().to_string()),
                            params: ThreadListParams {
                                cursor,
                                limit: Some(AGENT_PICKER_PAGE_SIZE),
                                sort_key: None,
                                sort_direction: Some(SortDirection::Desc),
                                model_providers: Some(vec![]),
                                source_kinds: Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
                                archived: None,
                                section_id: None,
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
                        Err(err) if threads.is_empty() => return Err(err.to_string()),
                        Err(err) => {
                            tracing::warn!(%err, "failed to refresh remaining agent picker descendants");
                            break;
                        }
                    };
                    threads.extend(
                        page.data
                            .into_iter()
                            .take(AGENT_PICKER_MAX_THREADS - threads.len()),
                    );
                    let Some(next_cursor) = page.next_cursor else {
                        break;
                    };
                    cursor = Some(next_cursor);
                }
                threads.reverse();
                Ok(threads)
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
        root: ThreadId,
        request_id: Uuid,
        result: Result<Vec<Thread>, String>,
    ) {
        if !self
            .agent_navigation
            .finish_picker_refresh(root, request_id)
            || self.primary_thread_id != Some(root)
        {
            return;
        }
        let threads = match result {
            Ok(threads) => threads,
            Err(err) => {
                tracing::warn!(%err, "failed to refresh agent picker descendants");
                return;
            }
        };
        let selected = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID);
        for thread in threads {
            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
            let live = self
                .thread_event_channels
                .get(&thread_id)
                .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::Live);
            let previous = self.agent_navigation.get(&thread_id);
            let is_running = matches!(thread.status, ThreadStatus::Active { .. });
            let update_liveness = previous.is_none() || !is_running;
            let is_closed = !live && matches!(thread.status, ThreadStatus::NotLoaded);
            if !is_closed && previous.is_some_and(|entry| entry.is_closed) {
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
        }

        let params = self.agent_picker_selection_view_params(selected);
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params);
    }
}
