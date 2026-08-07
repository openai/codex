//! Persists generic rollout-migration progress in the state database.
//!
//! It stores a monotonic creation-ordered cursor for each migration and records fingerprinted
//! rollouts that the migration could not process. Thread-store uses this state to avoid rescanning
//! old rollouts on every startup while still retrying skipped files that later change.

use super::*;

impl StateRuntime {
    pub async fn get_rollout_migration_state(
        &self,
        migration_id: &str,
    ) -> anyhow::Result<Option<crate::RolloutMigrationState>> {
        let row = sqlx::query(
            r#"
SELECT last_checked_thread_created_at, last_checked_thread_id
FROM rollout_migration_state
WHERE migration_id = ?
            "#,
        )
        .bind(migration_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.as_ref()
            .map(crate::RolloutMigrationState::try_from_row)
            .transpose()
    }

    /// Advance one migration's checked frontier without letting concurrent startup checks move
    /// it backward.
    pub async fn advance_rollout_migration_state(
        &self,
        migration_id: &str,
        last_checked_thread: Option<&crate::RolloutMigrationCursor>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let (thread_created_at, thread_id) = last_checked_thread.map_or((None, None), |cursor| {
            (
                Some(cursor.thread_created_at),
                Some(cursor.thread_id.as_str()),
            )
        });
        sqlx::query(
            r#"
INSERT INTO rollout_migration_state (
    migration_id,
    last_checked_thread_created_at,
    last_checked_thread_id,
    updated_at
)
VALUES (?, ?, ?, ?)
ON CONFLICT(migration_id) DO UPDATE SET
    last_checked_thread_created_at = excluded.last_checked_thread_created_at,
    last_checked_thread_id = excluded.last_checked_thread_id,
    updated_at = excluded.updated_at
WHERE excluded.last_checked_thread_created_at IS NOT NULL
  AND (
    rollout_migration_state.last_checked_thread_created_at IS NULL
    OR excluded.last_checked_thread_created_at
        > rollout_migration_state.last_checked_thread_created_at
    OR (
        excluded.last_checked_thread_created_at
            = rollout_migration_state.last_checked_thread_created_at
        AND excluded.last_checked_thread_id > rollout_migration_state.last_checked_thread_id
    )
  )
            "#,
        )
        .bind(migration_id)
        .bind(thread_created_at)
        .bind(thread_id)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn list_rollout_migration_skipped_rollouts(
        &self,
        migration_id: &str,
    ) -> anyhow::Result<Vec<crate::RolloutMigrationSkippedRollout>> {
        let rows = sqlx::query(
            r#"
SELECT rollout_path, rollout_size_bytes, rollout_modified_at_ns, skip_reason
FROM rollout_migration_skipped_rollouts
WHERE migration_id = ?
            "#,
        )
        .bind(migration_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.iter()
            .map(crate::RolloutMigrationSkippedRollout::try_from_row)
            .collect()
    }

    pub async fn record_rollout_migration_skip(
        &self,
        migration_id: &str,
        skipped_rollout: &crate::RolloutMigrationSkippedRollout,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
INSERT INTO rollout_migration_skipped_rollouts (
    migration_id,
    rollout_path,
    rollout_size_bytes,
    rollout_modified_at_ns,
    skip_reason,
    skipped_at
)
VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT(migration_id, rollout_path) DO UPDATE SET
    rollout_size_bytes = excluded.rollout_size_bytes,
    rollout_modified_at_ns = excluded.rollout_modified_at_ns,
    skip_reason = excluded.skip_reason,
    skipped_at = excluded.skipped_at
            "#,
        )
        .bind(migration_id)
        .bind(skipped_rollout.rollout_path.as_str())
        .bind(skipped_rollout.rollout_size_bytes)
        .bind(skipped_rollout.rollout_modified_at_ns)
        .bind(skipped_rollout.skip_reason.as_str())
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn remove_rollout_migration_skip(
        &self,
        migration_id: &str,
        rollout_path: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
DELETE FROM rollout_migration_skipped_rollouts
WHERE migration_id = ? AND rollout_path = ?
            "#,
        )
        .bind(migration_id)
        .bind(rollout_path)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
