//! Walks immutable rollout ranges from the selected leaf toward its ancestors on demand.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;

use super::AncestorReads;
use super::FailedAncestor;
use super::LineageResolutionMode;
use super::LocalThreadStore;
use super::RolloutLineage;
use super::RolloutLineageSegment;
use super::malformed_lineage;
use super::resolve_ancestor_rollout;
use super::thread_rollout_resolver;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// The next file to open. A parent bookmark bounds the rows inherited from that file.
enum NextRollout {
    Selected(Option<PathBuf>),
    Parent(HistoryPosition),
}

pub(in crate::local) struct LineageWalk<'a> {
    store: &'a LocalThreadStore,
    requested_thread_id: ThreadId,
    mode: LineageResolutionMode,
    next: Option<NextRollout>,
    seen: HashSet<ThreadId>,
    prefetched_paths: HashMap<ThreadId, PathBuf>,
    excluded: Option<&'a HashMap<ThreadId, FailedAncestor>>,
    ancestor_paths: HashMap<PathBuf, (HistoryPosition, ThreadId)>,
    // Newest first, so discovering one older segment is a push. Only finish reverses this.
    segments: Vec<RolloutLineageSegment>,
}

impl<'a> LineageWalk<'a> {
    pub(super) fn new(
        store: &'a LocalThreadStore,
        requested_thread_id: ThreadId,
        mode: LineageResolutionMode,
        initial_path: Option<PathBuf>,
        excluded: Option<&'a HashMap<ThreadId, FailedAncestor>>,
    ) -> Self {
        Self {
            store,
            requested_thread_id,
            mode,
            next: Some(NextRollout::Selected(initial_path)),
            seen: HashSet::new(),
            prefetched_paths: HashMap::new(),
            excluded,
            ancestor_paths: HashMap::new(),
            segments: Vec::new(),
        }
    }

    pub(in crate::local) fn for_history_page(
        store: &'a LocalThreadStore,
        thread_id: ThreadId,
    ) -> Self {
        Self::new(
            store,
            thread_id,
            LineageResolutionMode::HistoryPage,
            /*initial_path*/ None,
            /*excluded*/ None,
        )
    }

    /// Opens all remaining ancestors, oldest first in the resulting lineage.
    pub(in crate::local) async fn finish(mut self) -> ThreadStoreResult<RolloutLineage> {
        self.resolve_all().await?;
        self.segments.reverse();
        Ok(RolloutLineage {
            segments: self.segments,
            reads: AncestorReads::new(self.ancestor_paths),
            requested_thread_id: self.requested_thread_id,
            mode: self.mode,
        })
    }

    pub(in crate::local) async fn resolve_all(
        &mut self,
    ) -> ThreadStoreResult<&[RolloutLineageSegment]> {
        while self.next_older().await? {}
        Ok(&self.segments)
    }

    /// Returns the segment at the given depth from the leaf and all newer segments.
    pub(in crate::local) async fn at_depth(
        &mut self,
        depth: usize,
    ) -> ThreadStoreResult<Option<(&RolloutLineageSegment, &[RolloutLineageSegment])>> {
        while self.segments.len() <= depth {
            if !self.next_older().await? {
                return Ok(None);
            }
        }
        Ok(self.segments[..=depth].split_last())
    }

    async fn next_older(&mut self) -> ThreadStoreResult<bool> {
        let Some(next) = self.next.take() else {
            return Ok(false);
        };
        let requested_thread_id = self.requested_thread_id;
        let store = self.store;
        let _writer_guard = match self.mode {
            LineageResolutionMode::Existing | LineageResolutionMode::HistoryPage => None,
            LineageResolutionMode::PlainForReference => {
                let id = match &next {
                    NextRollout::Selected(_) => requested_thread_id,
                    NextRollout::Parent(end) => end.thread_id,
                };
                Some(store.live_writer_locks.lock(id).await)
            }
        };
        let (rollout_id, rollout_path, bounds, end) = match next {
            NextRollout::Selected(initial_path) => {
                let (rollout_id, path) = match initial_path {
                    Some(path) => {
                        let id = thread_rollout_resolver::rollout_id_from_path_or_legacy_thread_id(
                            &path,
                            requested_thread_id,
                            ThreadHistoryMode::Paginated,
                        )?;
                        (id, path)
                    }
                    None => {
                        let resolved = thread_rollout_resolver::resolve_current_including_archived(
                            store,
                            requested_thread_id,
                        )
                        .await?
                        .ok_or_else(|| {
                            malformed_lineage(requested_thread_id, "missing source rollout")
                        })?;
                        (resolved.rollout_id, resolved.path)
                    }
                };
                let (mut path, mut bounds) = self
                    .mode
                    .read_rollout(store, path, requested_thread_id, Some(requested_thread_id))
                    .await?;
                match self.mode {
                    LineageResolutionMode::HistoryPage => {
                        bounds
                            .read_history_page_bounds(&path, requested_thread_id)
                            .await?;
                        if store.config.sqlite.prefers_rollback_journal().await {
                            super::super::thread_history_materialization::materialize_to_sqlite(
                                store, rollout_id, &path,
                            )
                            .await?;
                        }
                    }
                    LineageResolutionMode::PlainForReference if bounds.parent.is_none() => {
                        // A newly shared standalone source must remain readable by older binaries.
                        path = codex_rollout::materialize_rollout_for_reference(&path)
                            .await
                            .map_err(|err| ThreadStoreError::Internal {
                                message: format!(
                                    "failed to materialize referenced rollout {}: {err}",
                                    path.display()
                                ),
                            })?;
                    }
                    LineageResolutionMode::Existing | LineageResolutionMode::PlainForReference => {}
                }
                (rollout_id, path, bounds, None)
            }
            NextRollout::Parent(end) => {
                // Defer the directory scan too: pages contained in the leaf never need it.
                if matches!(self.mode, LineageResolutionMode::HistoryPage)
                    && let [child] = self.segments.as_slice()
                {
                    self.prefetched_paths = codex_rollout::prefetch_sibling_rollout_paths(
                        &store.config.codex_home,
                        &child.rollout_path,
                        end.thread_id,
                    )
                    .await
                    .unwrap_or_default();
                }
                let (path, bounds) = resolve_ancestor_rollout(
                    store,
                    requested_thread_id,
                    end,
                    self.mode,
                    self.prefetched_paths.remove(&end.thread_id),
                    self.excluded
                        .and_then(|excluded| excluded.get(&end.thread_id)),
                )
                .await?;
                if !matches!(self.mode, LineageResolutionMode::HistoryPage) {
                    self.ancestor_paths
                        .insert(path.clone(), (end, bounds.owner));
                }
                (end.thread_id, path, bounds, Some(end))
            }
        };
        if !self.seen.insert(rollout_id) {
            return Err(malformed_lineage(requested_thread_id, "cycle detected"));
        }
        self.next = bounds.parent.map(NextRollout::Parent);
        self.segments.push(RolloutLineageSegment {
            rollout_id,
            rollout_path,
            start_ordinal: bounds.start_ordinal,
            end,
            hides_parent: bounds.hides_parent,
        });
        Ok(true)
    }
}
