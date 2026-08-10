use std::path::Path;

use super::export::EXTERNAL_SESSION_IMPORTED_MARKER;
use super::ledger::checkpoint_existing_session_import;
use codex_core::ThreadManager;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutItem;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use tokio::sync::Semaphore;

/// A changed external session and the existing native thread it may extend.
pub struct ExistingSessionAppend<'a> {
    pub source_path: &'a Path,
    pub source_content_sha256: &'a str,
    pub expected_source_content_sha256: &'a str,
    pub thread_id: ThreadId,
    pub source_items: &'a [RolloutItem],
}

/// Appends an exact missing suffix to one cold imported thread and checkpoints the source.
///
/// Any unavailable, active, archived, malformed, or diverged destination fails closed.
pub async fn append_existing_session(
    codex_home: &Path,
    checkpoint_permits: &Semaphore,
    thread_manager: &ThreadManager,
    thread_store: &dyn ThreadStore,
    request: ExistingSessionAppend<'_>,
) -> bool {
    let ExistingSessionAppend {
        source_path,
        source_content_sha256,
        expected_source_content_sha256,
        thread_id,
        source_items,
    } = request;
    let Ok(mut thread) = thread_store
        .read_thread(ReadThreadParams {
            thread_id,
            include_archived: true,
            include_history: true,
        })
        .await
    else {
        return false;
    };
    if thread.thread_id != thread_id || thread.archived_at.is_some() {
        return false;
    }
    let Some(history) = thread.history.take() else {
        return false;
    };
    if history.thread_id != thread_id || thread_manager.get_thread(thread_id).await.is_ok() {
        return false;
    }
    let Some(metadata) = persistence_metadata(&history.items, thread_id) else {
        return false;
    };
    let Some(rollout_path) = thread.rollout_path else {
        return false;
    };
    if plan_append(source_items, &history.items).is_none() {
        return false;
    }
    if thread_store
        .resume_thread(ResumeThreadParams {
            thread_id,
            rollout_path: Some(rollout_path),
            history: None,
            include_archived: false,
            metadata,
        })
        .await
        .is_err()
    {
        return false;
    }

    let fresh_items = match thread_store
        .read_thread(ReadThreadParams {
            thread_id,
            include_archived: true,
            include_history: true,
        })
        .await
    {
        Ok(mut thread) if thread.thread_id == thread_id && thread.archived_at.is_none() => thread
            .history
            .take()
            .filter(|history| history.thread_id == thread_id)
            .filter(|history| persistence_metadata(&history.items, thread_id).is_some())
            .and_then(|history| plan_append(source_items, &history.items)),
        Ok(_) | Err(_) => None,
    };
    let Some(items) = fresh_items else {
        let _ = thread_store.discard_thread(thread_id).await;
        return false;
    };
    if thread_store
        .append_items(AppendThreadItemsParams { thread_id, items })
        .await
        .is_err()
    {
        let _ = thread_store.discard_thread(thread_id).await;
        return false;
    }
    if thread_store.shutdown_thread(thread_id).await.is_err() {
        let _ = thread_store.discard_thread(thread_id).await;
        return false;
    }

    let Ok(_checkpoint_permit) = checkpoint_permits.acquire().await else {
        return false;
    };
    let target_matches_source = match thread_store
        .read_thread(ReadThreadParams {
            thread_id,
            include_archived: true,
            include_history: true,
        })
        .await
    {
        Ok(mut thread) if thread.thread_id == thread_id && thread.archived_at.is_none() => thread
            .history
            .take()
            .filter(|history| history.thread_id == thread_id)
            .filter(|history| persistence_metadata(&history.items, thread_id).is_some())
            .is_some_and(|history| model_transcripts_match(source_items, &history.items)),
        Ok(_) | Err(_) => false,
    };
    if !target_matches_source {
        return false;
    }

    let codex_home = codex_home.to_path_buf();
    let source_path = source_path.to_path_buf();
    let expected_source_content_sha256 = expected_source_content_sha256.to_string();
    let source_content_sha256 = source_content_sha256.to_string();
    matches!(
        tokio::task::spawn_blocking(move || {
            checkpoint_existing_session_import(
                &codex_home,
                &source_path,
                thread_id,
                &expected_source_content_sha256,
                &source_content_sha256,
            )
        })
        .await,
        Ok(Ok(true))
    )
}

struct SourceModelItem<'a> {
    response_item: &'a ResponseItem,
    append_start_index: usize,
}

