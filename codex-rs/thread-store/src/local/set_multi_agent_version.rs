use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::read_session_meta_line;

use super::LocalThreadStore;
use super::live_writer;
use super::read_thread;
use crate::AppendThreadItemsParams;
use crate::ReadThreadParams;
use crate::SetMultiAgentVersionIfUnsetParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn set_multi_agent_version_if_unset(
    store: &LocalThreadStore,
    params: SetMultiAgentVersionIfUnsetParams,
) -> ThreadStoreResult<MultiAgentVersion> {
    let _permit = store
        .multi_agent_version_seed_semaphore
        .acquire()
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to lock thread multi-agent version: {err}"),
        })?;
    let has_live_writer = live_writer::rollout_path(store, params.thread_id)
        .await
        .is_ok();
    if has_live_writer {
        live_writer::persist_thread(store, params.thread_id).await?;
        live_writer::flush_thread(store, params.thread_id).await?;
    }

    let thread = read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id: params.thread_id,
            include_archived: params.include_archived,
            include_history: true,
        },
    )
    .await?;
    let rollout_path = thread
        .rollout_path
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("thread {} does not have a rollout path", params.thread_id),
        })?;
    let history = thread.history.ok_or_else(|| ThreadStoreError::Internal {
        message: format!("failed to load history for thread {}", params.thread_id),
    })?;
    if let Some(multi_agent_version) = history.items.iter().rev().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) if meta_line.meta.id == params.thread_id => {
            meta_line.meta.multi_agent_version
        }
        _ => None,
    }) {
        return Ok(multi_agent_version);
    }

    let mut session_meta =
        read_session_meta_line(&rollout_path)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to set thread multi-agent version: {err}"),
            })?;
    session_meta.git = None;
    session_meta.meta.multi_agent_version = Some(params.multi_agent_version);
    let item = RolloutItem::SessionMeta(session_meta);
    if has_live_writer {
        live_writer::append_items(
            store,
            AppendThreadItemsParams {
                thread_id: params.thread_id,
                items: vec![item],
            },
        )
        .await?;
    } else {
        append_rollout_item_to_path(&rollout_path, &item)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to set thread multi-agent version: {err}"),
            })?;
    }

    Ok(params.multi_agent_version)
}
