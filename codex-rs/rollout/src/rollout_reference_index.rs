//! Indexes direct fork references and attached children found in local rollout files.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::RolloutId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;

use crate::ARCHIVED_SESSIONS_SUBDIR;
use crate::SESSIONS_SUBDIR;
use crate::compression::RolloutFile;
use crate::rollout_file_name::RolloutFileName;

/// Fork relationships discovered from local rollout metadata.
///
/// Rollouts are indexed by immutable rollout IDs, not a thread's selected lineage. Attached
/// children refer to their parent's stable thread ID.
#[derive(Debug, Default)]
pub struct RolloutReferenceIndex {
    rollouts_by_id: HashMap<RolloutId, IndexedRollout>,
    reference_counts_by_rollout: HashMap<RolloutId, usize>,
}

#[derive(Debug)]
struct IndexedRollout {
    thread_id: ThreadId,
    path: PathBuf,
    history_base: Option<HistoryPosition>,
    attached_to_thread_id: Option<ThreadId>,
}

impl RolloutReferenceIndex {
    /// Scans active and archived local rollout metadata.
    pub async fn scan(codex_home: &Path) -> io::Result<Self> {
        Self::scan_paths(
            vec![
                codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
                codex_home.join(SESSIONS_SUBDIR),
            ],
            /*thread_ids*/ None,
        )
        .await
    }

    /// Scans only unarchived rollouts to locate files that still need to be archived.
    ///
    /// Reference counts exclude archived history and must not be used to decide whether a
    /// rollout can be deleted or compressed.
    pub async fn scan_unarchived(codex_home: &Path) -> io::Result<Self> {
        Self::scan_paths(
            vec![codex_home.join(SESSIONS_SUBDIR)],
            /*thread_ids*/ None,
        )
        .await
    }

    /// Scans unarchived files whose canonical filenames belong to the requested threads.
    ///
    /// Skips unrelated rollout contents, including compressed files. Metadata still determines
    /// ownership among the candidates. Reference counts are partial and must not be used to
    /// decide whether a rollout can be deleted or compressed.
    pub async fn scan_unarchived_threads(
        codex_home: &Path,
        thread_ids: &[ThreadId],
    ) -> io::Result<Self> {
        let thread_ids = thread_ids.iter().copied().collect();
        Self::scan_paths(vec![codex_home.join(SESSIONS_SUBDIR)], Some(&thread_ids)).await
    }

    /// Scans archived files whose canonical filenames belong to the requested threads.
    ///
    /// Metadata still determines ownership among the candidates. Reference counts are partial
    /// and must not be used to decide whether a rollout can be deleted or compressed.
    pub async fn scan_archived_threads(
        codex_home: &Path,
        thread_ids: &[ThreadId],
    ) -> io::Result<Self> {
        let thread_ids = thread_ids.iter().copied().collect();
        Self::scan_paths(
            vec![codex_home.join(ARCHIVED_SESSIONS_SUBDIR)],
            Some(&thread_ids),
        )
        .await
    }

    async fn scan_paths(
        mut stack: Vec<PathBuf>,
        thread_ids: Option<&HashSet<ThreadId>>,
    ) -> io::Result<Self> {
        let mut rollouts_by_id = HashMap::new();
        while let Some(directory) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(directory.as_path()).await {
                Ok(entries) => entries,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            loop {
                let Some(entry) = entries.next_entry().await? else {
                    break;
                };
                let path = entry.path();
                let file_type = entry.file_type().await?;
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                let Some(rollout_file) = RolloutFile::from_path(path) else {
                    continue;
                };
                let Some(file_name) = RolloutFileName::parse(rollout_file.plain_file_name()) else {
                    continue;
                };
                if thread_ids.is_some_and(|ids| !ids.contains(&file_name.thread_id())) {
                    continue;
                }
                let rollout_id = file_name.rollout_id();
                let Ok(meta) = crate::read_session_meta_line(rollout_file.path()).await else {
                    continue;
                };
                if let Entry::Vacant(entry) = rollouts_by_id.entry(rollout_id) {
                    entry.insert(IndexedRollout {
                        thread_id: meta.meta.id,
                        path: rollout_file.into_path(),
                        history_base: meta.meta.history_base,
                        attached_to_thread_id: meta
                            .meta
                            .forked_from_id
                            .filter(|_| meta.meta.attached_thread),
                    });
                }
            }
        }

        let mut reference_counts_by_rollout = HashMap::new();
        for (rollout_id, rollout) in &rollouts_by_id {
            let Some(history_base) = rollout.history_base else {
                continue;
            };
            if history_base.thread_id == *rollout_id {
                continue;
            }
            *reference_counts_by_rollout
                .entry(history_base.thread_id)
                .or_default() += 1;
        }
        Ok(Self {
            rollouts_by_id,
            reference_counts_by_rollout,
        })
    }

    /// Returns how many other discovered rollouts directly reference `rollout_id`.
    pub fn reference_count(&self, rollout_id: RolloutId) -> usize {
        self.reference_counts_by_rollout
            .get(&rollout_id)
            .copied()
            .unwrap_or_default()
    }

    /// Returns the direct history-base edge for `rollout_id`, if one was discovered.
    pub fn history_base(&self, rollout_id: RolloutId) -> Option<&HistoryPosition> {
        self.rollouts_by_id
            .get(&rollout_id)
            .and_then(|rollout| rollout.history_base.as_ref())
    }

    /// Find an indexed attached child of any candidate parent.
    pub fn attached_child(&self, parent_thread_ids: &[ThreadId]) -> Option<ThreadId> {
        let parents: HashSet<_> = parent_thread_ids.iter().copied().collect();
        self.rollouts_by_id.values().find_map(|rollout| {
            rollout
                .attached_to_thread_id
                .filter(|parent| parents.contains(parent))
                .map(|_| rollout.thread_id)
        })
    }

    /// Returns rollout IDs and paths whose session metadata belongs to `thread_id`.
    pub fn rollouts_for_thread(
        &self,
        thread_id: ThreadId,
    ) -> impl Iterator<Item = (RolloutId, &Path)> {
        self.rollouts_by_id
            .iter()
            .filter(move |(_, rollout)| rollout.thread_id == thread_id)
            .map(|(rollout_id, rollout)| (*rollout_id, rollout.path.as_path()))
    }
}

#[cfg(test)]
#[path = "rollout_reference_index_tests.rs"]
mod tests;
