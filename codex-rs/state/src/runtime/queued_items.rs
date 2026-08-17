use super::*;
use crate::MAX_QUEUE_ITEMS;
use crate::QueuedUserSubmissionRecord;
use sqlx::Connection;
use tokio::sync::Mutex;
use uuid::Uuid;

/// SQLite-backed persistence for durable, thread-scoped user messages.
#[derive(Clone)]
pub struct SqliteQueueStore {
    pool: Arc<SqlitePool>,
    change_version_connection: Arc<Mutex<Option<SqliteConnection>>>,
}

impl SqliteQueueStore {
    pub(crate) fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            pool,
            change_version_connection: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn close(&self) {
        let connection = self.change_version_connection.lock().await.take();
        if let Some(connection) = connection
            && let Err(error) = connection.close().await
        {
            tracing::warn!(%error, "failed to close queue change-version connection");
        }
        self.pool.close().await;
    }

    /// Observe queue-database commits through one stable SQLite connection.
    pub async fn change_version(&self) -> anyhow::Result<i64> {
        let mut connection = Arc::clone(&self.change_version_connection)
            .lock_owned()
            .await;
        if connection.is_none() {
            *connection = Some(self.pool.acquire().await?.detach());
        }
        let Some(connection) = connection.as_mut() else {
            unreachable!("queue change-version connection was initialized");
        };
        Ok(sqlx::query_scalar("PRAGMA data_version")
            .fetch_one(connection)
            .await?)
    }

    /// Return changed revisions only for the supplied loaded thread IDs.
    pub async fn changes_since(
        &self,
        revision: i64,
        thread_ids: &[ThreadId],
    ) -> anyhow::Result<Vec<(ThreadId, i64)>> {
        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT thread_id, revision FROM queued_thread_revisions WHERE revision > ",
        );
        query.push_bind(revision).push(" AND thread_id IN (");
        let mut separated = query.separated(", ");
        for thread_id in thread_ids {
            separated.push_bind(thread_id.to_string());
        }
        separated.push_unseparated(") ORDER BY revision");
        let rows = query
            .build_query_as::<(String, i64)>()
            .fetch_all(self.pool.as_ref())
            .await?;
        rows.into_iter()
            .map(|(thread_id, revision)| Ok((ThreadId::try_from(thread_id)?, revision)))
            .collect()
    }

    pub async fn enqueue(
        &self,
        thread_id: ThreadId,
        payload_json: &str,
    ) -> anyhow::Result<QueuedUserSubmissionRecord> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let row = sqlx::query(
            "INSERT INTO queued_items (
                id, thread_id, payload_json, queue_order,
                created_at_ms, updated_at_ms
             )
             SELECT ?, ?, ?,
                    COALESCE((SELECT MAX(queue_order) FROM queued_items WHERE thread_id = ?), -1) + 1,
                    ?, ?
             WHERE (SELECT COUNT(*) FROM queued_items WHERE thread_id = ?) < ?
             RETURNING id, thread_id, payload_json",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(thread_id.to_string())
        .bind(payload_json)
        .bind(thread_id.to_string())
        .bind(now_ms)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .bind(i64::try_from(MAX_QUEUE_ITEMS)?)
        .fetch_one(self.pool.as_ref())
        .await?;
        QueuedUserSubmissionRecord::try_from_row(&row)
    }

    pub async fn list_page(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<QueuedUserSubmissionRecord>> {
        let rows = sqlx::query(
            "SELECT id, thread_id, payload_json
             FROM queued_items
             WHERE thread_id = ?
             ORDER BY queue_order LIMIT ? OFFSET ?",
        )
        .bind(thread_id.to_string())
        .bind(i64::try_from(limit)?)
        .bind(i64::try_from(offset)?)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.iter()
            .map(QueuedUserSubmissionRecord::try_from_row)
            .collect()
    }

    pub async fn update(
        &self,
        thread_id: ThreadId,
        item_id: &str,
        payload_json: &str,
    ) -> anyhow::Result<Option<QueuedUserSubmissionRecord>> {
        let row = sqlx::query(
            "UPDATE queued_items
             SET payload_json = ?, updated_at_ms = ?
             WHERE thread_id = ? AND id = ?
             RETURNING id, thread_id, payload_json",
        )
        .bind(payload_json)
        .bind(datetime_to_epoch_millis(Utc::now()))
        .bind(thread_id.to_string())
        .bind(item_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.as_ref()
            .map(QueuedUserSubmissionRecord::try_from_row)
            .transpose()
    }

    pub async fn delete(&self, thread_id: ThreadId, item_id: &str) -> anyhow::Result<bool> {
        Ok(
            sqlx::query("DELETE FROM queued_items WHERE thread_id = ? AND id = ?")
                .bind(thread_id.to_string())
                .bind(item_id)
                .execute(self.pool.as_ref())
                .await?
                .rows_affected()
                > 0,
        )
    }

    pub async fn reorder(&self, thread_id: ThreadId, ordered_ids: &[String]) -> anyhow::Result<()> {
        let mut transaction = self.pool.begin().await?;
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, queue_order FROM queued_items
             WHERE thread_id = ? ORDER BY queue_order",
        )
        .bind(thread_id.to_string())
        .fetch_all(transaction.as_mut())
        .await?;
        let mut expected_ids = rows.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let mut requested_ids = ordered_ids.to_vec();
        expected_ids.sort();
        requested_ids.sort();
        if expected_ids != requested_ids {
            transaction.rollback().await?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "queue reorder must include every queued submission exactly once",
            )
            .into());
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let max_queue_order = rows.last().map_or(-1, |(_, queue_order)| *queue_order);
        for (index, item_id) in ordered_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE queued_items SET queue_order = ?, updated_at_ms = ?
                 WHERE thread_id = ? AND id = ?",
            )
            .bind(max_queue_order + i64::try_from(index)? + 1)
            .bind(now_ms)
            .bind(thread_id.to_string())
            .bind(item_id)
            .execute(transaction.as_mut())
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub(crate) async fn delete_thread_queue(&self, thread_id: ThreadId) -> anyhow::Result<bool> {
        Ok(sqlx::query("DELETE FROM queued_items WHERE thread_id = ?")
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?
            .rows_affected()
            > 0)
    }
}

#[cfg(test)]
#[path = "queued_items_tests.rs"]
mod tests;
