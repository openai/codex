//! Bounded failed-review evidence, retained independently of short-lived reviewers.
//! Insertion and eviction share one budget; deleting the reviewed thread cascades to its evidence.

use super::StateRuntime;
use codex_protocol::ThreadId;
use sqlx::Row;
use uuid::Uuid;

pub const MAX_GUARDIAN_REVIEW_RECORDS: usize = 64;
pub const MAX_GUARDIAN_REVIEW_RECORDS_PER_THREAD: usize = 8;
pub const MAX_GUARDIAN_REVIEW_BYTES: usize = 8 * 1024 * 1024;

/// One complete serialized review, ordered by its capture-time UUIDv7.
#[derive(Clone)]
pub struct GuardianReviewRecord {
    pub id: String,
    pub thread_id: ThreadId,
    pub record: Vec<u8>,
}

impl GuardianReviewRecord {
    pub fn new(thread_id: ThreadId, record: Vec<u8>) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            thread_id,
            record,
        }
    }
}

impl StateRuntime {
    /// Retain opt-in evidence for a persisted actor; the reviewer need not remain alive.
    pub async fn record_guardian_review_failure(
        &self,
        record: &GuardianReviewRecord,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            record.record.len() < MAX_GUARDIAN_REVIEW_BYTES,
            "Guardian feedback record exceeds its size limit"
        );
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "INSERT INTO guardian_review_feedback (id, thread_id, record) VALUES (?, ?, ?)",
        )
        .bind(&record.id)
        .bind(record.thread_id.to_string())
        .bind(&record.record)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM guardian_review_feedback WHERE id IN \
             (SELECT id FROM guardian_review_feedback WHERE thread_id = ? \
              ORDER BY id DESC LIMIT -1 OFFSET ?)",
        )
        .bind(record.thread_id.to_string())
        .bind(MAX_GUARDIAN_REVIEW_RECORDS_PER_THREAD as i64)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM guardian_review_feedback WHERE id IN \
             (SELECT id FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY id DESC) AS n, \
              SUM(length(record) + 1) OVER (ORDER BY id DESC) AS bytes \
              FROM guardian_review_feedback) WHERE n > ? OR bytes > ?)",
        )
        .bind(MAX_GUARDIAN_REVIEW_RECORDS as i64)
        .bind(MAX_GUARDIAN_REVIEW_BYTES as i64)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Read the bounded retained history, oldest first. Callers must scope exports to a task.
    pub async fn list_guardian_review_records(&self) -> anyhow::Result<Vec<GuardianReviewRecord>> {
        sqlx::query("SELECT id, thread_id, record FROM guardian_review_feedback ORDER BY id")
            .fetch_all(self.pool.as_ref())
            .await?
            .into_iter()
            .map(|row| {
                Ok(GuardianReviewRecord {
                    id: row.try_get("id")?,
                    thread_id: ThreadId::from_string(row.try_get("thread_id")?)?,
                    record: row.try_get("record")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "guardian_feedback_tests.rs"]
mod tests;
