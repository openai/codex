use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;

use super::LocalThreadStore;
use super::thread_rollout_resolver;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// One immutable rollout range contributing to a paginated thread's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RolloutLineageSegment {
    pub(super) rollout_id: ThreadId,
    pub(super) rollout_path: PathBuf,
    pub(super) start_ordinal: u64,
    pub(super) end: Option<HistoryPosition>,
}

/// Ordered rollout ranges contributing to one forked history.
///
/// This is the only local abstraction that follows SessionMeta.history_base pointers. Readers
/// consume its bounded rollout segments without resolving or mutating fork pointers themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RolloutLineage {
    pub(super) segments: Vec<RolloutLineageSegment>,
}

impl LocalThreadStore {
    pub(super) async fn resolve_rollout_lineage(
        &self,
        requested_thread_id: ThreadId,
        initial_path: Option<PathBuf>,
    ) -> ThreadStoreResult<RolloutLineage> {
        self.resolve_rollout_lineage_with_representation(
            requested_thread_id,
            LineageRepresentation::Existing,
            initial_path,
        )
        .await
    }

    pub(super) async fn resolve_rollout_lineage_for_reference(
        &self,
        requested_thread_id: ThreadId,
    ) -> ThreadStoreResult<RolloutLineage> {
        self.resolve_rollout_lineage_with_representation(
            requested_thread_id,
            LineageRepresentation::PlainForReference,
            /*initial_path*/ None,
        )
        .await
    }

    async fn resolve_rollout_lineage_with_representation(
        &self,
        requested_thread_id: ThreadId,
        representation: LineageRepresentation,
        mut initial_path: Option<PathBuf>,
    ) -> ThreadStoreResult<RolloutLineage> {
        let mut segments = Vec::new();
        let mut seen = HashSet::new();
        let mut next_rollout_id = None;
        let mut end = None;

        loop {
            let coordination_id = next_rollout_id.unwrap_or(requested_thread_id);
            let _writer_guard = match representation {
                LineageRepresentation::Existing => None,
                LineageRepresentation::PlainForReference => {
                    Some(self.live_writer_locks.lock(coordination_id).await)
                }
            };
            let (rollout_id, rollout_path) = match (next_rollout_id, initial_path.take()) {
                (Some(rollout_id), _) => {
                    let rollout_path = resolve_rollout_path_by_id(self, rollout_id)
                        .await?
                        .ok_or_else(|| malformed_lineage(rollout_id, "missing source rollout"))?;
                    (rollout_id, rollout_path)
                }
                (None, Some(path)) => {
                    let rollout_id =
                        thread_rollout_resolver::rollout_id_from_path_or_legacy_thread_id(
                            &path,
                            requested_thread_id,
                            ThreadHistoryMode::Paginated,
                        )?;
                    (rollout_id, path)
                }
                (None, None) => {
                    let resolved = thread_rollout_resolver::resolve_current_including_archived(
                        self,
                        requested_thread_id,
                    )
                    .await?
                    .ok_or_else(|| {
                        malformed_lineage(requested_thread_id, "missing source rollout")
                    })?;
                    (resolved.rollout_id, resolved.path)
                }
            };
            if !seen.insert(rollout_id) {
                return Err(malformed_lineage(requested_thread_id, "cycle detected"));
            }
            let rollout_path = match representation {
                LineageRepresentation::Existing => rollout_path,
                LineageRepresentation::PlainForReference => {
                    let outside_codex_home = || ThreadStoreError::InvalidRequest {
                        message: format!(
                            "rollout path `{}` must be in Codex home directory",
                            rollout_path.display()
                        ),
                    };
                    let canonical_rollout_path = std::fs::canonicalize(rollout_path.as_path())
                        .map_err(|_| outside_codex_home())?;
                    // Resume can retain either the logical Codex home path or its canonical
                    // target. Keep references inside canonical managed roots so nested symlinks
                    // cannot escape them.
                    let is_managed_rollout = [
                        self.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR),
                        self.config
                            .codex_home
                            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
                    ]
                    .into_iter()
                    .filter_map(|root| std::fs::canonicalize(root).ok())
                    .any(|root| canonical_rollout_path.starts_with(root));
                    if !is_managed_rollout {
                        return Err(outside_codex_home());
                    }
                    canonical_rollout_path
                }
            };
            let meta = codex_rollout::read_session_meta_line(rollout_path.as_path())
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!(
                        "failed to read lineage metadata {}: {err}",
                        rollout_path.display()
                    ),
                })?;
            if next_rollout_id.is_none() && meta.meta.id != requested_thread_id {
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
            let rollout_path = match representation {
                LineageRepresentation::Existing => rollout_path,
                LineageRepresentation::PlainForReference
                    if next_rollout_id.is_none() && meta.meta.history_base.is_none() =>
                {
                    // A newly shared standalone source must remain readable by older binaries.
                    codex_rollout::materialize_rollout_for_reference(rollout_path.as_path())
                        .await
                        .map_err(|err| ThreadStoreError::Internal {
                            message: format!(
                                "failed to materialize referenced rollout {}: {err}",
                                rollout_path.display()
                            ),
                        })?
                }
                // Already-shared compressed history requires a compatible reader regardless of
                // new forks. Read it without publishing decoded copies into ancestors' folders;
                // their owners may concurrently archive or unarchive those immutable files.
                LineageRepresentation::PlainForReference => rollout_path,
            };
            if let Some(end) = end {
                validate_cutoff_bounds(requested_thread_id, rollout_path.as_path(), &end).await?;
            }
            let start_ordinal = match meta.meta.history_base {
                Some(base) => base.end_ordinal_exclusive.checked_add(1).ok_or_else(|| {
                    malformed_lineage(requested_thread_id, "source ordinal overflow")
                })?,
                None => 1,
            };
            segments.push(RolloutLineageSegment {
                rollout_id,
                rollout_path,
                start_ordinal,
                end,
            });

            let Some(base) = meta.meta.history_base else {
                break;
            };
            next_rollout_id = Some(base.thread_id);
            end = Some(base);
        }

        segments.reverse();
        Ok(RolloutLineage { segments })
    }
}

