//! Consumer task queries preserve missing amounts and require complete descendant groups.
use super::client::Live;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_backend_client::TaskUsage;
use codex_backend_client::TaskUsageThread;
use futures::StreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub(super) struct Chat {
    pub title: String,
    pub task: Option<TaskUsage>,
}
impl Chat {
    /// Local history can span accounts; unavailable task data does not verify ownership.
    pub(super) fn display_title(&self) -> &str {
        if self.task.as_ref().is_some_and(|task| {
            task.data_status != codex_backend_client::TaskUsageStatus::Unavailable
        }) {
            &self.title
        } else {
            "Chat usage unavailable"
        }
    }
}

#[derive(Default)]
pub(super) struct Chats {
    pub rows: Vec<Chat>,
    pub truncated: bool,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub(super) async fn read(
    handle: AppServerRequestHandle,
    live: Arc<Live>,
) -> Result<Option<Chats>, String> {
    let session = live.session().await?;
    if session.kind != super::models::AccountKind::Consumer {
        return Ok(None);
    }
    session.backend.ensure_identity().await?;
    let mut roots = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 60),
        super::chats::roots(&handle),
    )
    .await
    .map_err(|_| "Chat listing timed out. Press R to retry.".to_string())??;
    session.backend.ensure_identity().await?;
    roots.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    let truncated = roots.len() > 100;
    roots.truncate(/*len*/ 100);
    let discoveries = roots
        .iter()
        .map(|root| descendants(&handle, root))
        .collect::<Vec<_>>();
    let mut pending = futures::stream::iter(discoveries).buffer_unordered(/*n*/ 2);
    let mut queries = Vec::new();
    // Keep complete groups collected before the deadline; never query partial descendants.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 25);
    while let Ok(Some(result)) = tokio::time::timeout_at(deadline, pending.next()).await {
        if let Ok(query) = result {
            queries.push(query);
        }
    }
    drop(pending);
    let mut seen = HashSet::new();
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut count = 0;
    for query in queries {
        let ids = std::iter::once(&query.thread_id).chain(&query.descendant_thread_ids);
        if ids.clone().any(|id| !seen.insert(id.clone())) {
            return Err("Task descendant groups overlap.".into());
        }
        let size = 1 + query.descendant_thread_ids.len();
        if count + size > 1_000 || batch.len() == 100 {
            batches.push(std::mem::take(&mut batch));
            count = 0;
        }
        count += size;
        batch.push(query);
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    let requests = batches
        .into_iter()
        .map(|threads| async move {
            session
                .backend
                .request(|client| {
                    let threads = threads.clone();
                    async move { client.get_task_usage(&threads).await }
                })
                .await
        })
        .collect::<Vec<_>>();
    let expected_batches = requests.len();
    let mut completed_batches = 0;
    let mut pending = futures::stream::iter(requests).buffer_unordered(/*n*/ 2);
    let mut usage = HashMap::new();
    let mut freshness = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 25);
    while let Ok(Some(result)) = tokio::time::timeout_at(deadline, pending.next()).await {
        completed_batches += 1;
        match result {
            Ok(response) => {
                freshness.push(
                    response
                        .data_as_of
                        .as_deref()
                        .map(chrono::DateTime::parse_from_rfc3339)
                        .transpose()
                        .map_err(|_| "Invalid task usage timestamp.")?
                        .map(|time| time.with_timezone(&chrono::Utc)),
                );
                usage.extend(
                    response
                        .threads
                        .into_iter()
                        .map(|task| (task.thread_id.clone(), task)),
                );
            }
            Err(error)
                if error
                    .status()
                    .is_some_and(|status| matches!(status.as_u16(), 404 | 503)) =>
            {
                freshness.push(/*value*/ None);
            }
            Err(error) => return Err(super::client::request_error(error)),
        }
    }
    if completed_batches < expected_batches {
        freshness.push(/*value*/ None);
    }
    drop(pending);
    session.backend.ensure_identity().await?;
    Ok(Some(Chats {
        truncated,
        updated_at: if freshness.iter().all(Option::is_some) {
            freshness.into_iter().flatten().min()
        } else {
            None
        },
        rows: roots
            .into_iter()
            .map(|thread| Chat {
                task: usage.remove(&thread.id),
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
            .collect(),
    }))
}

async fn descendants(
    handle: &AppServerRequestHandle,
    root: &Thread,
) -> Result<TaskUsageThread, String> {
    let mut ids = HashSet::new();
    let mut parents = HashMap::new();
    for archived in [false, true] {
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let page: ThreadListResponse = handle
                .request_typed(ClientRequest::ThreadList {
                    request_id: RequestId::String(uuid::Uuid::new_v4().to_string()),
                    params: ThreadListParams {
                        ancestor_thread_id: Some(root.id.clone()),
                        archived: Some(archived),
                        cursor,
                        limit: Some(100),
                        ..super::chats::list_params()
                    },
                })
                .await
                .map_err(super::data::error)?;
            for thread in page.data {
                let parent = thread
                    .parent_thread_id
                    .filter(|_| thread.id != root.id)
                    .ok_or("Task descendant discovery is unsupported.")?;
                parents.insert(thread.id.clone(), parent);
                ids.insert(thread.id);
            }
            if ids.len() >= 1_000 {
                return Err("Task exceeds the reporting limit.".into());
            }
            match page.next_cursor {
                Some(next) if seen.insert(next.clone()) => cursor = Some(next),
                Some(_) => return Err("Task listing repeated a cursor.".into()),
                None => break,
            }
        }
    }
    // Older servers can ignore the ancestor filter. Prove every returned chain reaches this root.
    for id in &ids {
        let mut current = id;
        let mut chain = HashSet::new();
        while current != &root.id {
            if !chain.insert(current) {
                return Err("Task descendant cycle.".into());
            }
            current = parents
                .get(current)
                .ok_or("Task descendant discovery is incomplete.")?;
        }
    }
    let mut descendant_thread_ids = ids.into_iter().collect::<Vec<_>>();
    descendant_thread_ids.sort();
    Ok(TaskUsageThread {
        thread_id: root.id.clone(),
        created_at: (root.created_at > 0)
            .then(|| chrono::DateTime::from_timestamp(root.created_at, /*nsecs*/ 0))
            .flatten()
            .map(|time| time.to_rfc3339()),
        descendant_thread_ids,
    })
}

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
