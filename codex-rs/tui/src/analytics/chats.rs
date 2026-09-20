//! Discover recent local roots and load business chat credit estimates.

use super::client::Live;
use super::client::Session;
use super::data;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_backend_client::ThreadUsage;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub(super) struct Chat {
    pub title: String,
    pub usage: Option<ThreadUsage>,
}

impl Chat {
    /// Local history can span accounts; expose titles only with a returned account estimate.
    pub(super) fn display_title(&self) -> &str {
        if self.usage.is_some() {
            &self.title
        } else {
            "Chat usage unavailable"
        }
    }
}

#[derive(Default)]
pub(super) struct Chats {
    pub rows: Vec<Chat>,
}

pub(super) async fn read(
    handle: AppServerRequestHandle,
    live: Arc<Live>,
) -> Result<Option<Chats>, String> {
    let session = live.session().await?;
    if !super::models::thread_usage_supported(session.backend.account().plan_type) {
        return Ok(None);
    }
    live.ensure_identity().await?;
    let threads = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 60), roots(&handle))
        .await
        .map_err(|_| "Chat listing timed out. Press R to retry.".to_string());
    live.ensure_identity().await?;
    let threads = threads??;
    if threads.is_empty() {
        return Ok(Some(Chats { rows: Vec::new() }));
    }
    let ids = threads
        .iter()
        .map(|thread| thread.id.as_str())
        .collect::<Vec<_>>();
    // Repairing local history has its own budget; it must not consume estimate time.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 25);
    let mut data = Vec::new();
    for pair in ids.chunks(/*chunk_size*/ 200) {
        let midpoint = pair.len().min(/*other*/ 100);
        let (first, second) = tokio::join!(
            tokio::time::timeout_at(deadline, estimates(session, &pair[..midpoint])),
            tokio::time::timeout_at(deadline, estimates(session, &pair[midpoint..])),
        );
        live.ensure_identity().await?;
        for batch in [first, second].into_iter().flatten() {
            data.extend(batch?);
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
    }
    live.ensure_identity().await?;
    let mut usage: HashMap<_, _> = data
        .into_iter()
        .map(|row| (row.thread_id.clone(), row))
        .collect();
    let mut rows = threads
        .into_iter()
        .map(|thread| Chat {
            usage: usage.remove(&thread.id),
            title: thread
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| {
                    if thread.preview.trim().is_empty() {
                        "Untitled chat".into()
                    } else {
                        thread.preview
                    }
                }),
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| {
        std::cmp::Reverse(
            row.usage
                .as_ref()
                .map(|usage| usage.estimated_usage_credits_micros),
        )
    });
    Ok(Some(Chats { rows }))
}

/// Recent local roots are shared by consumer task and business credit reporting.
pub(super) async fn roots(
    handle: &AppServerRequestHandle,
) -> Result<Vec<codex_app_server_protocol::Thread>, String> {
    let cutoff = chrono::Utc::now().timestamp() - 30 * 24 * 60 * 60;
    let mut threads = Vec::new();
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    loop {
        let page: ThreadListResponse = handle
            .request_typed(ClientRequest::ThreadList {
                request_id: RequestId::String(uuid::Uuid::new_v4().to_string()),
                params: ThreadListParams {
                    cursor,
                    ..list_params()
                },
            })
            .await
            .map_err(data::error)?;
        let reached_cutoff = page.data.iter().any(|thread| thread.updated_at < cutoff);
        threads.extend(page.data.into_iter().filter(|thread| {
            thread.updated_at >= cutoff && !thread.ephemeral && thread.parent_thread_id.is_none()
        }));
        match page.next_cursor {
            Some(next) if !seen_cursors.insert(next.clone()) => {
                return Err("Chat listing repeated a cursor. Press R to retry.".into());
            }
            Some(next) if !reached_cutoff => cursor = Some(next),
            _ => break,
        }
    }
    // Keep repeated rows from overlapping pages out of the backend's distinct-ID batches.
    let mut seen_ids = HashSet::new();
    threads.retain(|thread| seen_ids.insert(thread.id.clone()));
    Ok(threads)
}

pub(super) fn list_params() -> ThreadListParams {
    ThreadListParams {
        limit: Some(100),
        sort_key: Some(ThreadSortKey::UpdatedAt),
        sort_direction: Some(SortDirection::Desc),
        model_providers: Some(Vec::new()),
        source_kinds: None,
        originators: None,
        archived: Some(false),
        cursor: None,
        section_id: None,
        project_id: None,
        cwd: None,
        use_state_db_only: false,
        search_term: None,
        parent_thread_id: None,
        ancestor_thread_id: None,
    }
}

/// Isolate unavailable estimates within the existing concurrency slot; keep healthy rows.
async fn estimates(session: &Session, ids: &[&str]) -> Result<Vec<ThreadUsage>, String> {
    let mut pending = if ids.is_empty() {
        Vec::new()
    } else {
        vec![ids]
    };
    let mut rows = Vec::new();
    // At most 15 requests per batch; leave remaining estimates unavailable during an outage.
    for _ in 0..15 {
        let Some(ids) = pending.pop() else { break };
        match session
            .backend
            .request(|client| async move { client.get_threads_usage(ids).await })
            .await
        {
            Ok(batch) => rows.extend(batch),
            Err(error) if error.status().is_some_and(|status| status.as_u16() == 503) => {
                if ids.len() > 1 {
                    let midpoint = ids.len().div_ceil(/*rhs*/ 2);
                    pending.push(&ids[midpoint..]);
                    pending.push(&ids[..midpoint]);
                }
            }
            Err(error) => return Err(super::client::request_error(error)),
        }
    }
    Ok(rows)
}

#[cfg(test)]
#[path = "chats_tests.rs"]
mod tests;