/// Returns the nonempty source suffix that can be safely appended to `history_items`.
///
/// The destination's complete model-visible transcript must be an exact prefix of the
/// source transcript. Rollout metadata does not participate in transcript identity.
fn plan_append(
    source_items: &[RolloutItem],
    history_items: &[RolloutItem],
) -> Option<Vec<RolloutItem>> {
    let source = source_model_items(source_items)?;
    let history = history_model_items(history_items)?;
    if history.len() >= source.len()
        || history
            .iter()
            .zip(&source)
            .any(|(history, source)| *history != source.response_item)
    {
        return None;
    }

    let append_start_index = source.get(history.len())?.append_start_index;
    let suffix = source_items[append_start_index..]
        .iter()
        .filter(|item| !is_import_marker(item))
        .cloned()
        .collect::<Vec<_>>();
    suffix
        .iter()
        .any(|item| matches!(item, RolloutItem::ResponseItem(_)))
        .then_some(suffix)
}

/// Returns whether source and destination contain the same complete model-visible transcript.
///
/// Rollout metadata does not participate in transcript identity. Unsupported history shapes fail
/// closed.
fn model_transcripts_match(source_items: &[RolloutItem], history_items: &[RolloutItem]) -> bool {
    let Some(source) = source_model_items(source_items) else {
        return false;
    };
    let Some(history) = history_model_items(history_items) else {
        return false;
    };
    history.len() == source.len()
        && history
            .iter()
            .zip(&source)
            .all(|(history, source)| *history == source.response_item)
}

fn source_model_items(items: &[RolloutItem]) -> Option<Vec<SourceModelItem<'_>>> {
    let mut model_items = Vec::new();
    let mut append_start_index = None;
    for (index, item) in items.iter().enumerate() {
        match item {
            RolloutItem::SessionMeta(_) | RolloutItem::InterAgentCommunicationMetadata { .. } => {}
            RolloutItem::ResponseItem(response_item) => {
                model_items.push(SourceModelItem {
                    response_item,
                    append_start_index: append_start_index.take().unwrap_or(index),
                });
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(_)) => {
                append_start_index = Some(index);
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                append_start_index.get_or_insert(index);
            }
            RolloutItem::EventMsg(EventMsg::AgentMessage(event))
                if event.message != EXTERNAL_SESSION_IMPORTED_MARKER =>
            {
                append_start_index = Some(index);
            }
            RolloutItem::EventMsg(
                EventMsg::ContextCompacted(_) | EventMsg::ThreadRolledBack(_),
            )
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::WorldState(_) => return None,
            RolloutItem::EventMsg(_) => {}
        }
    }
    Some(model_items)
}

fn history_model_items(items: &[RolloutItem]) -> Option<Vec<&ResponseItem>> {
    let mut model_items = Vec::new();
    for item in items {
        match item {
            RolloutItem::SessionMeta(_) | RolloutItem::InterAgentCommunicationMetadata { .. } => {}
            RolloutItem::ResponseItem(response_item) => model_items.push(response_item),
            RolloutItem::EventMsg(
                EventMsg::ContextCompacted(_) | EventMsg::ThreadRolledBack(_),
            )
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::WorldState(_) => return None,
            RolloutItem::EventMsg(_) => {}
        }
    }
    Some(model_items)
}

fn is_import_marker(item: &RolloutItem) -> bool {
    matches!(
        item,
        RolloutItem::EventMsg(EventMsg::AgentMessage(event))
            if event.message == EXTERNAL_SESSION_IMPORTED_MARKER
    )
}

fn persistence_metadata(
    history_items: &[RolloutItem],
    thread_id: ThreadId,
) -> Option<ThreadPersistenceMetadata> {
    let RolloutItem::SessionMeta(first_meta_line) = history_items.first()? else {
        return None;
    };
    if first_meta_line.meta.id != thread_id {
        return None;
    }
    let meta = history_items.iter().rev().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) if meta_line.meta.id == thread_id => {
            Some(&meta_line.meta)
        }
        _ => None,
    })?;
    if meta.cwd.as_os_str().is_empty()
        || meta
            .model_provider
            .as_deref()
            .is_none_or(|provider| provider.trim().is_empty())
    {
        return None;
    }
    let memory_mode = match meta.memory_mode.as_deref() {
        None | Some("enabled") => ThreadMemoryMode::Enabled,
        Some("disabled") => ThreadMemoryMode::Disabled,
        Some(_) => return None,
    };
    Some(ThreadPersistenceMetadata {
        cwd: Some(meta.cwd.clone()),
        model_provider: meta.model_provider.clone()?,
        memory_mode,
    })
}

#[cfg(test)]
#[path = "append_tests.rs"]
mod tests;
