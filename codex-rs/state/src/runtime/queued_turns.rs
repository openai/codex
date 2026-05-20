use super::*;
use uuid::Uuid;

impl StateRuntime {
    pub async fn append_thread_queued_turn(
        &self,
        thread_id: ThreadId,
        turn_start_params_json: &[u8],
    ) -> anyhow::Result<crate::ThreadQueuedTurn> {
        let queued_turn_id = Uuid::now_v7().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let row = sqlx::query(
            r#"
INSERT INTO thread_queued_turns (
    queued_turn_id,
    thread_id,
    turn_start_params_jsonb,
    queue_order,
    state,
    dispatch_turn_id,
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
    NULL,
    ?,
    ?
FROM thread_queued_turns
WHERE thread_id = ?
RETURNING
    queued_turn_id,
    thread_id,
    CAST(json(turn_start_params_jsonb) AS BLOB) AS turn_start_params_jsonb,
    queue_order,
    state,
    dispatch_turn_id,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(queued_turn_id)
        .bind(thread_id.to_string())
        .bind(turn_start_params_json)
        .bind(now_ms)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .fetch_one(self.pool.as_ref())
        .await?;

        thread_queued_turn_from_row(&row)
    }

    pub async fn list_visible_thread_queued_turns(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Vec<crate::ThreadQueuedTurn>> {
        self.list_visible_thread_queued_turns_page(thread_id, /*offset*/ 0, i64::MAX as usize)
            .await
    }

    pub async fn list_visible_thread_queued_turns_page(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::ThreadQueuedTurn>> {
        let rows = sqlx::query(
            r#"
SELECT
    queued_turn_id,
    thread_id,
    CAST(json(turn_start_params_jsonb) AS BLOB) AS turn_start_params_jsonb,
    queue_order,
    state,
    dispatch_turn_id,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
FROM thread_queued_turns
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

        rows.iter().map(thread_queued_turn_from_row).collect()
    }

    pub async fn delete_thread_queued_turn(
        &self,
        thread_id: ThreadId,
        queued_turn_id: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
DELETE FROM thread_queued_turns
WHERE thread_id = ?
  AND queued_turn_id = ?
  AND state IN ('pending', 'failed')
            "#,
        )
        .bind(thread_id.to_string())
        .bind(queued_turn_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn reorder_thread_queued_turns(
        &self,
        thread_id: ThreadId,
        ordered_ids: &[String],
    ) -> anyhow::Result<Vec<crate::ThreadQueuedTurn>> {
        let mut transaction = self.pool.begin().await?;
        let visible_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
SELECT queued_turn_id, queue_order
FROM thread_queued_turns
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
            .map(|(queued_turn_id, _)| queued_turn_id.clone())
            .collect::<Vec<_>>();
        let visible_queue_orders = visible_rows
            .into_iter()
            .map(|(_, queue_order)| queue_order)
            .collect::<Vec<_>>();
        let mut expected_ids = visible_ids.clone();
        expected_ids.sort();
        let mut requested_ids = ordered_ids.to_vec();
        requested_ids.sort();
        if expected_ids != requested_ids {
            anyhow::bail!("queue reorder must include every visible queued turn exactly once");
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        for (temporary_order, queued_turn_id) in ordered_ids.iter().enumerate() {
            sqlx::query(
                r#"
UPDATE thread_queued_turns
SET queue_order = ?, updated_at_ms = ?
WHERE thread_id = ?
  AND queued_turn_id = ?
  AND state IN ('pending', 'failed')
                "#,
            )
            .bind(-((temporary_order as i64) + 1))
            .bind(now_ms)
            .bind(thread_id.to_string())
            .bind(queued_turn_id)
            .execute(transaction.as_mut())
            .await?;
        }
        for (queue_order, queued_turn_id) in visible_queue_orders.into_iter().zip(ordered_ids) {
            sqlx::query(
                r#"
UPDATE thread_queued_turns
SET queue_order = ?, updated_at_ms = ?
WHERE thread_id = ?
  AND queued_turn_id = ?
                AND state IN ('pending', 'failed')
                "#,
            )
            .bind(queue_order)
            .bind(now_ms)
            .bind(thread_id.to_string())
            .bind(queued_turn_id)
            .execute(transaction.as_mut())
            .await?;
        }
        transaction.commit().await?;

        self.list_visible_thread_queued_turns(thread_id).await
    }

    pub async fn claim_head_thread_queued_turn(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<crate::ThreadQueuedTurn>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let row = sqlx::query(
            r#"
UPDATE thread_queued_turns
SET state = 'dispatching', updated_at_ms = ?
WHERE queued_turn_id = (
    SELECT head.queued_turn_id
    FROM thread_queued_turns AS head
    WHERE head.thread_id = ?
      AND head.state IN ('pending', 'failed')
      AND NOT EXISTS (
          SELECT 1
          FROM thread_queued_turns AS active
          WHERE active.thread_id = head.thread_id
            AND active.state = 'dispatching'
      )
    ORDER BY head.queue_order ASC
    LIMIT 1
)
  AND state = 'pending'
RETURNING
    queued_turn_id,
    thread_id,
    CAST(json(turn_start_params_jsonb) AS BLOB) AS turn_start_params_jsonb,
    queue_order,
    state,
    dispatch_turn_id,
    CASE
        WHEN failure_jsonb IS NULL THEN NULL
        ELSE CAST(json(failure_jsonb) AS BLOB)
    END AS failure_jsonb,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(now_ms)
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| thread_queued_turn_from_row(&row)).transpose()
    }

    pub async fn set_dispatching_thread_queued_turn_turn_id(
        &self,
        queued_turn_id: &str,
        turn_id: &str,
    ) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE thread_queued_turns
SET dispatch_turn_id = ?, updated_at_ms = ?
WHERE queued_turn_id = ?
  AND state = 'dispatching'
            "#,
        )
        .bind(turn_id)
        .bind(now_ms)
        .bind(queued_turn_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn remove_dispatching_thread_queued_turn(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
DELETE FROM thread_queued_turns
WHERE thread_id = ?
  AND state = 'dispatching'
  AND dispatch_turn_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .bind(turn_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_thread_queued_turn_failed(
        &self,
        queued_turn_id: &str,
        failure_json: &[u8],
    ) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE thread_queued_turns
SET
    state = 'failed',
    failure_jsonb = jsonb(?),
    updated_at_ms = ?
WHERE queued_turn_id = ?
  AND state = 'dispatching'
            "#,
        )
        .bind(failure_json)
        .bind(now_ms)
        .bind(queued_turn_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn recover_dispatching_thread_queued_turns(
        &self,
        thread_id: ThreadId,
        failure_json: &[u8],
    ) -> anyhow::Result<u64> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE thread_queued_turns
SET
    state = 'failed',
    failure_jsonb = jsonb(?),
    updated_at_ms = ?
WHERE thread_id = ?
  AND state = 'dispatching'
            "#,
        )
        .bind(failure_json)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected())
    }
}

fn thread_queued_turn_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<crate::ThreadQueuedTurn> {
    crate::model::ThreadQueuedTurnRow::try_from_row(row)?.try_into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::test_thread_metadata;
    use crate::runtime::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;

    async fn runtime_with_thread() -> (Arc<StateRuntime>, ThreadId) {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state runtime");
        let thread_id = ThreadId::new();
        runtime
            .upsert_thread(&test_thread_metadata(
                codex_home.as_path(),
                thread_id,
                codex_home.clone(),
            ))
            .await
            .expect("insert thread");
        (runtime, thread_id)
    }

    #[tokio::test]
    async fn queued_turn_claim_is_single_winner_and_hides_dispatching_row() {
        let (runtime, thread_id) = runtime_with_thread().await;
        runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append queued turn");

        let (first, second) = tokio::join!(
            runtime.claim_head_thread_queued_turn(thread_id),
            runtime.claim_head_thread_queued_turn(thread_id),
        );
        let claimed = [first.expect("first claim"), second.expect("second claim")]
            .into_iter()
            .flatten()
            .count();
        assert_eq!(claimed, 1);
        assert_eq!(
            runtime
                .list_visible_thread_queued_turns(thread_id)
                .await
                .expect("list visible queued turns"),
            Vec::new()
        );
    }

    #[tokio::test]
    async fn queued_turn_added_during_dispatch_claim_waits_for_existing_claim() {
        let (runtime, thread_id) = runtime_with_thread().await;
        runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append first");
        runtime
            .claim_head_thread_queued_turn(thread_id)
            .await
            .expect("claim first")
            .expect("claimed row");

        let second = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append second");

        assert_eq!(
            runtime
                .claim_head_thread_queued_turn(thread_id)
                .await
                .expect("claim blocked by dispatch"),
            None
        );
        assert_eq!(
            runtime
                .list_visible_thread_queued_turns(thread_id)
                .await
                .expect("list visible queued turns"),
            vec![second]
        );
    }

    #[tokio::test]
    async fn dispatch_claim_rejects_stale_mutations_and_keeps_later_rows_reorderable() {
        let (runtime, thread_id) = runtime_with_thread().await;
        let first = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append first");
        let second = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append second");
        let third = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append third");

        runtime
            .claim_head_thread_queued_turn(thread_id)
            .await
            .expect("claim first")
            .expect("claimed row");

        assert!(
            !runtime
                .delete_thread_queued_turn(thread_id, &first.queued_turn_id)
                .await
                .expect("dispatching row is not deletable")
        );
        assert!(
            runtime
                .reorder_thread_queued_turns(
                    thread_id,
                    &[
                        first.queued_turn_id.clone(),
                        third.queued_turn_id.clone(),
                        second.queued_turn_id.clone(),
                    ],
                )
                .await
                .is_err()
        );
        let reordered_ids = runtime
            .reorder_thread_queued_turns(
                thread_id,
                &[third.queued_turn_id.clone(), second.queued_turn_id.clone()],
            )
            .await
            .expect("reorder visible rows")
            .into_iter()
            .map(|queued_turn| queued_turn.queued_turn_id)
            .collect::<Vec<_>>();
        assert_eq!(
            reordered_ids,
            vec![third.queued_turn_id, second.queued_turn_id]
        );
    }

    #[tokio::test]
    async fn abandoned_dispatch_claim_recovers_as_failed_and_blocks_fifo() {
        let (runtime, thread_id) = runtime_with_thread().await;
        let first = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append first");
        runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append second");
        runtime
            .claim_head_thread_queued_turn(thread_id)
            .await
            .expect("claim first")
            .expect("claimed row");

        assert_eq!(
            runtime
                .recover_dispatching_thread_queued_turns(
                    thread_id,
                    br#"{"message":"dispatch interrupted"}"#,
                )
                .await
                .expect("recover dispatching rows"),
            1
        );

        let visible = runtime
            .list_visible_thread_queued_turns(thread_id)
            .await
            .expect("list recovered queue");
        assert_eq!(visible[0].queued_turn_id, first.queued_turn_id);
        assert_eq!(visible[0].state, crate::ThreadQueuedTurnState::Failed);
        assert_eq!(
            runtime
                .claim_head_thread_queued_turn(thread_id)
                .await
                .expect("failed head blocks claim"),
            None
        );
    }

    #[tokio::test]
    async fn failed_head_blocks_later_pending_work_until_removed() {
        let (runtime, thread_id) = runtime_with_thread().await;
        let first = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append first");
        runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append second");

        let claimed = runtime
            .claim_head_thread_queued_turn(thread_id)
            .await
            .expect("claim first")
            .expect("claimed row");
        assert_eq!(claimed.queued_turn_id, first.queued_turn_id);
        runtime
            .mark_thread_queued_turn_failed(&claimed.queued_turn_id, br#"{"message":"nope"}"#)
            .await
            .expect("mark failed");

        assert_eq!(
            runtime
                .claim_head_thread_queued_turn(thread_id)
                .await
                .expect("blocked claim"),
            None
        );
        assert!(
            runtime
                .delete_thread_queued_turn(thread_id, &first.queued_turn_id)
                .await
                .expect("delete failed head")
        );
        assert!(
            runtime
                .claim_head_thread_queued_turn(thread_id)
                .await
                .expect("claim next")
                .is_some()
        );
    }

    #[tokio::test]
    async fn dispatch_claim_clears_only_for_its_submitted_turn() {
        let (runtime, thread_id) = runtime_with_thread().await;
        let queued_turn = runtime
            .append_thread_queued_turn(thread_id, br#"{"threadId":"t","input":[]}"#)
            .await
            .expect("append queued turn");
        runtime
            .claim_head_thread_queued_turn(thread_id)
            .await
            .expect("claim queued turn")
            .expect("claimed row");

        assert!(
            !runtime
                .remove_dispatching_thread_queued_turn(thread_id, "regular-turn")
                .await
                .expect("unmatched turn must not clear claim")
        );
        assert!(
            runtime
                .set_dispatching_thread_queued_turn_turn_id(
                    &queued_turn.queued_turn_id,
                    "queued-turn",
                )
                .await
                .expect("record submitted queued turn id")
        );
        assert!(
            !runtime
                .remove_dispatching_thread_queued_turn(thread_id, "regular-turn")
                .await
                .expect("different started turn must not clear claim")
        );
        assert!(
            runtime
                .remove_dispatching_thread_queued_turn(thread_id, "queued-turn")
                .await
                .expect("matching queued turn clears claim")
        );
    }
}
