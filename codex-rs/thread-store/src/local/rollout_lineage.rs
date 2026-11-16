use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;

use codex_rollout::RolloutItem;

mod ancestor_reads;
mod walk;

pub(super) use ancestor_reads::AncestorReads;
use ancestor_reads::FailedAncestor;
pub(super) use walk::LineageWalk;

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
    hides_parent: bool,
}

/// The resolved ranges of a forked history, ordered from oldest to newest.
#[derive(Clone, Debug)]
pub(super) struct RolloutLineage {
    pub(super) segments: Vec<RolloutLineageSegment>,
    pub(super) reads: AncestorReads,
    requested_thread_id: ThreadId,
    mode: LineageResolutionMode,
}

impl LocalThreadStore {
    pub(super) async fn resolve_rollout_lineage(
        &self,
        requested_thread_id: ThreadId,
        initial_path: Option<PathBuf>,
    ) -> ThreadStoreResult<RolloutLineage> {
        LineageWalk::new(
            self,
            requested_thread_id,
            LineageResolutionMode::Existing,
            initial_path,
            /*excluded*/ None,
        )
        .finish()
        .await
    }

    pub(super) async fn resolve_rollout_lineage_for_history_page(
        &self,
        requested_thread_id: ThreadId,
    ) -> ThreadStoreResult<RolloutLineage> {
        LineageWalk::for_history_page(self, requested_thread_id)
            .finish()
            .await
    }

    pub(super) async fn resolve_rollout_lineage_for_reference(
        &self,
        requested_thread_id: ThreadId,
    ) -> ThreadStoreResult<RolloutLineage> {
        LineageWalk::new(
            self,
            requested_thread_id,
            LineageResolutionMode::PlainForReference,
            /*initial_path*/ None,
            /*excluded*/ None,
        )
        .finish()
        .await
    }
}

/// A mid-turn fork starts with its header, settings, optional model guidance, and parent abort.
/// Stop before the first child event; a real child interruption follows its own turn start.
async fn attached_history_start(path: &Path, start: u64) -> std::io::Result<u64> {
    let mut reader = codex_rollout::open_rollout_line_reader(path).await?;
    for _ in 0..4 {
        let Some(line) = reader.next_line().await? else {
            break;
        };
        let Ok(line) = codex_rollout::parse_rollout_line(&line) else {
            break;
        };
        match line.item {
            RolloutItem::SessionMeta(_)
            | RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(_))
            | RolloutItem::ResponseItem(_) => {}
            RolloutItem::EventMsg(EventMsg::TurnAborted(_)) => {
                return Ok(line.ordinal.map_or(start, |n| n.saturating_add(1)));
            }
            _ => break,
        }
    }
    Ok(start)
}

// Resume can retain either a logical or canonical path. Validate both against the same
// canonical managed roots so a nested symlink cannot escape this home.
async fn managed_rollout_path(store: &LocalThreadStore, rollout_path: &Path) -> Option<PathBuf> {
    let path = tokio::fs::canonicalize(rollout_path).await.ok()?;
    for subdir in [
        codex_rollout::SESSIONS_SUBDIR,
        codex_rollout::ARCHIVED_SESSIONS_SUBDIR,
    ] {
        if tokio::fs::canonicalize(store.config.codex_home.join(subdir))
            .await
            .is_ok_and(|root| path.starts_with(root))
        {
            return Some(path);
        }
    }
    None
}

// The filename scan skips file symlinks. Following one here could change which plain
// sibling the reader prefers after canonicalizing a compressed rollout.
async fn existing_regular_rollout_path(path: &Path) -> Option<PathBuf> {
    let path = codex_rollout::existing_rollout_path(path).await?;
    let metadata = tokio::fs::symlink_metadata(&path).await.ok()?;
    metadata.is_file().then_some(path)
}

/// Resolve the representation the reader will open and check its immutable ID inside managed storage.
async fn managed_ancestor_path(
    store: &LocalThreadStore,
    path: &Path,
    rollout_id: ThreadId,
) -> Option<PathBuf> {
    if codex_rollout::rollout_id_from_path(path) != Some(rollout_id) {
        return None;
    }
    let path = existing_regular_rollout_path(path).await?;
    let path = managed_rollout_path(store, &path).await?;
    (codex_rollout::rollout_id_from_path(&path) == Some(rollout_id)).then_some(path)
}

struct RolloutBounds {
    owner: ThreadId,
    start_ordinal: u64,
    parent: Option<HistoryPosition>,
    hides_parent: bool,
}

