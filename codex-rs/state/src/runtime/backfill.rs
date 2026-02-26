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
        sqlx::query(
            r#"
INSERT INTO backfill_state (id, status, last_watermark, last_success_at, updated_at)
VALUES (?, ?, NULL, NULL, ?)
ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(1_i64)
        .bind(crate::BackfillStatus::Pending.as_str())
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::state_db_filename;
    use super::test_support::unique_temp_dir;
    use crate::STATE_DB_FILENAME;
    use crate::STATE_DB_VERSION;
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn init_removes_legacy_state_db_files() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex_home");

        let current_name = state_db_filename();
        let previous_version = STATE_DB_VERSION.saturating_sub(1);
        let unversioned_name = format!("{STATE_DB_FILENAME}.sqlite");
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let path = codex_home.join(format!("{unversioned_name}{suffix}"));
            tokio::fs::write(path, b"legacy")
                .await
                .expect("write legacy");
            let old_version_path = codex_home.join(format!(
                "{STATE_DB_FILENAME}_{previous_version}.sqlite{suffix}"
            ));
            tokio::fs::write(old_version_path, b"old_version")
                .await
                .expect("write old version");
        }
        let unrelated_path = codex_home.join("state.sqlite_backup");
        tokio::fs::write(&unrelated_path, b"keep")
            .await
            .expect("write unrelated");
        let numeric_path = codex_home.join("123");
        tokio::fs::write(&numeric_path, b"keep")
            .await
            .expect("write numeric");

        let _runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        for suffix in ["", "-wal", "-shm", "-journal"] {
            let legacy_path = codex_home.join(format!("{unversioned_name}{suffix}"));
            assert_eq!(
                tokio::fs::try_exists(&legacy_path)
                    .await
                    .expect("check legacy path"),
                false
            );
            let old_version_path = codex_home.join(format!(
                "{STATE_DB_FILENAME}_{previous_version}.sqlite{suffix}"
            ));
            assert_eq!(
                tokio::fs::try_exists(&old_version_path)
                    .await
                    .expect("check old version path"),
                false
            );
        }
        assert_eq!(
            tokio::fs::try_exists(codex_home.join(current_name))
                .await
                .expect("check new db path"),
            true
        );
        assert_eq!(
            tokio::fs::try_exists(&unrelated_path)
                .await
                .expect("check unrelated path"),
            true
        );
        assert_eq!(
            tokio::fs::try_exists(&numeric_path)
                .await
                .expect("check numeric path"),
            true
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn backfill_state_persists_progress_and_completion() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
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
    async fn backfill_claim_is_singleton_until_stale_and_blocked_when_complete() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let claimed = runtime
            .try_claim_backfill(3600)
            .await
            .expect("initial backfill claim");
        assert_eq!(claimed, true);

        let duplicate_claim = runtime
            .try_claim_backfill(3600)
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
            .try_claim_backfill(10)
            .await
            .expect("stale backfill claim");
        assert_eq!(stale_claim, true);

        runtime
            .mark_backfill_complete(None)
            .await
            .expect("mark complete");
        let claim_after_complete = runtime
            .try_claim_backfill(3600)
            .await
            .expect("claim after complete");
        assert_eq!(claim_after_complete, false);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
