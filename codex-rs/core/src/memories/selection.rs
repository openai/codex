use codex_protocol::ThreadId;
use codex_state::ThreadMemory;
use codex_state::ThreadMetadata;
use std::collections::BTreeMap;

use super::types::RolloutCandidate;

/// Selects rollout candidates that need stage-1 memory extraction.
///
/// A rollout is selected when it is not the active thread and has no memory yet
/// (or the stored memory is older than the thread metadata timestamp).
pub(crate) fn select_rollout_candidates_from_db(
    items: &[ThreadMetadata],
    current_thread_id: ThreadId,
    existing_memories: &[ThreadMemory],
    max_items: usize,
) -> Vec<RolloutCandidate> {
    if max_items == 0 {
        return Vec::new();
    }

    let memory_updated_by_thread = existing_memories
        .iter()
        .map(|memory| (memory.thread_id.to_string(), memory.updated_at))
        .collect::<BTreeMap<_, _>>();

    let mut candidates = Vec::new();

    for item in items {
        if item.id == current_thread_id {
            continue;
        }

        let memory_updated_at = memory_updated_by_thread.get(&item.id.to_string());
        if memory_updated_at.is_some_and(|memory_updated_at| *memory_updated_at >= item.updated_at)
        {
            continue;
        }

        candidates.push(RolloutCandidate {
            thread_id: item.id,
            rollout_path: item.rollout_path.clone(),
            cwd: item.cwd.clone(),
            title: item.title.clone(),
            updated_at: Some(item.updated_at.to_rfc3339()),
        });

        if candidates.len() >= max_items {
            break;
        }
    }

    candidates
}
