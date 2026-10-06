//! Demand-driven history discovery. Each source retains its cursor and unshown rows.
//! One activation requests at most one page per source, then publishes up to ten tasks.

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
use uuid::Uuid;

const PAGE_SIZE: u32 = 10;

pub(crate) async fn list_pinned_threads(
    handle: &AppServerRequestHandle,
) -> Result<Option<Vec<Thread>>, String> {
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
                        originators: None,
                        cursor: cursor.clone(),
                        limit: Some(100),
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
                    threads.extend(page.data.into_iter().filter(|thread| {
                        is_overview_thread(thread) && supports_shared_pinning(&thread.source)
                    }));
                    let Some(next_cursor) = page.next_cursor else {
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

fn is_overview_thread(thread: &Thread) -> bool {
    !thread.ephemeral
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
    rows: VecDeque<Thread>,
    exhausted: bool,
    sort_key: ThreadSortKey,
}

impl Default for SourcePage {
    fn default() -> Self {
        Self {
            cursor: None,
            rows: VecDeque::new(),
            exhausted: false,
            sort_key: ThreadSortKey::RecencyAt,
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
    ) -> (Vec<Thread>, bool) {
        let [interactive, other] = &mut self.sources;
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
                .filter_map(|(index, source)| source.rows.front().map(|row| (index, row)))
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
                        originators: None,
                        cursor: self.cursor.clone(),
                        limit: Some(PAGE_SIZE),
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
                    self.exhausted = page.next_cursor.is_none();
                    self.cursor = page.next_cursor;
                    self.rows
                        .extend(page.data.into_iter().filter(is_overview_thread));
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
