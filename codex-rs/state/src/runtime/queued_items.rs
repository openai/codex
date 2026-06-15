use super::*;
use uuid::Uuid;

#[derive(Clone)]
pub struct QueueStore {
    pool: Arc<SqlitePool>,
}

impl QueueStore {
    pub(crate) fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    pub(crate) async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn enqueue(
        &self,
        thread_id: ThreadId,
        payload_json: &[u8],
    ) -> anyhow::Result<crate::QueuedItemRecord> {
        let payload_json = std::str::from_utf8(payload_json)?;
        let queued_item_id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let row = sqlx::query(
            r#"
INSERT INTO queued_items (
    queued_item_id,
    thread_id,
    payload_jsonb,
    queue_order,
    state,
    failure_jsonb,
    created_at_ms,
    updated_at_ms
)
SELECT
    ?,
    ?,
    jsonb(?),
    COALESCE(MAX(queue_order), -1) + 1,
    'pending',
    NULL,
    ?,
    ?
FROM queued_items
WHERE thread_id = ?
RETURNING
    queued_item_id,
    thread_id,
    CAST(json(payload_jsonb) AS BLOB) AS payload_jsonb,
    queue_order,
    state,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(queued_item_id)
        .bind(thread_id.to_string())
        .bind(payload_json)
        .bind(now_ms)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .fetch_one(self.pool.as_ref())
        .await?;

        queued_item_from_row(&row)
    }

    pub async fn list_page(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::QueuedItemRecord>> {
        let rows = sqlx::query(
            r#"
SELECT
    queued_item_id,
    thread_id,
    CAST(json(payload_jsonb) AS BLOB) AS payload_jsonb,
    queue_order,
    state,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
FROM queued_items
WHERE thread_id = ?
  AND state IN ('pending', 'failed')
ORDER BY queue_order ASC
LIMIT ?
OFFSET ?
            "#,
        )
        .bind(thread_id.to_string())
        .bind(i64::try_from(limit)?)
        .bind(i64::try_from(offset)?)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(queued_item_from_row).collect()
    }

    pub async fn delete(&self, thread_id: ThreadId, queued_item_id: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
DELETE FROM queued_items
WHERE thread_id = ?
  AND queued_item_id = ?
  AND state IN ('pending', 'failed')
            "#,
        )
        .bind(thread_id.to_string())
        .bind(queued_item_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn reorder(
        &self,
        thread_id: ThreadId,
        ordered_ids: &[String],
    ) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        let visible_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
SELECT queued_item_id, queue_order
FROM queued_items
WHERE thread_id = ?
  AND state IN ('pending', 'failed')
ORDER BY queue_order ASC
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_all(transaction.as_mut())
        .await?;

        let visible_ids = visible_rows
            .iter()
            .map(|(queued_item_id, _)| queued_item_id.clone())
            .collect::<Vec<_>>();
        let visible_queue_orders = visible_rows
            .into_iter()
            .map(|(_, queue_order)| queue_order)
            .collect::<Vec<_>>();
        let mut expected_ids = visible_ids;
        expected_ids.sort();
        let mut requested_ids = ordered_ids.to_vec();
        requested_ids.sort();
        if expected_ids != requested_ids {
            transaction.rollback().await?;
            return Ok(false);
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        for (queue_order, queued_item_id) in visible_queue_orders.into_iter().zip(ordered_ids) {
            sqlx::query(
                r#"
UPDATE queued_items
SET queue_order = ?, updated_at_ms = ?
WHERE thread_id = ?
  AND queued_item_id = ?
  AND state IN ('pending', 'failed')
                "#,
            )
            .bind(queue_order)
            .bind(now_ms)
            .bind(thread_id.to_string())
            .bind(queued_item_id)
            .execute(transaction.as_mut())
            .await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// Atomically claims the pending FIFO head. A failed or already claimed
    /// head blocks later items.
    pub async fn claim_next(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<crate::QueuedItemClaim>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let claim_token = Uuid::new_v4().to_string();
        let row = sqlx::query(
            r#"
UPDATE queued_items
SET state = 'claimed', claim_token = ?, updated_at_ms = ?
WHERE queued_item_id = (
    SELECT queued_item_id
    FROM queued_items
    WHERE thread_id = ?
    ORDER BY queue_order ASC
    LIMIT 1
)
  AND state = 'pending'
RETURNING
    queued_item_id,
    thread_id,
    CAST(json(payload_jsonb) AS BLOB) AS payload_jsonb,
    queue_order,
    state,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&claim_token)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| {
            Ok(crate::QueuedItemClaim {
                item: queued_item_from_row(&row)?,
                claim_token,
            })
        })
        .transpose()
    }

    pub async fn has_claimed_item(&self, thread_id: ThreadId) -> anyhow::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM queued_items WHERE thread_id = ? AND state = 'claimed')",
        )
        .bind(thread_id.to_string())
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn release_claim(
        &self,
        queued_item_id: &str,
        claim_token: &str,
    ) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE queued_items
SET state = 'pending', claim_token = NULL, failure_jsonb = NULL, updated_at_ms = ?
WHERE queued_item_id = ? AND state = 'claimed' AND claim_token = ?
            "#,
        )
        .bind(now_ms)
        .bind(queued_item_id)
        .bind(claim_token)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn complete_claim(
        &self,
        queued_item_id: &str,
        claim_token: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM queued_items \
             WHERE queued_item_id = ? AND state = 'claimed' AND claim_token = ?",
        )
        .bind(queued_item_id)
        .bind(claim_token)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn fail_claim(
        &self,
        queued_item_id: &str,
        claim_token: &str,
        failure_json: &[u8],
    ) -> anyhow::Result<bool> {
        let failure_json = std::str::from_utf8(failure_json)?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE queued_items
SET state = 'failed', claim_token = NULL, failure_jsonb = jsonb(?), updated_at_ms = ?
WHERE queued_item_id = ? AND state = 'claimed' AND claim_token = ?
            "#,
        )
        .bind(failure_json)
        .bind(now_ms)
        .bind(queued_item_id)
        .bind(claim_token)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn recover_claims_as_failed_before(
        &self,
        thread_id: ThreadId,
        stale_before_ms: i64,
        failure_json: &[u8],
    ) -> anyhow::Result<u64> {
        let failure_json = std::str::from_utf8(failure_json)?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE queued_items
SET state = 'failed', claim_token = NULL, failure_jsonb = jsonb(?), updated_at_ms = ?
WHERE thread_id = ?
  AND state = 'claimed'
  AND updated_at_ms <= ?
            "#,
        )
        .bind(failure_json)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .bind(stale_before_ms)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn delete_thread_queue(&self, thread_id: ThreadId) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM queued_items WHERE thread_id = ?")
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn queued_item_from_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<crate::QueuedItemRecord> {
    crate::model::QueuedItemRow::try_from_row(row)?.try_into()
}

#[cfg(test)]
#[path = "queued_items_tests.rs"]
mod tests;
