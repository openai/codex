use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadHistoryMode;
use tracing::debug;

use super::LocalThreadStore;
use super::helpers::rollout_path_is_archived;
use super::read_thread;
use crate::LoadThreadHistoryParams;
use crate::StoredModelContext;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[cfg(test)]
#[path = "load_latest_model_context_tests.rs"]
mod tests;

const READ_CHUNK_SIZE: usize = 64 * 1024;

pub(super) async fn load_latest_model_context(
    store: &LocalThreadStore,
    params: LoadThreadHistoryParams,
) -> ThreadStoreResult<StoredModelContext> {
    let path = read_thread::resolve_rollout_path(store, params.thread_id, params.include_archived)
        .await?
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: format!("no rollout found for thread id {}", params.thread_id),
        })?;
    if !params.include_archived
        && rollout_path_is_archived(store.config.codex_home.as_path(), path.as_path())
    {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread {} is archived", params.thread_id),
        });
    }

    let session_meta = codex_rollout::read_session_meta_line(path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read session metadata {}: {err}", path.display()),
        })?;
    if session_meta.meta.id != params.thread_id {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout at {} belongs to thread {}, not {}",
                path.display(),
                session_meta.meta.id,
                params.thread_id
            ),
        });
    }

    let items = if matches!(session_meta.meta.history_mode, ThreadHistoryMode::Paginated)
        && !path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .is_some_and(|file_name| file_name.ends_with(".jsonl.zst"))
    {
        match scan_bounded_model_context(path.clone(), session_meta.clone()).await? {
            Some(items) => items,
            None => {
                debug!(
                    thread_id = %params.thread_id,
                    rollout_path = %path.display(),
                    "falling back to full rollout load for model context"
                );
                read_thread::load_history_items(path.as_path()).await?
            }
        }
    } else {
        read_thread::load_history_items(path.as_path()).await?
    };

    Ok(StoredModelContext {
        thread_id: params.thread_id,
        items,
    })
}

async fn scan_bounded_model_context(
    path: PathBuf,
    session_meta: SessionMetaLine,
) -> ThreadStoreResult<Option<Vec<RolloutItem>>> {
    let path_for_error = path.clone();
    tokio::task::spawn_blocking(move || scan_bounded_model_context_blocking(&path, session_meta))
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to join model context scan: {err}"),
        })?
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to scan model context {}: {err}",
                path_for_error.display()
            ),
        })
}

