//! Bounded durable failed-review evidence, with an in-memory fallback for storage failures.
//! Captures are exported only by a feedback request that includes logs.

use crate::FeedbackAttachment;
use codex_protocol::ThreadId;
pub use codex_state::GuardianReviewRecord;
use codex_state::MAX_GUARDIAN_REVIEW_BYTES as MAX_BYTES;
use codex_state::MAX_GUARDIAN_REVIEW_RECORDS as MAX_RECORDS;
use codex_state::MAX_GUARDIAN_REVIEW_RECORDS_PER_THREAD as MAX_RECORDS_PER_THREAD;
use codex_state::StateRuntime;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

const PERSIST_TIMEOUT: Duration = Duration::from_millis(250);

static RECORDS: Mutex<ReviewRecords> = Mutex::new(ReviewRecords {
    records: VecDeque::new(),
    bytes: 0,
    discarded_records: 0,
});

/// A consistent snapshot of retained failures and their owning threads.
pub struct GuardianReviewFailures {
    pub attachment: Option<FeedbackAttachment>,
    /// Unique threads, ordered by most recently retained failure first.
    pub thread_ids: Vec<ThreadId>,
    /// Process-wide evictions or oversized records, not a count for this task tree.
    pub process_discarded_records: usize,
}

#[derive(Clone, Default)]
struct ReviewRecords {
    records: VecDeque<GuardianReviewRecord>,
    bytes: usize,
    discarded_records: usize,
}

impl ReviewRecords {
    fn push(&mut self, record: GuardianReviewRecord) {
        if record.record.len() + 1 > MAX_BYTES {
            self.discarded_records = self.discarded_records.saturating_add(1);
            return;
        }
        if self
            .records
            .iter()
            .filter(|entry| entry.thread_id == record.thread_id)
            .count()
            >= MAX_RECORDS_PER_THREAD
            && let Some(index) = self
                .records
                .iter()
                .position(|entry| entry.thread_id == record.thread_id)
            && let Some(removed) = self.records.remove(index)
        {
            self.bytes -= removed.record.len() + 1;
            self.discarded_records = self.discarded_records.saturating_add(1);
        }
        while self.records.len() >= MAX_RECORDS || self.bytes + record.record.len() + 1 > MAX_BYTES
        {
            if let Some(removed) = self.records.pop_front() {
                self.bytes -= removed.record.len() + 1;
                self.discarded_records = self.discarded_records.saturating_add(1);
            }
        }
        self.bytes += record.record.len() + 1;
        self.records.push_back(record);
    }

    fn snapshot(&self, thread_ids: &[ThreadId]) -> GuardianReviewFailures {
        let thread_ids = thread_ids.iter().copied().collect::<HashSet<_>>();
        let mut buffer = Vec::new();
        for record in &self.records {
            if thread_ids.contains(&record.thread_id) {
                buffer.extend_from_slice(&record.record);
                buffer.push(b'\n');
            }
        }
        let attachment = (!buffer.is_empty()).then_some(FeedbackAttachment {
            filename: "auto-review-failures.jsonl".to_string(),
            buffer,
            content_type: Some("application/x-ndjson".to_string()),
        });
        let mut seen = HashSet::new();
        let thread_ids = self
            .records
            .iter()
            .rev()
            .filter(|record| {
                thread_ids.contains(&record.thread_id) && seen.insert(record.thread_id)
            })
            .map(|record| record.thread_id)
            .collect();
        GuardianReviewFailures {
            attachment,
            thread_ids,
            process_discarded_records: self.discarded_records,
        }
    }
}

/// Retain a failed review in memory and persist it before its session is cleaned up.
pub async fn record_guardian_review_failure(
    state_db: Option<&StateRuntime>,
    record: GuardianReviewRecord,
) {
    RECORDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(record.clone());
    if let Some(state_db) = state_db
        && !matches!(
            tokio::time::timeout(
                PERSIST_TIMEOUT,
                state_db.record_guardian_review_failure(&record)
            )
            .await,
            Ok(Ok(()))
        )
    {
        tracing::warn!("failed to persist Guardian feedback; retaining an in-memory fallback");
    }
}

/// Recover retained failures before selecting rollout attachments, scoped to the reported task tree.
pub async fn guardian_review_failures(
    state_db: Option<&StateRuntime>,
    thread_ids: &[ThreadId],
) -> GuardianReviewFailures {
    let fallback = RECORDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let Some(state_db) = state_db else {
        return fallback.snapshot(thread_ids);
    };
    let Ok(mut records) = state_db.list_guardian_review_records().await else {
        tracing::warn!("failed to load persisted Guardian feedback");
        return fallback.snapshot(thread_ids);
    };
    records.extend(fallback.records);
    records.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    records.dedup_by(|left, right| left.id == right.id);
    let mut retained = ReviewRecords::default();
    for record in records {
        retained.push(record);
    }
    // This legacy diagnostic describes only the in-memory fallback, not durable evictions.
    retained.discarded_records = fallback.discarded_records;
    retained.snapshot(thread_ids)
}

#[cfg(test)]
#[path = "guardian_tests.rs"]
mod tests;
