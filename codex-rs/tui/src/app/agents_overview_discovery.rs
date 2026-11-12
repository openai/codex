//! Demand-driven history discovery. Each source retains its cursor and unshown rows.
//! Each new query scans the active backend from the start with its own paging cursor.

use super::App;
use crate::app_event::AppEvent;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::protocol::SubAgentSource;
use codex_state::PINNED_THREAD_SECTION_ID;
use std::collections::VecDeque;
use std::sync::PoisonError;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

const PAGE_SIZE: u32 = 10;

// Pair each row with its batched receipt before buffering or merging source pages.
fn listed_threads(page: ThreadListResponse) -> impl Iterator<Item = (Thread, bool)> {
    page.data.into_iter().map(move |thread| {
        let unread = page
            .read_states
            .as_ref()
            .and_then(|states| states.get(&thread.id))
            .is_some_and(|state| state.first_unread.is_some());
        (thread, unread)
    })
}

pub(crate) async fn list_pinned_threads(
    handle: &AppServerRequestHandle,
) -> Result<Option<Vec<(Thread, bool)>>, String> {
    let mut threads = Vec::new();
    for source_kinds in [
        Vec::new(),
        vec![ThreadSourceKind::Exec, ThreadSourceKind::AppServer],
    ] {
        let mut cursor = None;
        loop {
            let result = handle
                .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                    params: ThreadListParams {
                        include_attached_threads: false,
                        current_account_only: true,
                        excluded_thread_ids: None,
                        originators: None,
                        cursor: cursor.clone(),
                        limit: Some(100),
                        attached_to_thread_id: None,
                        sort_key: Some(ThreadSortKey::SectionPosition),
                        sort_direction: None,
                        model_providers: Some(Vec::new()),
                        source_kinds: Some(source_kinds.clone()),
                        archived: Some(false),
                        section_id: Some(Some(PINNED_THREAD_SECTION_ID.to_string())),
                        project_id: None,
                        parent_thread_id: None,
                        ancestor_thread_id: None,
                        cwd: None,
                        use_state_db_only: true,
                        search_term: None,
                    },
                })
                .await;
            match result {
                Ok(page) => {
                    let next_cursor = page.next_cursor.clone();
                    threads.extend(listed_threads(page).filter(|(thread, _)| {
                        is_overview_root(thread) && supports_shared_pinning(&thread.source)
                    }));
                    let Some(next_cursor) = next_cursor else {
                        break;
                    };
                    cursor = Some(next_cursor);
                }
                Err(TypedRequestError::Server { source, .. })
                    if matches!(source.code, -32602..=-32600) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    Ok(Some(threads))
}

pub(super) fn is_visible_in_agents_overview(thread: &Thread) -> bool {
    !thread.ephemeral && thread.attached_to_thread_id.is_none()
}

pub(super) fn is_overview_root(thread: &Thread) -> bool {
    is_visible_in_agents_overview(thread)
        && thread.parent_thread_id.is_none()
        && !matches!(
            thread.source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        )
}

pub(super) fn supports_shared_pinning(source: &SessionSource) -> bool {
    matches!(
        source,
        SessionSource::Cli | SessionSource::VsCode | SessionSource::Exec | SessionSource::AppServer
    ) || matches!(source, SessionSource::Custom(source) if matches!(source.as_str(), "atlas" | "chatgpt"))
}

#[derive(Clone, Debug)]
struct SourcePage {
    cursor: Option<String>,
    rows: VecDeque<(Thread, bool)>,
    exhausted: bool,
    sort_key: ThreadSortKey,
    page_size: u32,
}

impl Default for SourcePage {
    fn default() -> Self {
        Self {
            cursor: None,
            rows: VecDeque::new(),
            exhausted: false,
            sort_key: ThreadSortKey::RecencyAt,
            page_size: PAGE_SIZE,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AgentsOverviewDiscovery {
    sources: [SourcePage; 2],
}

impl AgentsOverviewDiscovery {
    pub(super) fn has_more(&self) -> bool {
        self.sources
            .iter()
            .any(|source| !source.exhausted || !source.rows.is_empty())
    }

    pub(super) async fn next_batch(
        &mut self,
        handle: &AppServerRequestHandle,
        limit: usize,
    ) -> (Vec<(Thread, bool)>, bool) {
        let [interactive, other] = &mut self.sources;
        // Cloud does not support filtering its task list to Exec and AppServer threads.
        if matches!(handle, AppServerRequestHandle::Cloud(_)) {
            other.exhausted = true;
        }
        let (left, right) = tokio::join!(
            interactive.fill(handle, Vec::new()),
            other.fill(
                handle,
                vec![ThreadSourceKind::Exec, ThreadSourceKind::AppServer]
            ),
        );
        let mut rows = Vec::new();
        for _ in 0..limit {
            let next = self
                .sources
                .iter()
                .enumerate()
                .filter_map(|(index, source)| {
                    source.rows.front().map(|(thread, _)| (index, thread))
                })
                .max_by(|(_, left), (_, right)| {
                    left.recency_at
                        .unwrap_or(left.updated_at)
                        .cmp(&right.recency_at.unwrap_or(right.updated_at))
                        .then_with(|| left.id.cmp(&right.id))
                })
                .map(|(index, _)| index);
            let Some(index) = next else { break };
            if let Some(row) = self.sources[index].rows.pop_front() {
                rows.push(row);
            }
        }
        (rows, left && right)
    }
}

impl SourcePage {
    async fn fill(
        &mut self,
        handle: &AppServerRequestHandle,
        source_kinds: Vec<ThreadSourceKind>,
    ) -> bool {
        if self.exhausted || self.rows.len() >= PAGE_SIZE as usize {
            return true;
        }
        loop {
            let result = handle
                .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                    params: ThreadListParams {
                        include_attached_threads: false,
                        current_account_only: true,
                        excluded_thread_ids: None,
                        attached_to_thread_id: None,
                        originators: None,
                        cursor: self.cursor.clone(),
                        limit: Some(self.page_size),
                        sort_key: Some(self.sort_key),
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
                .await;
            match result {
                Err(TypedRequestError::Server { source, .. })
                    if self.sort_key == ThreadSortKey::RecencyAt
                        && matches!(source.code, -32600 | -32602)
                        && source.message.contains("recency_at") =>
                {
                    self.sort_key = ThreadSortKey::UpdatedAt;
                    self.cursor = None;
                }
                Ok(page) => {
                    if page.next_cursor.is_some() && page.next_cursor == self.cursor {
                        return false;
                    }
                    self.exhausted = page.next_cursor.is_none();
                    self.cursor = page.next_cursor.clone();
                    self.rows.extend(
                        listed_threads(page).filter(|(thread, _)| is_overview_root(thread)),
                    );
                    return true;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to list more agent threads");
                    return false;
                }
            }
        }
    }
}

#[derive(Default)]
pub(super) struct HistorySearch {
    pub(super) id: Uuid,
    pub(super) cursor: AgentsOverviewDiscovery,
    /// Requested number of matches visible under the active status filter.
    pub(super) target: usize,
    pub(super) loading: bool,
    pub(super) failed: bool,
    pub(super) task: Option<AbortOnDropHandle<()>>,
}

pub(super) fn matches_query(thread: &Thread, query: &str) -> bool {
    let name = thread.name.as_deref().unwrap_or_default();
    let cwd = thread.cwd.display();
    format!("{name} {} {cwd}", thread.preview)
        .to_lowercase()
        .contains(query)
}

impl App {
    pub(super) fn search_agents_overview(
        &mut self,
        handle: AppServerRequestHandle,
        id: Uuid,
        limit: usize,
    ) {
        let shared = &self.agents_overview.view_state;
        let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let query = state.search.to_lowercase();
        let Some(search) = state.history.as_mut().filter(|search| search.id == id) else {
            return;
        };
        let first = self.agents_overview.search_id != id;
        let mut cursor = std::mem::take(&mut search.cursor);
        if first {
            self.agents_overview.search_matches.clear();
            self.agents_overview.search_id = id;
        }
        for source in &mut cursor.sources {
            source.page_size = 100;
        }
        if matches!(handle, AppServerRequestHandle::Cloud(_)) {
            cursor.sources[1].exhausted = true;
        }
        let tx = self.app_event_tx.clone();
        search.task = Some(AbortOnDropHandle::new(tokio::spawn(async move {
            if first {
                tokio::time::sleep(std::time::Duration::from_millis(180)).await;
            }
            loop {
                let (mut rows, ok) = cursor.next_batch(&handle, limit).await;
                rows.retain(|(thread, _)| matches_query(thread, &query));
                if rows.is_empty() && ok && cursor.has_more() {
                    continue;
                }
                tx.send(AppEvent::AgentsOverviewSearchPage {
                    id,
                    rows,
                    cursor,
                    ok,
                });
                return;
            }
        })));
    }

    pub(super) fn apply_agents_overview_search_page(
        &mut self,
        id: Uuid,
        rows: Vec<(Thread, bool)>,
        cursor: AgentsOverviewDiscovery,
        ok: bool,
    ) {
        let overview = &mut self.agents_overview;
        let mut state = overview
            .view_state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(search) = state.history.as_mut().filter(|s| s.id == id) else {
            return;
        };
        let backend_id = self.app_server_backend_id;
        for (thread, unread) in rows {
            if let Ok(id) = codex_protocol::ThreadId::from_string(&thread.id)
                && self.backend_registry.owner(id).or(backend_id) == backend_id
                && !overview.threads.get(&id).is_some_and(Option::is_some)
                && !overview.removed_threads.contains(&id)
                && !overview.hidden_threads.contains(&id)
                && let std::collections::hash_map::Entry::Vacant(entry) =
                    overview.search_matches.entry(id)
            {
                entry.insert(thread);
                if unread {
                    overview.unread.insert(id);
                } else {
                    overview.unread.remove(&id);
                }
            }
        }
        search.cursor = cursor;
        search.loading = false;
        search.failed = !ok;
        drop(state);
        self.repaint_agents_overview();
    }
}