async fn resolve_rollout_path_by_id(
    store: &LocalThreadStore,
    rollout_id: ThreadId,
) -> ThreadStoreResult<Option<PathBuf>> {
    codex_rollout::find_rollout_path_by_rollout_id(store.config.codex_home.as_path(), rollout_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to locate rollout {rollout_id}: {err}"),
        })
}

#[derive(Clone, Copy)]
enum LineageRepresentation {
    Existing,
    PlainForReference,
}

impl RolloutLineage {
    pub(super) fn segments(&self) -> &[RolloutLineageSegment] {
        self.segments.as_slice()
    }

    pub(super) fn segment_index_for_ordinal(&self, ordinal: u64) -> Option<usize> {
        self.segments.iter().position(|segment| {
            ordinal >= segment.start_ordinal()
                && segment
                    .end_ordinal()
                    .is_none_or(|end_ordinal| ordinal < end_ordinal)
        })
    }

    pub(super) async fn truncate_at(
        mut self,
        end: HistoryPosition,
    ) -> ThreadStoreResult<RolloutLineage> {
        let segment_index = self
            .segments
            .iter()
            .position(|segment| segment.rollout_id == end.thread_id)
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "fork position is outside the source lineage".to_string(),
            })?;
        self.segments.truncate(segment_index + 1);
        let segment = self
            .segments
            .last_mut()
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "rollout lineage has no segments".to_string(),
            })?;
        validate_cutoff_bounds(end.thread_id, segment.rollout_path.as_path(), &end).await?;
        segment.end = Some(end);
        Ok(self)
    }
}

impl RolloutLineageSegment {
    pub(super) fn rollout_id(&self) -> ThreadId {
        self.rollout_id
    }

    pub(super) fn start_ordinal(&self) -> u64 {
        self.start_ordinal
    }

    pub(super) fn end_ordinal(&self) -> Option<u64> {
        self.end.map(|end| end.end_ordinal_exclusive)
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
    let path = rollout_path.to_path_buf();
    let end_byte_offset = end.end_byte_offset;
    let contains_prefix = tokio::task::spawn_blocking(move || {
        codex_rollout::rollout_contains_prefix(&path, end_byte_offset)
    })
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to join rollout prefix validation: {err}"),
    })?
    .map_err(|err| ThreadStoreError::Internal {
        message: format!(
            "failed to read lineage metadata {}: {err}",
            rollout_path.display()
        ),
    })?;
    if !contains_prefix {
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
