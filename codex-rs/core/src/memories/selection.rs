use chrono::Duration;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_state::ThreadMetadata;

use super::types::RolloutCandidate;

/// Selects rollout candidates that need stage-1 memory extraction.
///
/// A rollout is selected when it is not the active thread and was updated
/// within the configured max age window.
pub(crate) fn select_rollout_candidates_from_db(
    items: &[ThreadMetadata],
    current_thread_id: ThreadId,
    max_items: usize,
    max_age_days: i64,
) -> Vec<RolloutCandidate> {
    if max_items == 0 {
        return Vec::new();
    }

    let cutoff = Utc::now() - Duration::days(max_age_days.max(0));

    let mut candidates = Vec::new();

    for item in items {
        if item.id == current_thread_id {
            continue;
        }
        if item.updated_at < cutoff {
            continue;
        }

        candidates.push(RolloutCandidate {
            thread_id: item.id,
            rollout_path: item.rollout_path.clone(),
            cwd: item.cwd.clone(),
            updated_at: Some(item.updated_at.to_rfc3339()),
        });

        if candidates.len() >= max_items {
            break;
        }
    }

    candidates
}
