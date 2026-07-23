use super::*;

impl StateRuntime {
    pub async fn get_backfill_state(&self) -> anyhow::Result<crate::BackfillState> {
        self.ensure_backfill_state_row().await?;
        let row = sqlx::query(
            r#"
SELECT status, last_watermark, last_success_at
FROM backfill_state
WHERE id = 1
            "#,
        )
        .fetch_one(self.pool.as_ref())
        .await?;
        crate::BackfillState::try_from_row(&row)
    }

    /// Attempt to claim ownership of rollout metadata backfill.
    ///
    /// Returns `true` when this runtime claimed the backfill worker slot.
    /// Returns `false` if backfill is already complete or currently owned by a
    /// non-expired worker.
    pub async fn try_claim_backfill(&self, lease_seconds: i64) -> anyhow::Result<bool> {
        self.ensure_backfill_state_row().await?;
        let now = Utc::now().timestamp();
        let lease_cutoff = now.saturating_sub(lease_seconds.max(0));
        let result = sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
  AND status != ?
  AND (status != ? OR updated_at <= ?)
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(now)
        .bind(crate::BackfillStatus::Complete.as_str())
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(lease_cutoff)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Mark rollout metadata backfill as running.
    pub async fn mark_backfill_running(&self) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Persist rollout metadata backfill progress.
    pub async fn checkpoint_backfill(&self, watermark: &str) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, last_watermark = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(watermark)
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Mark rollout metadata backfill as complete.
    pub async fn mark_backfill_complete(&self, last_watermark: Option<&str>) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
UPDATE backfill_state
SET
    status = ?,
    last_watermark = COALESCE(?, last_watermark),
    last_success_at = ?,
    updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Complete.as_str())
        .bind(last_watermark)
        .bind(now)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    async fn ensure_backfill_state_row(&self) -> anyhow::Result<()> {
        ensure_backfill_state_row_in_pool(self.pool.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use chrono::Utc;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use sqlx::Connection;

    #[tokio::test]
    async fn backfill_state_persists_progress_and_completion() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(
            crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
            "test-provider".to_string(),
        )
        .await
        .expect("initialize runtime");

        let initial = runtime
            .get_backfill_state()
            .await
            .expect("get initial backfill state");
        assert_eq!(initial.status, crate::BackfillStatus::Pending);
        assert_eq!(initial.last_watermark, None);
        assert_eq!(initial.last_success_at, None);

        runtime
            .mark_backfill_running()
            .await
            .expect("mark backfill running");
        runtime
            .checkpoint_backfill("sessions/2026/01/27/rollout-a.jsonl")
            .await
            .expect("checkpoint backfill");

        let running = runtime
            .get_backfill_state()
            .await
            .expect("get running backfill state");
        assert_eq!(running.status, crate::BackfillStatus::Running);
        assert_eq!(
            running.last_watermark,
            Some("sessions/2026/01/27/rollout-a.jsonl".to_string())
        );
        assert_eq!(running.last_success_at, None);

        runtime
            .mark_backfill_complete(Some("sessions/2026/01/28/rollout-b.jsonl"))
            .await
            .expect("mark backfill complete");
        let completed = runtime
            .get_backfill_state()
            .await
            .expect("get completed backfill state");
        assert_eq!(completed.status, crate::BackfillStatus::Complete);
        assert_eq!(
            completed.last_watermark,
            Some("sessions/2026/01/28/rollout-b.jsonl".to_string())
        );
        assert!(completed.last_success_at.is_some());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_backfill_state_succeeds_while_another_connection_holds_writer_slot() {
        let codex_home = unique_temp_dir();
        let sqlite = crate::SqliteConfig::new_for_testing(codex_home.as_path().abs());
        let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");
        let write_pool = sqlite
            .open_read_write_pool(&sqlite.state_db_path())
            .await
            .expect("open write pool");
        let mut write_connection = write_pool.acquire().await.expect("open write connection");
        let write_transaction = write_connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("acquire write lock");

        let state = runtime
            .get_backfill_state()
            .await
            .expect("get backfill state");
        assert_eq!(state, crate::BackfillState::default());

        write_transaction
            .rollback()
            .await
            .expect("release write lock");
        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_backfill_state_repairs_a_missing_singleton_row() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(
            crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
            "test-provider".to_string(),
        )
        .await
        .expect("initialize runtime");
        sqlx::query("DELETE FROM backfill_state WHERE id = 1")
            .execute(runtime.pool.as_ref())
            .await
            .expect("delete backfill state row");

        let state = runtime
            .get_backfill_state()
            .await
            .expect("get repaired backfill state");
        assert_eq!(state, crate::BackfillState::default());
        let row_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM backfill_state WHERE id = 1")
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count repaired backfill state rows");
        assert_eq!(row_count, 1);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn backfill_claim_is_singleton_until_stale_and_blocked_when_complete() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(
            crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
            "test-provider".to_string(),
        )
        .await
        .expect("initialize runtime");

        let claimed = runtime
            .try_claim_backfill(/*lease_seconds*/ 3600)
            .await
            .expect("initial backfill claim");
        assert_eq!(claimed, true);

        let duplicate_claim = runtime
            .try_claim_backfill(/*lease_seconds*/ 3600)
            .await
            .expect("duplicate backfill claim");
        assert_eq!(duplicate_claim, false);

        let stale_updated_at = Utc::now().timestamp().saturating_sub(10_000);
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(stale_updated_at)
        .execute(runtime.pool.as_ref())
        .await
        .expect("force stale backfill lease");

        let stale_claim = runtime
            .try_claim_backfill(/*lease_seconds*/ 10)
            .await
            .expect("stale backfill claim");
        assert_eq!(stale_claim, true);

        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("mark complete");
        let claim_after_complete = runtime
            .try_claim_backfill(/*lease_seconds*/ 3600)
            .await
            .expect("claim after complete");
        assert_eq!(claim_after_complete, false);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
