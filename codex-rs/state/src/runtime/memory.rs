use super::*;
use crate::Stage1Output;
use crate::model::Stage1OutputRow;
use crate::model::ThreadRow;
use chrono::Duration;
use sqlx::Executor;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

const JOB_KIND_MEMORY_STAGE1: &str = "memory_stage1";
const JOB_KIND_MEMORY_CONSOLIDATE_CWD: &str = "memory_consolidate_cwd";
const JOB_KIND_MEMORY_CONSOLIDATE_USER: &str = "memory_consolidate_user";

const DEFAULT_RETRY_REMAINING: i64 = 3;

fn job_kind_for_scope(scope_kind: &str) -> Option<&'static str> {
    match scope_kind {
        MEMORY_SCOPE_KIND_CWD => Some(JOB_KIND_MEMORY_CONSOLIDATE_CWD),
        MEMORY_SCOPE_KIND_USER => Some(JOB_KIND_MEMORY_CONSOLIDATE_USER),
        _ => None,
    }
}

fn scope_kind_for_job_kind(job_kind: &str) -> Option<&'static str> {
    match job_kind {
        JOB_KIND_MEMORY_CONSOLIDATE_CWD => Some(MEMORY_SCOPE_KIND_CWD),
        JOB_KIND_MEMORY_CONSOLIDATE_USER => Some(MEMORY_SCOPE_KIND_USER),
        _ => None,
    }
}

fn normalize_cwd_for_scope_matching(cwd: &str) -> Option<PathBuf> {
    Path::new(cwd).canonicalize().ok()
}

impl StateRuntime {
    pub async fn claim_stage1_jobs_for_startup(
        &self,
        current_thread_id: ThreadId,
        params: Stage1StartupClaimParams<'_>,
    ) -> anyhow::Result<Vec<Stage1JobClaim>> {
        let Stage1StartupClaimParams {
            scan_limit,
            max_claimed,
            max_age_days,
            min_rollout_idle_hours,
            allowed_sources,
            lease_seconds,
        } = params;
        if scan_limit == 0 || max_claimed == 0 {
            return Ok(Vec::new());
        }

        let worker_id = current_thread_id;
        let current_thread_id = worker_id.to_string();
        let max_age_cutoff = (Utc::now() - Duration::days(max_age_days.max(0))).timestamp();
        let idle_cutoff = (Utc::now() - Duration::hours(min_rollout_idle_hours.max(0))).timestamp();

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
            "#,
        );
        push_thread_filters(
            &mut builder,
            false,
            allowed_sources,
            None,
            None,
            SortKey::UpdatedAt,
        );
        builder
            .push(" AND id != ")
            .push_bind(current_thread_id.as_str());
        builder
            .push(" AND updated_at >= ")
            .push_bind(max_age_cutoff);
        builder.push(" AND updated_at <= ").push_bind(idle_cutoff);
        push_thread_order_and_limit(&mut builder, SortKey::UpdatedAt, scan_limit);

        let items = builder
            .build()
            .fetch_all(self.pool.as_ref())
            .await?
            .into_iter()
            .map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .collect::<Result<Vec<_>, _>>()?;

        let mut claimed = Vec::new();

        for item in items {
            if claimed.len() >= max_claimed {
                break;
            }

            if let Stage1JobClaimOutcome::Claimed { ownership_token } = self
                .try_claim_stage1_job(
                    item.id,
                    worker_id,
                    item.updated_at.timestamp(),
                    lease_seconds,
                    max_claimed,
                )
                .await?
            {
                claimed.push(Stage1JobClaim {
                    thread: item,
                    ownership_token,
                });
            }
        }

