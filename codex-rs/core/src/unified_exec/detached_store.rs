use std::collections::HashMap;

use codex_protocol::ThreadId;
use tokio::sync::Mutex;

use crate::unified_exec::ProcessEntry;
use crate::unified_exec::UnifiedExecProcessManager;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DetachedProcessSummary {
    pub process_count: usize,
    pub process_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReattachSummary {
    pub reattached_count: usize,
    pub skipped_count: usize,
    pub skipped_process_ids: Vec<String>,
}

#[derive(Default)]
pub(crate) struct DetachedUnifiedExecStore {
    /// Detached unified-exec processes keyed by `(thread_id, process_id)`.
    entries: Mutex<HashMap<(ThreadId, String), ProcessEntry>>,
}

impl DetachedUnifiedExecStore {
    /// Export processes from an attached manager and store them under `thread_id`.
    ///
    /// This method does not hold the detached-store lock while awaiting manager
    /// export so process-manager and thread-manager lock domains stay separated.
    pub(crate) async fn detach_from_manager(
        &self,
        thread_id: ThreadId,
        manager: &UnifiedExecProcessManager,
    ) -> DetachedProcessSummary {
        let exported = manager.export_processes().await;
        let mut process_ids = exported
            .iter()
            .map(|entry| entry.process_id.clone())
            .collect::<Vec<_>>();
        process_ids.sort();

        let mut replaced_entries = Vec::new();
        {
            let mut entries = self.entries.lock().await;
            for entry in exported {
                let process_id = entry.process_id.clone();
                let key = (thread_id, process_id);
                if let Some(existing) = entries.insert(key, entry) {
                    // Keep exactly one detached process entry per
                    // (thread_id, process_id). Replacements are stale detached
                    // handles and must be terminated to avoid leaked children.
                    replaced_entries.push(existing);
                }
            }
        }
        for mut replaced in replaced_entries {
            Self::abort_all_watcher_tasks(&mut replaced);
            replaced.process.terminate();
        }

        DetachedProcessSummary {
            process_count: process_ids.len(),
            process_ids,
        }
    }

    /// Move detached processes for `thread_id` back into an attached manager.
    pub(crate) async fn reattach_to_manager(
        &self,
        thread_id: ThreadId,
        manager: &UnifiedExecProcessManager,
    ) -> ReattachSummary {
        let detached_entries = {
            let mut entries = self.entries.lock().await;
            let mut keys = entries
                .keys()
                .filter(|(candidate_thread_id, _)| *candidate_thread_id == thread_id)
                .cloned()
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.1.cmp(&right.1));

            keys.into_iter()
                .filter_map(|key| entries.remove(&key))
                .collect::<Vec<_>>()
        };

        if detached_entries.is_empty() {
            return ReattachSummary::default();
        }

        let mut skipped_entries = Vec::new();
        let summary = manager
            .import_processes(detached_entries, &mut skipped_entries)
            .await;
        if !skipped_entries.is_empty() {
            let mut entries = self.entries.lock().await;
            for entry in skipped_entries {
                let key = (thread_id, entry.process_id.clone());
                let _ = entries.insert(key, entry);
            }
        }

        summary
    }

    /// Terminate and remove all detached processes associated with `thread_id`.
    pub(crate) async fn clean_thread(&self, thread_id: ThreadId) -> DetachedProcessSummary {
        let entries = {
            let mut entries = self.entries.lock().await;
            let mut keys = entries
                .keys()
                .filter(|(candidate_thread_id, _)| *candidate_thread_id == thread_id)
                .cloned()
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.1.cmp(&right.1));

            keys.into_iter()
                .filter_map(|key| entries.remove(&key))
                .collect::<Vec<_>>()
        };

        let mut process_ids = entries
            .iter()
            .map(|entry| entry.process_id.clone())
            .collect::<Vec<_>>();
        process_ids.sort();
        for mut entry in entries {
            Self::abort_all_watcher_tasks(&mut entry);
            entry.process.terminate();
        }

        DetachedProcessSummary {
            process_count: process_ids.len(),
            process_ids,
        }
    }

    /// Terminate and remove every detached process across all threads.
    pub(crate) async fn clean_all(&self) {
        let entries = {
            let mut entries = self.entries.lock().await;
            entries.drain().map(|(_, entry)| entry).collect::<Vec<_>>()
        };
        for mut entry in entries {
            Self::abort_all_watcher_tasks(&mut entry);
            entry.process.terminate();
        }
    }

    fn abort_all_watcher_tasks(entry: &mut ProcessEntry) {
        if let Some(task) = entry.stream_task.take() {
            task.abort();
        }
        if let Some(task) = entry.exit_task.take() {
            task.abort();
        }
        if let Some(task) = entry.network_task.take() {
            task.abort();
        }
    }
}
