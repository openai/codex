//! Daemon-wide overview of loaded root sessions and their subagents.

use super::agents_overview_view::AgentsOverviewGroup;
use super::agents_overview_view::AgentsOverviewRow;
use super::agents_overview_view::AgentsOverviewView;
use super::*;
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
