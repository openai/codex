//! Indexes direct fork references found in local rollout files.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::RolloutId;
use codex_protocol::protocol::HistoryPosition;

use crate::ARCHIVED_SESSIONS_SUBDIR;
use crate::SESSIONS_SUBDIR;
use crate::compression::RolloutFile;

/// Direct history-base edges discovered from local rollout metadata.
///
/// This indexes immutable rollout IDs, not a thread's selected lineage. Callers use it to answer
/// cheap inverse-reference questions without each reimplementing rollout discovery.
#[derive(Debug, Default)]
pub struct RolloutReferenceIndex {
    history_base_by_rollout: HashMap<RolloutId, HistoryPosition>,
    reference_counts_by_rollout: HashMap<RolloutId, usize>,
}

impl RolloutReferenceIndex {
    /// Scans active and archived local rollout metadata without a deadline.
    pub async fn scan(codex_home: &Path) -> io::Result<Self> {
        let Some(index) = Self::scan_with_deadline(codex_home, ScanDeadline::Unlimited).await?
        else {
            return Err(io::Error::other(
                "unlimited rollout reference scan exceeded a deadline",
            ));
        };
        Ok(index)
    }

    /// Scans active and archived local rollout metadata until the worker deadline expires.
    ///
    /// Returns None instead of a partial index when the deadline expires.
    pub(crate) async fn scan_until(
        codex_home: &Path,
        started_at: Instant,
        max_runtime: Duration,
    ) -> io::Result<Option<Self>> {
        Self::scan_with_deadline(
            codex_home,
            ScanDeadline::Until {
                started_at,
                max_runtime,
            },
        )
        .await
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
        self.history_base_by_rollout.get(&rollout_id)
    }

    async fn scan_with_deadline(
        codex_home: &Path,
        deadline: ScanDeadline,
    ) -> io::Result<Option<Self>> {
        let mut history_base_by_rollout = HashMap::new();
        let mut seen_rollout_ids = HashSet::new();
        let mut stack = vec![
            codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
            codex_home.join(SESSIONS_SUBDIR),
        ];
        while let Some(directory) = stack.pop() {
            if deadline.expired() {
                return Ok(None);
            }
            let mut entries = match tokio::fs::read_dir(directory.as_path()).await {
                Ok(entries) => entries,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            loop {
                if deadline.expired() {
                    return Ok(None);
                }
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
                let Some(rollout_id) = crate::rollout_id_from_path(rollout_file.path()) else {
                    continue;
                };
                let Ok(meta) = crate::read_session_meta_line(rollout_file.path()).await else {
                    continue;
                };
                if !seen_rollout_ids.insert(rollout_id) {
                    continue;
                }
                if let Some(history_base) = meta.meta.history_base {
                    history_base_by_rollout.insert(rollout_id, history_base);
                }
            }
        }

        let mut reference_counts_by_rollout = HashMap::new();
        for (rollout_id, history_base) in &history_base_by_rollout {
            if history_base.thread_id == *rollout_id {
                continue;
            }
            *reference_counts_by_rollout
                .entry(history_base.thread_id)
                .or_default() += 1;
        }
        Ok(Some(Self {
            history_base_by_rollout,
            reference_counts_by_rollout,
        }))
    }
}

#[derive(Clone, Copy)]
enum ScanDeadline {
    Unlimited,
    Until {
        started_at: Instant,
        max_runtime: Duration,
    },
}

impl ScanDeadline {
    fn expired(self) -> bool {
        match self {
            Self::Unlimited => false,
            Self::Until {
                started_at,
                max_runtime,
            } => started_at.elapsed() >= max_runtime,
        }
    }
}

#[cfg(test)]
#[path = "rollout_reference_index_tests.rs"]
mod tests;