        Ok(claimed)
    }

    pub async fn get_stage1_output(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Stage1Output>> {
        let row = sqlx::query(
            r#"
SELECT thread_id, source_updated_at, raw_memory, summary, generated_at
FROM stage1_outputs
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| Stage1OutputRow::try_from_row(&row).and_then(Stage1Output::try_from))
            .transpose()
    }

    pub async fn list_stage1_outputs_for_scope(
        &self,
        scope_kind: &str,
        scope_key: &str,
        n: usize,
    ) -> anyhow::Result<Vec<Stage1Output>> {
        if n == 0 {
            return Ok(Vec::new());
        }

        let rows = match scope_kind {
            MEMORY_SCOPE_KIND_CWD => {
                let exact_rows = sqlx::query(
                    r#"
SELECT so.thread_id, so.source_updated_at, so.raw_memory, so.summary, so.generated_at
FROM stage1_outputs AS so
JOIN threads AS t ON t.id = so.thread_id
WHERE t.cwd = ?
ORDER BY so.source_updated_at DESC, so.thread_id DESC
LIMIT ?
                    "#,
                )
                .bind(scope_key)
                .bind(n as i64)
                .fetch_all(self.pool.as_ref())
                .await?;

                if let Some(normalized_scope_key) = normalize_cwd_for_scope_matching(scope_key) {
                    let mut rows = Vec::new();
                    let mut selected_thread_ids = HashSet::new();
                    let candidate_rows = sqlx::query(
                        r#"
SELECT so.thread_id, so.source_updated_at, so.raw_memory, so.summary, so.generated_at, t.cwd AS thread_cwd
FROM stage1_outputs AS so
JOIN threads AS t ON t.id = so.thread_id
ORDER BY so.source_updated_at DESC, so.thread_id DESC
                        "#,
                    )
                    .fetch_all(self.pool.as_ref())
                    .await?;

                    for row in candidate_rows {
                        if rows.len() >= n {
                            break;
                        }
                        let thread_id: String = row.try_get("thread_id")?;
                        if selected_thread_ids.contains(&thread_id) {
                            continue;
                        }
                        let thread_cwd: String = row.try_get("thread_cwd")?;
                        if let Some(normalized_thread_cwd) =
                            normalize_cwd_for_scope_matching(&thread_cwd)
                            && normalized_thread_cwd == normalized_scope_key
                        {
                            selected_thread_ids.insert(thread_id);
                            rows.push(row);
                        }
                    }
                    if rows.is_empty() { exact_rows } else { rows }
                } else {
                    exact_rows
                }
            }
            MEMORY_SCOPE_KIND_USER => {
                sqlx::query(
                    r#"
SELECT so.thread_id, so.source_updated_at, so.raw_memory, so.summary, so.generated_at
FROM stage1_outputs AS so
JOIN threads AS t ON t.id = so.thread_id
ORDER BY so.source_updated_at DESC, so.thread_id DESC
LIMIT ?
                    "#,
                )
                .bind(n as i64)
                .fetch_all(self.pool.as_ref())
                .await?
            }
            _ => return Ok(Vec::new()),
        };

        rows.into_iter()
            .map(|row| Stage1OutputRow::try_from_row(&row).and_then(Stage1Output::try_from))
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn try_claim_stage1_job(
        &self,
        thread_id: ThreadId,
        worker_id: ThreadId,
        source_updated_at: i64,
        lease_seconds: i64,
        max_running_jobs: usize,
    ) -> anyhow::Result<Stage1JobClaimOutcome> {
        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(lease_seconds.max(0));
        let max_running_jobs = max_running_jobs as i64;
        let ownership_token = Uuid::new_v4().to_string();
        let thread_id = thread_id.to_string();
        let worker_id = worker_id.to_string();

        let mut tx = self.pool.begin().await?;

        let existing_output = sqlx::query(
            r#"
SELECT source_updated_at
FROM stage1_outputs
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_output) = existing_output {
            let existing_source_updated_at: i64 = existing_output.try_get("source_updated_at")?;
            if existing_source_updated_at >= source_updated_at {
                tx.commit().await?;
                return Ok(Stage1JobClaimOutcome::SkippedUpToDate);
            }
        }

        let existing_job = sqlx::query(
            r#"
SELECT status, lease_until, retry_at, retry_remaining
FROM jobs
WHERE kind = ? AND job_key = ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        let should_insert = if let Some(existing_job) = existing_job {
            let status: String = existing_job.try_get("status")?;
            let existing_lease_until: Option<i64> = existing_job.try_get("lease_until")?;
            let retry_at: Option<i64> = existing_job.try_get("retry_at")?;
            let retry_remaining: i64 = existing_job.try_get("retry_remaining")?;

            if retry_remaining <= 0 {
                tx.commit().await?;
                return Ok(Stage1JobClaimOutcome::SkippedRetryExhausted);
            }
            if retry_at.is_some_and(|retry_at| retry_at > now) {
                tx.commit().await?;
                return Ok(Stage1JobClaimOutcome::SkippedRetryBackoff);
            }
            if status == "running"
                && existing_lease_until.is_some_and(|lease_until| lease_until > now)
            {
                tx.commit().await?;
                return Ok(Stage1JobClaimOutcome::SkippedRunning);
            }

            false
        } else {
            true
        };

        let fresh_running_jobs = sqlx::query(
            r#"
SELECT COUNT(*) AS count
FROM jobs
WHERE kind = ?
  AND status = 'running'
  AND lease_until IS NOT NULL
  AND lease_until > ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?
        .try_get::<i64, _>("count")?;
        if fresh_running_jobs >= max_running_jobs {
            tx.commit().await?;
            return Ok(Stage1JobClaimOutcome::SkippedRunning);
        }

        if should_insert {
            sqlx::query(
                r#"
INSERT INTO jobs (
    kind,
    job_key,
    status,
    worker_id,
    ownership_token,
    started_at,
    finished_at,
    lease_until,
    retry_at,
    retry_remaining,
    last_error,
    input_watermark,
    last_success_watermark
) VALUES (?, ?, 'running', ?, ?, ?, NULL, ?, NULL, ?, NULL, ?, NULL)
                "#,
            )
            .bind(JOB_KIND_MEMORY_STAGE1)
            .bind(thread_id.as_str())
            .bind(worker_id.as_str())
            .bind(ownership_token.as_str())
            .bind(now)
            .bind(lease_until)
            .bind(DEFAULT_RETRY_REMAINING)
            .bind(source_updated_at)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(Stage1JobClaimOutcome::Claimed { ownership_token });
        }

        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'running',
    worker_id = ?,
    ownership_token = ?,
    started_at = ?,
    finished_at = NULL,
    lease_until = ?,
    retry_at = NULL,
    last_error = NULL,
    input_watermark = ?
WHERE kind = ? AND job_key = ?
  AND (status != 'running' OR lease_until IS NULL OR lease_until <= ?)
  AND (retry_at IS NULL OR retry_at <= ?)
  AND retry_remaining > 0
            "#,
        )
        .bind(worker_id.as_str())
        .bind(ownership_token.as_str())
        .bind(now)
        .bind(lease_until)
        .bind(source_updated_at)
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;
        if rows_affected == 0 {
            Ok(Stage1JobClaimOutcome::SkippedRunning)
        } else {
            Ok(Stage1JobClaimOutcome::Claimed { ownership_token })
        }
    }

    pub async fn mark_stage1_job_succeeded(
        &self,
        thread_id: ThreadId,
        ownership_token: &str,
        source_updated_at: i64,
        raw_memory: &str,
        summary: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let thread_id = thread_id.to_string();

        let mut tx = self.pool.begin().await?;
        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'done',
    finished_at = ?,
    lease_until = NULL,
    last_error = NULL,
    last_success_watermark = input_watermark
WHERE kind = ? AND job_key = ?
  AND status = 'running' AND ownership_token = ?
            "#,
        )
        .bind(now)
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .bind(ownership_token)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            tx.commit().await?;
            return Ok(false);
        }

        sqlx::query(
            r#"
INSERT INTO stage1_outputs (
    thread_id,
    source_updated_at,
    raw_memory,
    summary,
    generated_at
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    source_updated_at = excluded.source_updated_at,
    raw_memory = excluded.raw_memory,
    summary = excluded.summary,
    generated_at = excluded.generated_at
WHERE excluded.source_updated_at >= stage1_outputs.source_updated_at
            "#,
        )
        .bind(thread_id.as_str())
        .bind(source_updated_at)
        .bind(raw_memory)
        .bind(summary)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        if let Some(thread_row) = sqlx::query(
            r#"
SELECT cwd
FROM threads
WHERE id = ?
            "#,
        )
        .bind(thread_id.as_str())
        .fetch_optional(&mut *tx)
        .await?
        {
            let cwd: String = thread_row.try_get("cwd")?;
            let normalized_cwd = normalize_cwd_for_scope_matching(&cwd)
                .unwrap_or_else(|| PathBuf::from(&cwd))
                .display()
                .to_string();
            enqueue_scope_consolidation_with_executor(
                &mut *tx,
                MEMORY_SCOPE_KIND_CWD,
                &normalized_cwd,
                source_updated_at,
            )
            .await?;
            enqueue_scope_consolidation_with_executor(
                &mut *tx,
                MEMORY_SCOPE_KIND_USER,
                MEMORY_SCOPE_KEY_USER,
                source_updated_at,
            )
            .await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    pub async fn mark_stage1_job_failed(
        &self,
        thread_id: ThreadId,
        ownership_token: &str,
        failure_reason: &str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let retry_at = now.saturating_add(retry_delay_seconds.max(0));
        let thread_id = thread_id.to_string();

        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'error',
    finished_at = ?,
    lease_until = NULL,
    retry_at = ?,
    retry_remaining = retry_remaining - 1,
    last_error = ?
WHERE kind = ? AND job_key = ?
  AND status = 'running' AND ownership_token = ?
            "#,
        )
        .bind(now)
        .bind(retry_at)
        .bind(failure_reason)
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn enqueue_scope_consolidation(
        &self,
        scope_kind: &str,
        scope_key: &str,
        input_watermark: i64,
    ) -> anyhow::Result<()> {
        enqueue_scope_consolidation_with_executor(
            self.pool.as_ref(),
            scope_kind,
            scope_key,
            input_watermark,
        )
        .await
    }

    pub async fn list_pending_scope_consolidations(
        &self,
        limit: usize,
    ) -> anyhow::Result<Vec<PendingScopeConsolidation>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let now = Utc::now().timestamp();

        let rows = sqlx::query(
            r#"
SELECT kind, job_key
FROM jobs
WHERE kind IN (?, ?)
  AND input_watermark IS NOT NULL
  AND input_watermark > COALESCE(last_success_watermark, 0)
  AND retry_remaining > 0
  AND (retry_at IS NULL OR retry_at <= ?)
  AND (status != 'running' OR lease_until IS NULL OR lease_until <= ?)
ORDER BY input_watermark DESC, kind ASC, job_key ASC
LIMIT ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_CWD)
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_USER)
        .bind(now)
        .bind(now)
        .bind(limit as i64)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let kind: String = row.try_get("kind").ok()?;
                let scope_kind = scope_kind_for_job_kind(&kind)?;
                let scope_key: String = row.try_get("job_key").ok()?;
                Some(PendingScopeConsolidation {
                    scope_kind: scope_kind.to_string(),
                    scope_key,
                })
            })
            .collect::<Vec<_>>())
    }

    /// Try to claim a phase-2 consolidation job for `(scope_kind, scope_key)`.
    pub async fn try_claim_phase2_job(
        &self,
        scope_kind: &str,
        scope_key: &str,
        worker_id: ThreadId,
        lease_seconds: i64,
    ) -> anyhow::Result<Phase2JobClaimOutcome> {
        let Some(job_kind) = job_kind_for_scope(scope_kind) else {
            return Ok(Phase2JobClaimOutcome::SkippedNotDirty);
        };

        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(lease_seconds.max(0));
        let ownership_token = Uuid::new_v4().to_string();
        let worker_id = worker_id.to_string();

        let mut tx = self.pool.begin().await?;

        let existing_job = sqlx::query(
            r#"
SELECT status, lease_until, retry_at, retry_remaining, input_watermark, last_success_watermark
FROM jobs
WHERE kind = ? AND job_key = ?
            "#,
        )
        .bind(job_kind)
        .bind(scope_key)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(existing_job) = existing_job else {
            tx.commit().await?;
            return Ok(Phase2JobClaimOutcome::SkippedNotDirty);
        };

        let input_watermark: Option<i64> = existing_job.try_get("input_watermark")?;
        let input_watermark_value = input_watermark.unwrap_or(0);
        let last_success_watermark: Option<i64> = existing_job.try_get("last_success_watermark")?;
        if input_watermark_value <= last_success_watermark.unwrap_or(0) {
            tx.commit().await?;
            return Ok(Phase2JobClaimOutcome::SkippedNotDirty);
        }

        let status: String = existing_job.try_get("status")?;
        let existing_lease_until: Option<i64> = existing_job.try_get("lease_until")?;
        let retry_at: Option<i64> = existing_job.try_get("retry_at")?;
        let retry_remaining: i64 = existing_job.try_get("retry_remaining")?;

        if retry_remaining <= 0 {
            tx.commit().await?;
            return Ok(Phase2JobClaimOutcome::SkippedNotDirty);
        }
        if retry_at.is_some_and(|retry_at| retry_at > now) {
            tx.commit().await?;
            return Ok(Phase2JobClaimOutcome::SkippedNotDirty);
        }
        if status == "running" && existing_lease_until.is_some_and(|lease_until| lease_until > now)
        {
            tx.commit().await?;
            return Ok(Phase2JobClaimOutcome::SkippedRunning);
        }

        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'running',
    worker_id = ?,
    ownership_token = ?,
    started_at = ?,
    finished_at = NULL,
    lease_until = ?,
    retry_at = NULL,
    last_error = NULL
WHERE kind = ? AND job_key = ?
  AND (status != 'running' OR lease_until IS NULL OR lease_until <= ?)
  AND (retry_at IS NULL OR retry_at <= ?)
  AND retry_remaining > 0
            "#,
        )
        .bind(worker_id.as_str())
        .bind(ownership_token.as_str())
        .bind(now)
        .bind(lease_until)
        .bind(job_kind)
        .bind(scope_key)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;
        if rows_affected == 0 {
            Ok(Phase2JobClaimOutcome::SkippedRunning)
        } else {
            Ok(Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark: input_watermark_value,
            })
        }
    }

    pub async fn heartbeat_phase2_job(
        &self,
        scope_kind: &str,
        scope_key: &str,
        ownership_token: &str,
        lease_seconds: i64,
    ) -> anyhow::Result<bool> {
        let Some(job_kind) = job_kind_for_scope(scope_kind) else {
            return Ok(false);
        };

        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(lease_seconds.max(0));
        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET lease_until = ?
WHERE kind = ? AND job_key = ?
  AND status = 'running' AND ownership_token = ?
            "#,
        )
        .bind(lease_until)
        .bind(job_kind)
        .bind(scope_key)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn mark_phase2_job_succeeded(
        &self,
        scope_kind: &str,
        scope_key: &str,
        ownership_token: &str,
        completed_watermark: i64,
    ) -> anyhow::Result<bool> {
        let Some(job_kind) = job_kind_for_scope(scope_kind) else {
            return Ok(false);
        };

        let now = Utc::now().timestamp();
        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'done',
    finished_at = ?,
    lease_until = NULL,
    last_error = NULL,
    last_success_watermark = max(COALESCE(last_success_watermark, 0), ?)
WHERE kind = ? AND job_key = ?
  AND status = 'running' AND ownership_token = ?
            "#,
        )
        .bind(now)
        .bind(completed_watermark)
        .bind(job_kind)
        .bind(scope_key)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn mark_phase2_job_failed(
        &self,
        scope_kind: &str,
        scope_key: &str,
        ownership_token: &str,
        failure_reason: &str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool> {
        let Some(job_kind) = job_kind_for_scope(scope_kind) else {
            return Ok(false);
        };

        let now = Utc::now().timestamp();
        let retry_at = now.saturating_add(retry_delay_seconds.max(0));
        let rows_affected = sqlx::query(
            r#"
UPDATE jobs
SET
    status = 'error',
    finished_at = ?,
    lease_until = NULL,
    retry_at = ?,
    retry_remaining = retry_remaining - 1,
    last_error = ?
WHERE kind = ? AND job_key = ?
  AND status = 'running' AND ownership_token = ?
            "#,
        )
        .bind(now)
        .bind(retry_at)
        .bind(failure_reason)
        .bind(job_kind)
        .bind(scope_key)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }
}

