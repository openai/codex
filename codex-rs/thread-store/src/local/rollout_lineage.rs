use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;

use super::LocalThreadStore;
use super::read_thread;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// One physical rollout range contributing to a logical paginated history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RolloutLineageSegment {
    pub(super) thread_id: ThreadId,
    pub(super) rollout_path: PathBuf,
    pub(super) end: Option<HistoryPosition>,
}

/// Ordered physical rollout ranges contributing to one logical forked history.
///
/// This is the only local abstraction that follows SessionMeta.history_base pointers. Readers
/// consume its bounded physical segments without resolving or mutating fork pointers themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RolloutLineage {
    pub(super) segments: Vec<RolloutLineageSegment>,
}

impl LocalThreadStore {
    pub(super) async fn resolve_rollout_lineage(
        &self,
        requested_thread_id: ThreadId,
    ) -> ThreadStoreResult<RolloutLineage> {
        let mut segments = Vec::new();
        let mut seen = HashSet::new();
        let mut thread_id = requested_thread_id;
        let mut end = None;

        loop {
            if !seen.insert(thread_id) {
                return Err(malformed_lineage(requested_thread_id, "cycle detected"));
            }
            let rollout_path =
                read_thread::resolve_rollout_path(self, thread_id, /*include_archived*/ true)
                    .await?
                    .ok_or_else(|| malformed_lineage(thread_id, "missing source rollout"))?;
            let meta = codex_rollout::read_session_meta_line(rollout_path.as_path())
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!(
                        "failed to read lineage metadata {}: {err}",
                        rollout_path.display()
                    ),
                })?;
            if meta.meta.id != thread_id {
                return Err(malformed_lineage(
                    requested_thread_id,
                    "source rollout belongs to another thread",
                ));
            }
            if meta.meta.history_mode != ThreadHistoryMode::Paginated {
                return Err(malformed_lineage(
                    requested_thread_id,
                    "source rollout is not paginated",
                ));
            }
            if let Some(end) = end {
                validate_cutoff_bounds(requested_thread_id, rollout_path.as_path(), &end).await?;
            }
            segments.push(RolloutLineageSegment {
                thread_id,
                rollout_path,
                end,
            });

            let Some(base) = meta.meta.history_base else {
                break;
            };
            thread_id = base.thread_id;
            end = Some(base);
        }

        segments.reverse();
        Ok(RolloutLineage { segments })
    }

    pub(super) async fn resolve_rollout_lineage_at(
        &self,
        end: HistoryPosition,
    ) -> ThreadStoreResult<RolloutLineage> {
        let mut lineage = self.resolve_rollout_lineage(end.thread_id).await?;
        let Some(segment) = lineage.segments.last_mut() else {
            return Err(ThreadStoreError::Internal {
                message: "rollout lineage has no segments".to_string(),
            });
        };
        validate_cutoff_bounds(end.thread_id, segment.rollout_path.as_path(), &end).await?;
        segment.end = Some(end);
        Ok(lineage)
    }
}

async fn validate_cutoff_bounds(
    requested_thread_id: ThreadId,
    rollout_path: &Path,
    end: &HistoryPosition,
) -> ThreadStoreResult<()> {
    if end.end_ordinal_exclusive == 0 {
        return Err(malformed_lineage(
            requested_thread_id,
            "cutoff cannot include source session metadata",
        ));
    }
    let file_len = tokio::fs::metadata(rollout_path)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read lineage metadata {}: {err}",
                rollout_path.display()
            ),
        })?
        .len();
    if end.end_byte_offset > file_len {
        return Err(malformed_lineage(
            requested_thread_id,
            "cutoff byte offset is past the source rollout",
        ));
    }
    Ok(())
}

fn malformed_lineage(thread_id: ThreadId, detail: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("invalid paginated history lineage for {thread_id}: {detail}"),
    }
}

#[cfg(test)]
#[path = "rollout_lineage_tests.rs"]
mod tests;