fn scan_bounded_model_context_blocking(
    path: &Path,
    session_meta: SessionMetaLine,
) -> io::Result<Option<Vec<RolloutItem>>> {
    let mut selector = ModelContextSelector::default();
    let mut items_newest_first = Vec::new();
    scan_rollout_from_end(path, |item| {
        let selection = selector.observe(&item);
        items_newest_first.push(item);
        Ok(selection)
    })?;

    if !selector.is_complete() {
        return Ok(None);
    }

    items_newest_first.reverse();
    // The head SessionMeta is canonical even when copied fork history contains later metadata.
    // A successful bounded scan stops at a turn boundary after the head, so this does not
    // duplicate the rollout's own first line.
    items_newest_first.insert(0, RolloutItem::SessionMeta(session_meta));
    Ok(Some(items_newest_first))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanControl {
    Continue,
    Stop,
    Fallback,
}

#[derive(Debug, Default)]
struct ModelContextSelector {
    saw_checkpoint: bool,
    saw_resume_metadata: bool,
    active_segment: ActiveSegment,
    fallback: bool,
}

impl ModelContextSelector {
    fn observe(&mut self, item: &RolloutItem) -> ScanControl {
        match item {
            RolloutItem::Compacted(compacted) => {
                if compacted.replacement_history.is_none() || compacted.window_number.is_none() {
                    self.fallback = true;
                    return ScanControl::Fallback;
                }
                if self.saw_checkpoint {
                    // A second checkpoint before a usable turn boundary means the selector cannot
                    // prove that the first checkpoint belongs to a surviving replay segment.
                    self.fallback = true;
                    return ScanControl::Fallback;
                }
                self.saw_checkpoint = true;
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)) => {
                // Paginated threads reject rollback. Keep old rollouts correct rather than
                // duplicating rollback survival semantics in this storage-only selector.
                self.fallback = true;
                return ScanControl::Fallback;
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                self.active_segment
                    .turn_id
                    .get_or_insert_with(|| event.turn_id.clone());
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                if let Some(turn_id) = &event.turn_id {
                    self.active_segment
                        .turn_id
                        .get_or_insert_with(|| turn_id.clone());
                }
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                if turn_ids_are_compatible(
                    self.active_segment.turn_id.as_deref(),
                    Some(event.turn_id.as_str()),
                ) {
                    self.finalize_active_segment();
                }
            }
            RolloutItem::TurnContext(context) => {
                if self.active_segment.turn_id.is_none() {
                    self.active_segment.turn_id = context.turn_id.clone();
                }
                if turn_ids_are_compatible(
                    self.active_segment.turn_id.as_deref(),
                    context.turn_id.as_deref(),
                ) {
                    self.active_segment.has_turn_context = true;
                }
            }
            RolloutItem::ResponseItem(response_item) => {
                self.active_segment.has_user_turn |=
                    matches!(response_item, ResponseItem::Message { role, .. } if role == "user");
            }
            RolloutItem::InterAgentCommunication(_) => {
                self.active_segment.has_user_turn = true;
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                self.active_segment.has_user_turn = true;
            }
            RolloutItem::EventMsg(_)
            | RolloutItem::SessionMeta(_)
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::WorldState(_) => {}
        }

        if self.is_complete() {
            ScanControl::Stop
        } else {
            ScanControl::Continue
        }
    }

    fn finalize_active_segment(&mut self) {
        if self.active_segment.has_user_turn && self.active_segment.has_turn_context {
            self.saw_resume_metadata = true;
        }
        self.active_segment = ActiveSegment::default();
    }

    fn is_complete(&self) -> bool {
        !self.fallback && self.saw_checkpoint && self.saw_resume_metadata
    }
}

#[derive(Debug, Default)]
struct ActiveSegment {
    turn_id: Option<String>,
    has_user_turn: bool,
    has_turn_context: bool,
}

fn turn_ids_are_compatible(active_turn_id: Option<&str>, item_turn_id: Option<&str>) -> bool {
    active_turn_id
        .is_none_or(|turn_id| item_turn_id.is_none_or(|item_turn_id| item_turn_id == turn_id))
}

fn scan_rollout_from_end(
    path: &Path,
    mut visit_item: impl FnMut(RolloutItem) -> io::Result<ScanControl>,
) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut remaining = file.metadata()?.len();
    let mut line_reversed = Vec::new();
    let mut buffer = vec![0u8; READ_CHUNK_SIZE];

    while remaining > 0 {
        let read_size =
            usize::try_from(remaining.min(READ_CHUNK_SIZE as u64)).map_err(io::Error::other)?;
        remaining -= read_size as u64;
        file.seek(SeekFrom::Start(remaining))?;
        file.read_exact(&mut buffer[..read_size])?;

        for &byte in buffer[..read_size].iter().rev() {
            if byte == b'\n' {
                if process_reversed_line(&mut line_reversed, &mut visit_item)? {
                    return Ok(());
                }
            } else {
                line_reversed.push(byte);
            }
        }
    }

    let _ = process_reversed_line(&mut line_reversed, &mut visit_item)?;
    Ok(())
}

fn process_reversed_line(
    line_reversed: &mut Vec<u8>,
    visit_item: &mut impl FnMut(RolloutItem) -> io::Result<ScanControl>,
) -> io::Result<bool> {
    if line_reversed.is_empty() {
        return Ok(false);
    }
    line_reversed.reverse();
    let line = std::str::from_utf8(line_reversed)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let parsed = serde_json::from_str::<RolloutLine>(line.trim());
    line_reversed.clear();
    let Ok(line) = parsed else {
        return Ok(false);
    };
    Ok(matches!(
        visit_item(line.item)?,
        ScanControl::Stop | ScanControl::Fallback
    ))
}
