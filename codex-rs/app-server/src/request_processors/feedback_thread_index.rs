//! Selects a bounded feedback subtree, preserving the reported thread and prioritizing
//! children with retained failed reviews. Filenames name archive entries or individual
//! attachments when archiving fails; the index describes selection, not delivery.

use codex_feedback::FeedbackAttachment;
use codex_feedback::GuardianReviewFailures;
use codex_protocol::ThreadId;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::HashSet;

const MAX_FEEDBACK_TREE_THREADS: usize = 8;
const MAX_LISTED_OMISSIONS: usize = 64;

#[derive(Serialize)]
pub(super) struct FeedbackThreadIndex {
    rollout_archive_filename: &'static str,
    pub threads: Vec<FeedbackThread>,
    retained_failure_thread_ids: Vec<ThreadId>,
    omitted_thread_ids: Vec<ThreadId>,
    unlisted_omitted_thread_count: usize,
    process_discarded_review_records: usize,
    notes: &'static str,
}

#[derive(Serialize)]
pub(super) struct FeedbackThread {
    pub thread_id: ThreadId,
    pub rollout_filename: Option<String>,
    pub guardian_rollout_filename: Option<String>,
}

impl FeedbackThreadIndex {
    pub fn new(
        reported_thread_id: ThreadId,
        mut subtree: Vec<ThreadId>,
        failures: &GuardianReviewFailures,
    ) -> Self {
        // UUIDv7 ordering provides a deterministic newest-child fallback.
        subtree.sort_unstable_by_key(|id| Reverse(id.to_string()));
        subtree.dedup();
        subtree.retain(|id| *id != reported_thread_id);
        let descendants = subtree.iter().copied().collect::<HashSet<_>>();
        let mut selected = HashSet::from([reported_thread_id]);
        let mut thread_ids = vec![reported_thread_id];
        for id in failures.thread_ids.iter().chain(&subtree) {
            if thread_ids.len() == MAX_FEEDBACK_TREE_THREADS {
                break;
            }
            if descendants.contains(id) && selected.insert(*id) {
                thread_ids.push(*id);
            }
        }
        let omitted_count = subtree.len().saturating_sub(thread_ids.len() - 1);
        let omitted_thread_ids = subtree
            .into_iter()
            .filter(|id| !selected.contains(id))
            .take(MAX_LISTED_OMISSIONS)
            .collect::<Vec<_>>();
        Self {
            rollout_archive_filename: "rollouts.tar.gz",
            threads: thread_ids
                .into_iter()
                .map(|thread_id| FeedbackThread {
                    thread_id,
                    rollout_filename: None,
                    guardian_rollout_filename: None,
                })
                .collect(),
            retained_failure_thread_ids: failures.thread_ids.clone(),
            unlisted_omitted_thread_count: omitted_count - omitted_thread_ids.len(),
            omitted_thread_ids,
            process_discarded_review_records: failures.process_discarded_records,
            notes: "Selection only, not an upload receipt. Null filenames mean no path was available. \
                    Omitted threads exceeded the rollout thread limit; their retained reviews are \
                    still in auto-review-failures.jsonl. Review evidence is persisted and bounded; \
                    missing evidence does not imply no denial. The discarded-record count covers \
                    only the process-wide in-memory fallback and resets on restart.",
        }
    }

    pub fn attachment(&self) -> serde_json::Result<FeedbackAttachment> {
        Ok(FeedbackAttachment {
            filename: "feedback-thread-index.json".to_string(),
            buffer: serde_json::to_vec(self)?,
            content_type: Some("application/json".to_string()),
        })
    }
}

#[cfg(test)]
#[path = "feedback_thread_index_tests.rs"]
mod tests;