async fn resolve_ancestor_rollout(
    store: &LocalThreadStore,
    requested_thread_id: ThreadId,
    end: HistoryPosition,
    mode: LineageResolutionMode,
    prefetched_path: Option<PathBuf>,
    excluded: Option<&FailedAncestor>,
) -> ThreadStoreResult<(PathBuf, RolloutBounds)> {
    if end.end_ordinal_exclusive == 0 {
        return Err(malformed_lineage(
            requested_thread_id,
            "cutoff cannot include source session metadata",
        ));
    }
    let (path, mut bounds) = resolve_ancestor_candidate(
        store,
        requested_thread_id,
        end.thread_id,
        mode,
        prefetched_path,
        excluded,
    )
    .await?;
    if matches!(mode, LineageResolutionMode::HistoryPage)
        && let Err(error) = bounds
            .read_history_page_bounds(&path, requested_thread_id)
            .await
    {
        let failed = FailedAncestor {
            path: tokio::fs::canonicalize(&path).await.unwrap_or(path),
            owner: bounds.owner,
            error,
        };
        let (path, mut bounds) = resolve_ancestor_candidate(
            store,
            requested_thread_id,
            end.thread_id,
            mode,
            /*prefetched_path*/ None,
            Some(&failed),
        )
        .await
        .map_err(|_| failed.error.clone())?;
        bounds
            .read_history_page_bounds(&path, requested_thread_id)
            .await
            .map_err(|_| failed.error)?;
        return Ok((path, bounds));
    }
    Ok((path, bounds))
}

async fn resolve_ancestor_candidate(
    store: &LocalThreadStore,
    requested_thread_id: ThreadId,
    rollout_id: ThreadId,
    mode: LineageResolutionMode,
    prefetched_path: Option<PathBuf>,
    excluded: Option<&FailedAncestor>,
) -> ThreadStoreResult<(PathBuf, RolloutBounds)> {
    let mut rejected = None;
    let path = if excluded.is_none()
        && let Some(path) = prefetched_path
        && let Some(path) = existing_regular_rollout_path(&path).await
    {
        path
    } else {
        // SQLite records the current path by thread ID. An original rollout shares its owner's
        // ID, but the current path can also name a later rollout created by a revert.
        if excluded.is_none()
            && let Some(state_db) = store.state_db().await
            && let Ok(Some(path)) = state_db
                .find_rollout_path_by_id(rollout_id, /*archived_only*/ None)
                .await
            && let Some(path) = managed_ancestor_path(store, &path, rollout_id).await
        {
            match RolloutBounds::read(&path, requested_thread_id, Some(rollout_id)).await {
                Ok(bounds) => return Ok((path, bounds)),
                Err(error) => rejected = Some((path, error)),
            }
        }
        let excluded_path = excluded
            .map(|failed| failed.path.as_path())
            .or_else(|| rejected.as_ref().map(|(path, _)| path.as_path()));
        codex_rollout::find_rollout_path_by_rollout_id_excluding(
            &store.config.codex_home,
            rollout_id,
            excluded_path,
        )
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to locate rollout {rollout_id}: {err}"),
        })
        .and_then(|path| {
            path.ok_or_else(|| malformed_lineage(rollout_id, "missing source rollout"))
        })
        .map_err(|scan_error| rejected.take().map_or(scan_error, |(_, error)| error))?
    };
    if let Some(failed) = excluded {
        let path = managed_ancestor_path(store, &path, rollout_id)
            .await
            .ok_or_else(|| malformed_lineage(rollout_id, "invalid replacement rollout path"))?;
        // A revert can give a rollout a new ID without changing the thread that owns its history.
        let bounds = RolloutBounds::read(&path, requested_thread_id, Some(failed.owner)).await?;
        return Ok((path, bounds));
    }
    mode.read_rollout(
        store,
        path,
        requested_thread_id,
        rejected.is_some().then_some(rollout_id),
    )
    .await
    .map_err(|scan_error| rejected.map_or(scan_error, |(_, error)| error))
}

#[derive(Clone, Copy, Debug)]
enum LineageResolutionMode {
    Existing,
    HistoryPage,
    PlainForReference,
}

impl LineageResolutionMode {
    async fn read_rollout(
        self,
        store: &LocalThreadStore,
        path: PathBuf,
        requested_thread_id: ThreadId,
        expected_owner: Option<ThreadId>,
    ) -> ThreadStoreResult<(PathBuf, RolloutBounds)> {
        let path = match self {
            Self::Existing | Self::HistoryPage => path,
            Self::PlainForReference => {
                managed_rollout_path(store, &path).await.ok_or_else(|| {
                    ThreadStoreError::InvalidRequest {
                        message: format!(
                            "rollout path `{}` must be in Codex home directory",
                            path.display()
                        ),
                    }
                })?
            }
        };
        let bounds = RolloutBounds::read(&path, requested_thread_id, expected_owner).await?;
        Ok((path, bounds))
    }
}