async fn enqueue_scope_consolidation_with_executor<'e, E>(
    executor: E,
    scope_kind: &str,
    scope_key: &str,
    input_watermark: i64,
) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let Some(job_kind) = job_kind_for_scope(scope_kind) else {
        return Ok(());
    };

    sqlx::query(
        r#"
INSERT INTO jobs (
    kind,
    job_key,
    status,
    worker_id,
    ownership_token,
    started_at,
    finished_at,
    lease_until,
    retry_at,
    retry_remaining,
    last_error,
    input_watermark,
    last_success_watermark
) VALUES (?, ?, 'pending', NULL, NULL, NULL, NULL, NULL, NULL, ?, NULL, ?, 0)
ON CONFLICT(kind, job_key) DO UPDATE SET
    status = CASE
        WHEN jobs.status = 'running' THEN 'running'
        ELSE 'pending'
    END,
    retry_at = CASE
        WHEN jobs.status = 'running' THEN jobs.retry_at
        ELSE NULL
    END,
    retry_remaining = max(jobs.retry_remaining, excluded.retry_remaining),
    input_watermark = max(COALESCE(jobs.input_watermark, 0), excluded.input_watermark)
        "#,
    )
    .bind(job_kind)
    .bind(scope_key)
    .bind(DEFAULT_RETRY_REMAINING)
    .bind(input_watermark)
    .execute(executor)
    .await?;

    Ok(())
}
