//! Reconstructs model context and preserves source runtime metadata across fork cutoffs.

use std::io;

use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::ModelContextScan;
use codex_rollout::ModelContextScanProgress;
use codex_rollout::ReverseJsonlScanner;
use codex_rollout::RolloutItem;
use codex_rollout::ScanOutcome;

use super::LocalThreadStore;
use super::read_thread;
use super::rollout_lineage::RolloutLineage;
use super::thread_rollout_resolver;
use crate::LoadThreadHistoryParams;
use crate::StoredModelContext;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[cfg(test)]
#[path = "model_context_tests.rs"]
mod tests;

/// Loads rollout items needed to reconstruct the latest model-visible context.
///
/// Paginated JSONL rollouts use a reverse scan. It stops at the newest `CompactedItem` with both
/// replacement history and a window number, and returns that compaction plus its newer suffix. If
/// the newest compaction lacks either field, the scan continues to the beginning of the rollout.
///
/// Compressed segments are decoded before applying their original JSONL offsets. Legacy rollouts
/// keep the existing full-history path.
pub(super) async fn load_latest_model_context(
    store: &LocalThreadStore,
    params: LoadThreadHistoryParams,
) -> ThreadStoreResult<StoredModelContext> {
    let resolved = if params.include_archived {
        thread_rollout_resolver::resolve_current_including_archived(store, params.thread_id).await?
    } else {
        thread_rollout_resolver::resolve_current(store, params.thread_id).await?
    };
    let path =
        resolved
            .map(|resolved| resolved.path)
            .ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("no rollout found for thread id {}", params.thread_id),
            })?;

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

    let items = if matches!(session_meta.meta.history_mode, ThreadHistoryMode::Paginated) {
        let lineage = store.resolve_rollout_lineage(params.thread_id).await?;
        scan_model_context_from_lineage(lineage, session_meta).await?
    } else {
        read_thread::load_history_items(path.as_path()).await?
    };

    Ok(StoredModelContext {
        thread_id: params.thread_id,
        items,
    })
}

/// Loads startup context from a fork's frozen inherited prefix.
pub(super) async fn load_for_fork(
    lineage: RolloutLineage,
    history_base: Option<HistoryPosition>,
) -> ThreadStoreResult<Vec<RolloutItem>> {
    let source_path = lineage
        .segments()
        .last()
        .map(|segment| segment.rollout_path.as_path())
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "fork lineage has no source segment".to_string(),
        })?;
    let mut session_meta = codex_rollout::read_session_meta_line(source_path)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read session metadata {}: {err}",
                source_path.display()
            ),
        })?;
    if session_meta.meta.multi_agent_version.is_none() {
        // Recover only the runtime version before applying the fork cutoff. Stop at the
        // newest version-bearing context instead of retaining the source's full replay.
        let source_lineage = lineage.clone();
        session_meta.meta.multi_agent_version = tokio::task::spawn_blocking(move || {
            for segment in source_lineage.segments().iter().rev() {
                let file =
                    codex_rollout::open_rollout_seekable_reader(segment.rollout_path.as_path())?;
                let mut scanner = match segment.end.map(|end| end.end_byte_offset) {
                    Some(end_byte_offset) => ReverseJsonlScanner::new_at(file, end_byte_offset)?,
                    None => ReverseJsonlScanner::new(file)?,
                };
                while let Some(outcome) = scanner.scan_next_rollout_line()? {
                    let ScanOutcome::Parsed(line) = outcome else {
                        continue;
                    };
                    if let Some(version) = codex_rollout::resume_multi_agent_version(&line.item) {
                        return Ok(Some(version));
                    }
                    // Ancestor metadata does not describe the immediate source's runtime.
                    if matches!(line.item, RolloutItem::SessionMeta(_)) {
                        break;
                    }
                }
            }
            Ok::<_, io::Error>(None)
        })
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to join fork runtime version scan: {err}"),
        })?
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read fork runtime version: {err}"),
        })?;
    }
    match history_base {
        Some(history_base) => {
            let lineage = lineage.truncate_at(history_base).await?;
            scan_model_context_from_lineage(lineage, session_meta).await
        }
        None => Ok(vec![RolloutItem::SessionMeta(session_meta)]),
    }
}

async fn scan_model_context_from_lineage(
    lineage: RolloutLineage,
    session_meta: SessionMetaLine,
) -> ThreadStoreResult<Vec<RolloutItem>> {
    let scan = tokio::task::spawn_blocking(move || {
        scan_model_context_from_lineage_blocking(&lineage, session_meta)
    })
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to join model context scan: {err}"),
    })?;
    match scan {
        Ok(items) => Ok(items),
        Err(err) => Err(ThreadStoreError::Internal {
            message: format!("failed to scan paginated model context lineage: {err}"),
        }),
    }
}

fn scan_model_context_from_lineage_blocking(
    lineage: &RolloutLineage,
    session_meta: SessionMetaLine,
) -> io::Result<Vec<RolloutItem>> {
    let mut scan = ModelContextScan::default();
    'segments: for segment in lineage.segments().iter().rev() {
        let file = codex_rollout::open_rollout_seekable_reader(segment.rollout_path.as_path())?;
        let mut scanner = match segment.end.map(|end| end.end_byte_offset) {
            Some(end_byte_offset) => ReverseJsonlScanner::new_at(file, end_byte_offset)?,
            None => ReverseJsonlScanner::new(file)?,
        };
        while let Some(outcome) = scanner.scan_next_rollout_line()? {
            let ScanOutcome::Parsed(line) = outcome else {
                continue;
            };
            // Each rollout segment contributes only its local delta. Its session metadata is
            // replaced with the requested thread's canonical SessionMeta after replay.
            if matches!(&line.item, RolloutItem::SessionMeta(_)) {
                break;
            }
            match scan.push(line.item) {
                ModelContextScanProgress::Continue => {}
                ModelContextScanProgress::Complete => break 'segments,
            }
        }
    }

    let mut items = scan.finish();
    items.insert(0, RolloutItem::SessionMeta(session_meta));
    Ok(items)
}