impl RolloutBounds {
    async fn read(
        path: &Path,
        requested_thread_id: ThreadId,
        expected_owner: Option<ThreadId>,
    ) -> ThreadStoreResult<Self> {
        let meta = codex_rollout::read_session_meta_line(path)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to read lineage metadata {}: {err}", path.display()),
            })?;
        if expected_owner.is_some_and(|owner| owner != meta.meta.id) {
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
        let Some(base) = meta.meta.history_base else {
            return Ok(RolloutBounds {
                owner: meta.meta.id,
                start_ordinal: 1,
                parent: None,
                hides_parent: false,
            });
        };
        let start_ordinal = base
            .end_ordinal_exclusive
            .checked_add(1)
            .ok_or_else(|| malformed_lineage(requested_thread_id, "source ordinal overflow"))?;
        // Reverts can point to earlier records of the same child. Stop at the original parent bookmark.
        let hides_parent = meta.meta.attached_thread
            && codex_rollout::forked_from_ordinal_exclusive(&meta.meta, Some(path))
                .is_some_and(|end| base.end_ordinal_exclusive <= end);
        Ok(RolloutBounds {
            owner: meta.meta.id,
            start_ordinal,
            parent: Some(base),
            hides_parent,
        })
    }

    async fn read_history_page_bounds(
        &mut self,
        path: &Path,
        requested_thread_id: ThreadId,
    ) -> ThreadStoreResult<()> {
        if self.hides_parent {
            self.start_ordinal = attached_history_start(path, self.start_ordinal)
                .await
                .map_err(|err| malformed_lineage(requested_thread_id, &err.to_string()))?;
            self.parent = None;
            self.hides_parent = false;
        }
        Ok(())
    }
}

impl RolloutLineage {
    /// Use the projected paths for named turn lookup, hiding history inherited by an attached child.
    pub(super) async fn for_history_page(&self) -> ThreadStoreResult<Cow<'_, Self>> {
        let Some(first) = self
            .segments
            .iter()
            .rposition(|segment| segment.hides_parent)
        else {
            return Ok(Cow::Borrowed(self));
        };
        let mut segments = self.segments[first..].to_vec();
        let child = &mut segments[0];
        child.start_ordinal = attached_history_start(&child.rollout_path, child.start_ordinal)
            .await
            .map_err(|err| {
                self.reads.record_failure(&child.rollout_path);
                malformed_lineage(
                    self.requested_thread_id,
                    &format!(
                        "failed to read attached history {}: {err}",
                        child.rollout_path.display()
                    ),
                )
            })?;
        child.hides_parent = false;
        Ok(Cow::Owned(Self {
            segments,
            reads: self.reads.clone(),
            requested_thread_id: self.requested_thread_id,
            mode: LineageResolutionMode::HistoryPage,
        }))
    }

    pub(super) fn segments(&self) -> &[RolloutLineageSegment] {
        self.segments.as_slice()
    }

    pub(super) fn segment_index_for_ordinal(&self, ordinal: u64) -> Option<usize> {
        self.segments
            .iter()
            .position(|segment| segment.contains(ordinal))
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
        if end.end_ordinal_exclusive < segment.start_ordinal {
            return Err(malformed_lineage(
                end.thread_id,
                "fork boundary is outside inherited source history",
            ));
        }
        if self.reads.tracks(&segment.rollout_path)
            && let Some(inherited) = segment.end
        {
            if end.end_byte_offset > inherited.end_byte_offset
                || end.end_ordinal_exclusive > inherited.end_ordinal_exclusive
            {
                return Err(malformed_lineage(
                    end.thread_id,
                    "fork boundary exceeds inherited source history",
                ));
            }
        } else {
            validate_cutoff_byte_offset(end.thread_id, &segment.rollout_path, &end).await?;
        }
        segment.end = Some(end);
        Ok(self)
    }
}

impl RolloutLineageSegment {
    pub(super) fn contains(&self, ordinal: u64) -> bool {
        ordinal >= self.start_ordinal && self.end_ordinal().is_none_or(|end| ordinal < end)
    }

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

async fn validate_cutoff_byte_offset(
    requested_thread_id: ThreadId,
    rollout_path: &Path,
    end: &HistoryPosition,
) -> ThreadStoreResult<()> {
    let path = rollout_path.to_path_buf();
    let end_byte_offset = end.end_byte_offset;
    let (stop, keep_running) = tokio::sync::oneshot::channel::<()>();
    let contains_prefix = tokio::task::spawn_blocking(move || {
        codex_rollout::rollout_contains_prefix(&path, end_byte_offset, || stop.is_closed())
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
    drop(keep_running);
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
