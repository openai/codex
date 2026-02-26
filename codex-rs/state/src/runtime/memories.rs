use super::threads::push_thread_filters;
use super::threads::push_thread_order_and_limit;
use super::*;
use crate::model::Phase2InputSelection;
use crate::model::Phase2JobClaimOutcome;
use crate::model::Stage1JobClaim;
use crate::model::Stage1JobClaimOutcome;
use crate::model::Stage1Output;
use crate::model::Stage1OutputRow;
use crate::model::Stage1StartupClaimParams;
use crate::model::ThreadRow;
use crate::model::stage1_output_ref_from_parts;
use chrono::Duration;
use sqlx::Executor;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use std::collections::HashSet;
use uuid::Uuid;

const JOB_KIND_MEMORY_STAGE1: &str = "memory_stage1";
const JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL: &str = "memory_consolidate_global";
const MEMORY_CONSOLIDATION_JOB_KEY: &str = "global";

const DEFAULT_RETRY_REMAINING: i64 = 3;

impl StateRuntime {
    /// Deletes all persisted memory state in one transaction.
    ///
    /// This removes every `stage1_outputs` row and all `jobs` rows for the
    /// stage-1 (`memory_stage1`) and phase-2 (`memory_consolidate_global`)
    /// memory pipelines.
    pub async fn clear_memory_data(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
DELETE FROM stage1_outputs
            "#,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM jobs
WHERE kind = ? OR kind = ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Record usage for cited stage-1 outputs.
    ///
    /// Each thread id increments `usage_count` by one and sets `last_usage` to
    /// the current Unix timestamp. Missing rows are ignored.
    pub async fn record_stage1_output_usage(
        &self,
        thread_ids: &[ThreadId],
    ) -> anyhow::Result<usize> {
        if thread_ids.is_empty() {
            return Ok(0);
        }

        let now = Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;
        let mut updated_rows = 0;

        for thread_id in thread_ids {
            updated_rows += sqlx::query(
                r#"
UPDATE stage1_outputs
SET
    usage_count = COALESCE(usage_count, 0) + 1,
    last_usage = ?
WHERE thread_id = ?
                "#,
            )
            .bind(now)
            .bind(thread_id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected() as usize;
        }

        tx.commit().await?;
        Ok(updated_rows)
    }

    /// Selects and claims stage-1 startup jobs for stale threads.
    ///
    /// Query behavior:
    /// - starts from `threads` filtered to active threads and allowed sources
    ///   (`push_thread_filters`)
    /// - excludes the current thread id
    /// - keeps only threads in the age window:
    ///   `updated_at >= now - max_age_days` and `updated_at <= now - min_rollout_idle_hours`
    /// - keeps only threads whose memory is stale:
    ///   `COALESCE(stage1_outputs.source_updated_at, -1) < threads.updated_at` and
    ///   `COALESCE(jobs.last_success_watermark, -1) < threads.updated_at`
    /// - orders by `updated_at DESC, id DESC` and applies `scan_limit`
    ///
    /// For each selected thread, this function calls [`Self::try_claim_stage1_job`]
    /// with `source_updated_at = thread.updated_at.timestamp()` and returns up to
    /// `max_claimed` successful claims.
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
    agent_nickname,
    agent_role,
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
LEFT JOIN stage1_outputs
    ON stage1_outputs.thread_id = threads.id
LEFT JOIN jobs
    ON jobs.kind = 
            "#,
        );
        builder.push_bind(JOB_KIND_MEMORY_STAGE1);
        builder.push(
            r#"
   AND jobs.job_key = threads.id
            "#,
        );
        push_thread_filters(
            &mut builder,
            false,
            allowed_sources,
            None,
            None,
            SortKey::UpdatedAt,
            None,
        );
        builder
            .push(" AND id != ")
            .push_bind(current_thread_id.as_str());
        builder
            .push(" AND updated_at >= ")
            .push_bind(max_age_cutoff);
        builder.push(" AND updated_at <= ").push_bind(idle_cutoff);
        builder.push(" AND COALESCE(stage1_outputs.source_updated_at, -1) < updated_at");
        builder.push(" AND COALESCE(jobs.last_success_watermark, -1) < updated_at");
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

    /// Lists the most recent non-empty stage-1 outputs for global consolidation.
    ///
    /// Query behavior:
    /// - filters out rows where both `raw_memory` and `rollout_summary` are blank
    /// - joins `threads` to include thread `cwd` and `rollout_path`
    /// - orders by `source_updated_at DESC, thread_id DESC`
    /// - applies `LIMIT n`
    pub async fn list_stage1_outputs_for_global(
        &self,
        n: usize,
    ) -> anyhow::Result<Vec<Stage1Output>> {
        if n == 0 {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
SELECT
    so.thread_id,
    COALESCE(t.rollout_path, '') AS rollout_path,
    so.source_updated_at,
    so.raw_memory,
    so.rollout_summary,
    so.rollout_slug,
    so.generated_at
     , COALESCE(t.cwd, '') AS cwd
FROM stage1_outputs AS so
LEFT JOIN threads AS t
    ON t.id = so.thread_id
WHERE length(trim(so.raw_memory)) > 0 OR length(trim(so.rollout_summary)) > 0
ORDER BY so.source_updated_at DESC, so.thread_id DESC
LIMIT ?
            "#,
        )
        .bind(n as i64)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| Stage1OutputRow::try_from_row(&row).and_then(Stage1Output::try_from))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Returns the current phase-2 input set along with its diff against the
    /// last successful phase-2 selection.
    ///
    /// Query behavior:
    /// - current selection is the latest `n` non-empty stage-1 outputs ordered
    ///   by `source_updated_at DESC, thread_id DESC`
    /// - previously selected rows are identified by `selected_for_phase2 = 1`
    /// - `previous_selected` contains the current persisted rows that belonged
    ///   to the last successful phase-2 baseline
    /// - `retained_thread_ids` records which current rows still match the exact
    ///   snapshot selected in the last successful phase-2 run
    /// - removed rows are previously selected rows that are still present in
    ///   `stage1_outputs` but fall outside the current top-`n` selection
    pub async fn get_phase2_input_selection(
        &self,
        n: usize,
    ) -> anyhow::Result<Phase2InputSelection> {
        if n == 0 {
            return Ok(Phase2InputSelection::default());
        }

        let current_rows = sqlx::query(
            r#"
SELECT
    so.thread_id,
    COALESCE(t.rollout_path, '') AS rollout_path,
    so.source_updated_at,
    so.raw_memory,
    so.rollout_summary,
    so.rollout_slug,
    so.generated_at,
    so.selected_for_phase2,
    so.selected_for_phase2_source_updated_at,
    COALESCE(t.cwd, '') AS cwd
FROM stage1_outputs AS so
LEFT JOIN threads AS t
    ON t.id = so.thread_id
WHERE length(trim(so.raw_memory)) > 0 OR length(trim(so.rollout_summary)) > 0
ORDER BY so.source_updated_at DESC, so.thread_id DESC
LIMIT ?
            "#,
        )
        .bind(n as i64)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut current_thread_ids = HashSet::with_capacity(current_rows.len());
        let mut selected = Vec::with_capacity(current_rows.len());
        let mut retained_thread_ids = Vec::new();
        for row in current_rows {
            let thread_id = row.try_get::<String, _>("thread_id")?;
            current_thread_ids.insert(thread_id.clone());
            let source_updated_at = row.try_get::<i64, _>("source_updated_at")?;
            if row.try_get::<i64, _>("selected_for_phase2")? != 0
                && row.try_get::<Option<i64>, _>("selected_for_phase2_source_updated_at")?
                    == Some(source_updated_at)
            {
                retained_thread_ids.push(ThreadId::try_from(thread_id.clone())?);
            }
            selected.push(Stage1Output::try_from(Stage1OutputRow::try_from_row(
                &row,
            )?)?);
        }

        let previous_rows = sqlx::query(
            r#"
SELECT
    so.thread_id,
    COALESCE(t.rollout_path, '') AS rollout_path,
    so.source_updated_at,
    so.raw_memory,
    so.rollout_summary,
    so.rollout_slug
  , so.generated_at
  , COALESCE(t.cwd, '') AS cwd
FROM stage1_outputs AS so
LEFT JOIN threads AS t
    ON t.id = so.thread_id
WHERE so.selected_for_phase2 = 1
ORDER BY so.source_updated_at DESC, so.thread_id DESC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        let previous_selected = previous_rows
            .iter()
            .map(Stage1OutputRow::try_from_row)
            .map(|row| row.and_then(Stage1Output::try_from))
            .collect::<Result<Vec<_>, _>>()?;
        let mut removed = Vec::new();
        for row in previous_rows {
            let thread_id = row.try_get::<String, _>("thread_id")?;
            if current_thread_ids.contains(thread_id.as_str()) {
                continue;
            }
            removed.push(stage1_output_ref_from_parts(
                thread_id,
                row.try_get("source_updated_at")?,
                row.try_get("rollout_slug")?,
            )?);
        }

        Ok(Phase2InputSelection {
            selected,
            previous_selected,
            retained_thread_ids,
            removed,
        })
    }

    /// Attempts to claim a stage-1 job for a thread at `source_updated_at`.
    ///
    /// Claim semantics:
    /// - skips as up-to-date when either:
    ///   - `stage1_outputs.source_updated_at >= source_updated_at`, or
    ///   - `jobs.last_success_watermark >= source_updated_at`
    /// - inserts or updates a `jobs` row to `running` only when:
    ///   - global running job count for `memory_stage1` is below `max_running_jobs`
    ///   - existing row is not actively running with a valid lease
    ///   - retry backoff (if present) has elapsed, or `source_updated_at` advanced
    ///   - retries remain, or `source_updated_at` advanced (which resets retries)
    ///
    /// The update path refreshes ownership token, lease, and `input_watermark`.
    /// If claiming fails, a follow-up read maps current row state to a precise
    /// skip outcome (`SkippedRunning`, `SkippedRetryBackoff`, or
    /// `SkippedRetryExhausted`).
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

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

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
SELECT last_success_watermark
FROM jobs
WHERE kind = ? AND job_key = ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_job) = existing_job {
            let last_success_watermark =
                existing_job.try_get::<Option<i64>, _>("last_success_watermark")?;
            if last_success_watermark.is_some_and(|watermark| watermark >= source_updated_at) {
                tx.commit().await?;
                return Ok(Stage1JobClaimOutcome::SkippedUpToDate);
            }
        }

        let rows_affected = sqlx::query(
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
)
SELECT ?, ?, 'running', ?, ?, ?, NULL, ?, NULL, ?, NULL, ?, NULL
WHERE (
    SELECT COUNT(*)
    FROM jobs
    WHERE kind = ?
      AND status = 'running'
      AND lease_until IS NOT NULL
      AND lease_until > ?
) < ?
ON CONFLICT(kind, job_key) DO UPDATE SET
    status = 'running',
    worker_id = excluded.worker_id,
    ownership_token = excluded.ownership_token,
    started_at = excluded.started_at,
    finished_at = NULL,
    lease_until = excluded.lease_until,
    retry_at = NULL,
    retry_remaining = CASE
        WHEN excluded.input_watermark > COALESCE(jobs.input_watermark, -1) THEN ?
        ELSE jobs.retry_remaining
    END,
    last_error = NULL,
    input_watermark = excluded.input_watermark
WHERE
    (jobs.status != 'running' OR jobs.lease_until IS NULL OR jobs.lease_until <= excluded.started_at)
    AND (
        jobs.retry_at IS NULL
        OR jobs.retry_at <= excluded.started_at
        OR excluded.input_watermark > COALESCE(jobs.input_watermark, -1)
    )
    AND (
        jobs.retry_remaining > 0
        OR excluded.input_watermark > COALESCE(jobs.input_watermark, -1)
    )
    AND (
        SELECT COUNT(*)
        FROM jobs AS running_jobs
        WHERE running_jobs.kind = excluded.kind
          AND running_jobs.status = 'running'
          AND running_jobs.lease_until IS NOT NULL
          AND running_jobs.lease_until > excluded.started_at
          AND running_jobs.job_key != excluded.job_key
    ) < ?
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
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(now)
        .bind(max_running_jobs)
        .bind(DEFAULT_RETRY_REMAINING)
        .bind(max_running_jobs)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            tx.commit().await?;
            return Ok(Stage1JobClaimOutcome::Claimed { ownership_token });
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

        tx.commit().await?;

        if let Some(existing_job) = existing_job {
            let status: String = existing_job.try_get("status")?;
            let existing_lease_until: Option<i64> = existing_job.try_get("lease_until")?;
            let retry_at: Option<i64> = existing_job.try_get("retry_at")?;
            let retry_remaining: i64 = existing_job.try_get("retry_remaining")?;

            if retry_remaining <= 0 {
                return Ok(Stage1JobClaimOutcome::SkippedRetryExhausted);
            }
            if retry_at.is_some_and(|retry_at| retry_at > now) {
                return Ok(Stage1JobClaimOutcome::SkippedRetryBackoff);
            }
            if status == "running"
                && existing_lease_until.is_some_and(|lease_until| lease_until > now)
            {
                return Ok(Stage1JobClaimOutcome::SkippedRunning);
            }
        }

        Ok(Stage1JobClaimOutcome::SkippedRunning)
    }

    /// Marks a claimed stage-1 job successful and upserts generated output.
    ///
    /// Transaction behavior:
    /// - updates `jobs` only for the currently owned running row
    /// - sets `status='done'` and `last_success_watermark = input_watermark`
    /// - upserts `stage1_outputs` for the thread, replacing existing output only
    ///   when `source_updated_at` is newer or equal
    /// - preserves any existing `selected_for_phase2` baseline until the next
    ///   successful phase-2 run rewrites the baseline selection, including the
    ///   snapshot timestamp chosen during that run
    /// - persists optional `rollout_slug` for rollout summary artifact naming
    /// - enqueues/advances the global phase-2 job watermark using
    ///   `source_updated_at`
    pub async fn mark_stage1_job_succeeded(
        &self,
        thread_id: ThreadId,
        ownership_token: &str,
        source_updated_at: i64,
        raw_memory: &str,
        rollout_summary: &str,
        rollout_slug: Option<&str>,
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
    rollout_summary,
    rollout_slug,
    generated_at
) VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    source_updated_at = excluded.source_updated_at,
    raw_memory = excluded.raw_memory,
    rollout_summary = excluded.rollout_summary,
    rollout_slug = excluded.rollout_slug,
    generated_at = excluded.generated_at
WHERE excluded.source_updated_at >= stage1_outputs.source_updated_at
            "#,
        )
        .bind(thread_id.as_str())
        .bind(source_updated_at)
        .bind(raw_memory)
        .bind(rollout_summary)
        .bind(rollout_slug)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        enqueue_global_consolidation_with_executor(&mut *tx, source_updated_at).await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Marks a claimed stage-1 job successful when extraction produced no output.
    ///
    /// Transaction behavior:
    /// - updates `jobs` only for the currently owned running row
    /// - sets `status='done'` and `last_success_watermark = input_watermark`
    /// - deletes any existing `stage1_outputs` row for the thread
    /// - enqueues/advances the global phase-2 job watermark using the claimed
    ///   `input_watermark` only when deleting an existing `stage1_outputs` row
    pub async fn mark_stage1_job_succeeded_no_output(
        &self,
        thread_id: ThreadId,
        ownership_token: &str,
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

        let source_updated_at = sqlx::query(
            r#"
SELECT input_watermark
FROM jobs
WHERE kind = ? AND job_key = ? AND ownership_token = ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_STAGE1)
        .bind(thread_id.as_str())
        .bind(ownership_token)
        .fetch_one(&mut *tx)
        .await?
        .try_get::<i64, _>("input_watermark")?;

        let deleted_rows = sqlx::query(
            r#"
DELETE FROM stage1_outputs
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if deleted_rows > 0 {
            enqueue_global_consolidation_with_executor(&mut *tx, source_updated_at).await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    /// Marks a claimed stage-1 job as failed and schedules retry backoff.
    ///
    /// Query behavior:
    /// - updates only the owned running row for `(kind='memory_stage1', job_key)`
    /// - sets `status='error'`, clears lease, writes `last_error`
    /// - decrements `retry_remaining`
    /// - sets `retry_at = now + retry_delay_seconds`
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

    /// Enqueues or advances the global phase-2 consolidation job watermark.
    ///
    /// The underlying upsert keeps the job `running` when already running, resets
    /// `pending/error` jobs to `pending`, and advances `input_watermark` so each
    /// enqueue marks new consolidation work even when `source_updated_at` is
    /// older than prior maxima.
    pub async fn enqueue_global_consolidation(&self, input_watermark: i64) -> anyhow::Result<()> {
        enqueue_global_consolidation_with_executor(self.pool.as_ref(), input_watermark).await
    }

    /// Attempts to claim the global phase-2 consolidation job.
    ///
    /// Claim semantics:
    /// - reads the singleton global job row (`kind='memory_consolidate_global'`)
    /// - returns `SkippedNotDirty` when `input_watermark <= last_success_watermark`
    /// - returns `SkippedNotDirty` when retries are exhausted or retry backoff is active
    /// - returns `SkippedRunning` when an active running lease exists
    /// - otherwise updates the row to `running`, sets ownership + lease, and
    ///   returns `Claimed`
    pub async fn try_claim_global_phase2_job(
        &self,
        worker_id: ThreadId,
        lease_seconds: i64,
    ) -> anyhow::Result<Phase2JobClaimOutcome> {
        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(lease_seconds.max(0));
        let ownership_token = Uuid::new_v4().to_string();
        let worker_id = worker_id.to_string();

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        let existing_job = sqlx::query(
            r#"
SELECT status, lease_until, retry_at, retry_remaining, input_watermark, last_success_watermark
FROM jobs
WHERE kind = ? AND job_key = ?
            "#,
        )
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
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
  AND input_watermark > COALESCE(last_success_watermark, 0)
  AND (status != 'running' OR lease_until IS NULL OR lease_until <= ?)
  AND (retry_at IS NULL OR retry_at <= ?)
  AND retry_remaining > 0
            "#,
        )
        .bind(worker_id.as_str())
        .bind(ownership_token.as_str())
        .bind(now)
        .bind(lease_until)
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
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

    /// Extends the lease for an owned running phase-2 global job.
    ///
    /// Query behavior:
    /// - `UPDATE jobs SET lease_until = ?` for the singleton global row
    /// - requires `status='running'` and matching `ownership_token`
    pub async fn heartbeat_global_phase2_job(
        &self,
        ownership_token: &str,
        lease_seconds: i64,
    ) -> anyhow::Result<bool> {
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
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    /// Marks the owned running global phase-2 job as succeeded.
    ///
    /// Query behavior:
    /// - updates only the owned running singleton global row
    /// - sets `status='done'`, clears lease/errors
    /// - advances `last_success_watermark` to
    ///   `max(existing_last_success_watermark, completed_watermark)`
    /// - rewrites `selected_for_phase2` so only the exact selected stage-1
    ///   snapshots remain marked as part of the latest successful phase-2
    ///   selection, and persists each selected snapshot's
    ///   `source_updated_at` for future retained-vs-added diffing
    pub async fn mark_global_phase2_job_succeeded(
        &self,
        ownership_token: &str,
        completed_watermark: i64,
        selected_outputs: &[Stage1Output],
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;
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
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
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
UPDATE stage1_outputs
SET
    selected_for_phase2 = 0,
    selected_for_phase2_source_updated_at = NULL
WHERE selected_for_phase2 != 0 OR selected_for_phase2_source_updated_at IS NOT NULL
            "#,
        )
        .execute(&mut *tx)
        .await?;

        for output in selected_outputs {
            sqlx::query(
                r#"
UPDATE stage1_outputs
SET
    selected_for_phase2 = 1,
    selected_for_phase2_source_updated_at = ?
WHERE thread_id = ? AND source_updated_at = ?
                "#,
            )
            .bind(output.source_updated_at.timestamp())
            .bind(output.thread_id.to_string())
            .bind(output.source_updated_at.timestamp())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    /// Marks the owned running global phase-2 job as failed and schedules retry.
    ///
    /// Query behavior:
    /// - updates only the owned running singleton global row
    /// - sets `status='error'`, clears lease
    /// - writes failure reason and retry time
    /// - decrements `retry_remaining`
    pub async fn mark_global_phase2_job_failed(
        &self,
        ownership_token: &str,
        failure_reason: &str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool> {
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
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    /// Fallback failure finalization when ownership may have been lost.
    ///
    /// Query behavior:
    /// - same state transition as [`Self::mark_global_phase2_job_failed`]
    /// - matches rows where `ownership_token = ? OR ownership_token IS NULL`
    /// - allows recovering a stuck unowned running row
    pub async fn mark_global_phase2_job_failed_if_unowned(
        &self,
        ownership_token: &str,
        failure_reason: &str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool> {
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
  AND status = 'running'
  AND (ownership_token = ? OR ownership_token IS NULL)
            "#,
        )
        .bind(now)
        .bind(retry_at)
        .bind(failure_reason)
        .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
        .bind(MEMORY_CONSOLIDATION_JOB_KEY)
        .bind(ownership_token)
        .execute(self.pool.as_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }
}

async fn enqueue_global_consolidation_with_executor<'e, E>(
    executor: E,
    input_watermark: i64,
) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
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
    input_watermark = CASE
        WHEN excluded.input_watermark > COALESCE(jobs.input_watermark, 0)
            THEN excluded.input_watermark
        ELSE COALESCE(jobs.input_watermark, 0) + 1
    END
        "#,
    )
    .bind(JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL)
    .bind(MEMORY_CONSOLIDATION_JOB_KEY)
    .bind(DEFAULT_RETRY_REMAINING)
    .bind(input_watermark)
    .execute(executor)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::test_thread_metadata;
    use super::test_support::unique_temp_dir;
    use crate::model::Phase2JobClaimOutcome;
    use crate::model::Stage1JobClaimOutcome;
    use crate::model::Stage1StartupClaimParams;
    use chrono::Duration;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use sqlx::Row;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn stage1_claim_skips_when_up_to_date() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.join("a"));
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("upsert thread");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner_a, 100, 3600, 64)
            .await
            .expect("claim stage1 job");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };

        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    ownership_token.as_str(),
                    100,
                    "raw",
                    "sum",
                    None,
                )
                .await
                .expect("mark stage1 succeeded"),
            "stage1 success should finalize for current token"
        );

        let up_to_date = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim stage1 up-to-date");
        assert_eq!(up_to_date, Stage1JobClaimOutcome::SkippedUpToDate);

        let needs_rerun = runtime
            .try_claim_stage1_job(thread_id, owner_b, 101, 3600, 64)
            .await
            .expect("claim stage1 newer source");
        assert!(
            matches!(needs_rerun, Stage1JobClaimOutcome::Claimed { .. }),
            "newer source_updated_at should be claimable"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_running_stale_can_be_stolen_but_fresh_running_is_skipped() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd))
            .await
            .expect("upsert thread");

        let claim_a = runtime
            .try_claim_stage1_job(thread_id, owner_a, 100, 3600, 64)
            .await
            .expect("claim a");
        assert!(matches!(claim_a, Stage1JobClaimOutcome::Claimed { .. }));

        let claim_b_fresh = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim b fresh");
        assert_eq!(claim_b_fresh, Stage1JobClaimOutcome::SkippedRunning);

        sqlx::query("UPDATE jobs SET lease_until = 0 WHERE kind = 'memory_stage1' AND job_key = ?")
            .bind(thread_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .expect("force stale lease");

        let claim_b_stale = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim b stale");
        assert!(matches!(
            claim_b_stale,
            Stage1JobClaimOutcome::Claimed { .. }
        ));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_concurrent_claim_for_same_thread_is_conflict_safe() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let thread_id_a = thread_id;
        let thread_id_b = thread_id;
        let runtime_a = Arc::clone(&runtime);
        let runtime_b = Arc::clone(&runtime);
        let claim_with_retry = |runtime: Arc<StateRuntime>,
                                thread_id: ThreadId,
                                owner: ThreadId| async move {
            for attempt in 0..5 {
                match runtime
                    .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
                    .await
                {
                    Ok(outcome) => return outcome,
                    Err(err) if err.to_string().contains("database is locked") && attempt < 4 => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(err) => panic!("claim stage1 should not fail: {err}"),
                }
            }
            panic!("claim stage1 should have returned within retry budget")
        };

        let (claim_a, claim_b) = tokio::join!(
            claim_with_retry(runtime_a, thread_id_a, owner_a),
            claim_with_retry(runtime_b, thread_id_b, owner_b),
        );

        let claim_outcomes = vec![claim_a, claim_b];
        let claimed_count = claim_outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Stage1JobClaimOutcome::Claimed { .. }))
            .count();
        assert_eq!(claimed_count, 1);
        assert!(
            claim_outcomes.iter().all(|outcome| {
                matches!(
                    outcome,
                    Stage1JobClaimOutcome::Claimed { .. } | Stage1JobClaimOutcome::SkippedRunning
                )
            }),
            "unexpected claim outcomes: {claim_outcomes:?}"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_concurrent_claims_respect_running_cap() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let runtime_a = Arc::clone(&runtime);
        let runtime_b = Arc::clone(&runtime);

        let (claim_a, claim_b) = tokio::join!(
            async move {
                runtime_a
                    .try_claim_stage1_job(thread_a, owner_a, 100, 3_600, 1)
                    .await
                    .expect("claim stage1 thread a")
            },
            async move {
                runtime_b
                    .try_claim_stage1_job(thread_b, owner_b, 101, 3_600, 1)
                    .await
                    .expect("claim stage1 thread b")
            },
        );

        let claim_outcomes = vec![claim_a, claim_b];
        let claimed_count = claim_outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Stage1JobClaimOutcome::Claimed { .. }))
            .count();
        assert_eq!(claimed_count, 1);
        assert!(
            claim_outcomes
                .iter()
                .any(|outcome| { matches!(outcome, Stage1JobClaimOutcome::SkippedRunning) }),
            "one concurrent claim should be throttled by running cap: {claim_outcomes:?}"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_filters_by_age_idle_and_current_thread() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let now = Utc::now();
        let fresh_at = now - Duration::hours(1);
        let just_under_idle_at = now - Duration::hours(12) + Duration::minutes(1);
        let eligible_idle_at = now - Duration::hours(12) - Duration::minutes(1);
        let old_at = now - Duration::days(31);

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let fresh_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("fresh thread id");
        let just_under_idle_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("just under idle thread id");
        let eligible_idle_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("eligible idle thread id");
        let old_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("old thread id");

        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = now;
        current.updated_at = now;
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current");

        let mut fresh =
            test_thread_metadata(&codex_home, fresh_thread_id, codex_home.join("fresh"));
        fresh.created_at = fresh_at;
        fresh.updated_at = fresh_at;
        runtime.upsert_thread(&fresh).await.expect("upsert fresh");

        let mut just_under_idle = test_thread_metadata(
            &codex_home,
            just_under_idle_thread_id,
            codex_home.join("just-under-idle"),
        );
        just_under_idle.created_at = just_under_idle_at;
        just_under_idle.updated_at = just_under_idle_at;
        runtime
            .upsert_thread(&just_under_idle)
            .await
            .expect("upsert just-under-idle");

        let mut eligible_idle = test_thread_metadata(
            &codex_home,
            eligible_idle_thread_id,
            codex_home.join("eligible-idle"),
        );
        eligible_idle.created_at = eligible_idle_at;
        eligible_idle.updated_at = eligible_idle_at;
        runtime
            .upsert_thread(&eligible_idle)
            .await
            .expect("upsert eligible-idle");

        let mut old = test_thread_metadata(&codex_home, old_thread_id, codex_home.join("old"));
        old.created_at = old_at;
        old.updated_at = old_at;
        runtime.upsert_thread(&old).await.expect("upsert old");

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 1,
                    max_claimed: 5,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs");

        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].thread.id, eligible_idle_thread_id);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_prefilters_threads_with_up_to_date_memory() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let now = Utc::now();
        let eligible_newer_at = now - Duration::hours(13);
        let eligible_older_at = now - Duration::hours(14);

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let up_to_date_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("up-to-date thread id");
        let stale_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("stale thread id");
        let worker_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("worker id");

        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = now;
        current.updated_at = now;
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current thread");

        let mut up_to_date = test_thread_metadata(
            &codex_home,
            up_to_date_thread_id,
            codex_home.join("up-to-date"),
        );
        up_to_date.created_at = eligible_newer_at;
        up_to_date.updated_at = eligible_newer_at;
        runtime
            .upsert_thread(&up_to_date)
            .await
            .expect("upsert up-to-date thread");

        let up_to_date_claim = runtime
            .try_claim_stage1_job(
                up_to_date_thread_id,
                worker_id,
                up_to_date.updated_at.timestamp(),
                3600,
                64,
            )
            .await
            .expect("claim up-to-date thread for seed");
        let up_to_date_token = match up_to_date_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected seed claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    up_to_date_thread_id,
                    up_to_date_token.as_str(),
                    up_to_date.updated_at.timestamp(),
                    "raw",
                    "summary",
                    None,
                )
                .await
                .expect("mark up-to-date thread succeeded"),
            "seed stage1 success should complete for up-to-date thread"
        );

        let mut stale =
            test_thread_metadata(&codex_home, stale_thread_id, codex_home.join("stale"));
        stale.created_at = eligible_older_at;
        stale.updated_at = eligible_older_at;
        runtime
            .upsert_thread(&stale)
            .await
            .expect("upsert stale thread");

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 1,
                    max_claimed: 1,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 startup jobs");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].thread.id, stale_thread_id);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_enforces_global_running_cap() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                current_thread_id,
                codex_home.join("current"),
            ))
            .await
            .expect("upsert current");

        let now = Utc::now();
        let started_at = now.timestamp();
        let lease_until = started_at + 3600;
        let eligible_at = now - Duration::hours(13);
        let existing_running = 10usize;
        let total_candidates = 80usize;

        for idx in 0..total_candidates {
            let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
            let mut metadata = test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join(format!("thread-{idx}")),
            );
            metadata.created_at = eligible_at - Duration::seconds(idx as i64);
            metadata.updated_at = eligible_at - Duration::seconds(idx as i64);
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("upsert thread");

            if idx < existing_running {
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
                .bind("memory_stage1")
                .bind(thread_id.to_string())
                .bind(current_thread_id.to_string())
                .bind(Uuid::new_v4().to_string())
                .bind(started_at)
                .bind(lease_until)
                .bind(3)
                .bind(metadata.updated_at.timestamp())
                .execute(runtime.pool.as_ref())
                .await
                .expect("seed running stage1 job");
            }
        }

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 200,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs");
        assert_eq!(claims.len(), 54);

        let running_count = sqlx::query(
            r#"
SELECT COUNT(*) AS count
FROM jobs
WHERE kind = 'memory_stage1'
  AND status = 'running'
  AND lease_until IS NOT NULL
  AND lease_until > ?
            "#,
        )
        .bind(Utc::now().timestamp())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("count running stage1 jobs")
        .try_get::<i64, _>("count")
        .expect("running count value");
        assert_eq!(running_count, 64);

        let more_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 200,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs with cap reached");
        assert_eq!(more_claims.len(), 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_processes_two_full_batches_across_startup_passes() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = Utc::now();
        current.updated_at = Utc::now();
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current");

        let eligible_at = Utc::now() - Duration::hours(13);
        for idx in 0..200 {
            let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
            let mut metadata = test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join(format!("thread-{idx}")),
            );
            metadata.created_at = eligible_at - Duration::seconds(idx as i64);
            metadata.updated_at = eligible_at - Duration::seconds(idx as i64);
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("upsert eligible thread");
        }

        let allowed_sources = vec!["cli".to_string()];
        let first_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 5_000,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3_600,
                },
            )
            .await
            .expect("first stage1 startup claim");
        assert_eq!(first_claims.len(), 64);

        for claim in first_claims {
            assert!(
                runtime
                    .mark_stage1_job_succeeded(
                        claim.thread.id,
                        claim.ownership_token.as_str(),
                        claim.thread.updated_at.timestamp(),
                        "raw",
                        "summary",
                        None,
                    )
                    .await
                    .expect("mark first-batch stage1 success"),
                "first batch stage1 completion should succeed"
            );
        }

        let second_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 5_000,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3_600,
                },
            )
            .await
            .expect("second stage1 startup claim");
        assert_eq!(second_claims.len(), 64);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_output_cascades_on_thread_delete() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd))
            .await
            .expect("upsert thread");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim stage1");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    ownership_token.as_str(),
                    100,
                    "raw",
                    "sum",
                    None,
                )
                .await
                .expect("mark stage1 succeeded"),
            "mark stage1 succeeded should write stage1_outputs"
        );

        let count_before =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count before delete")
                .try_get::<i64, _>("count")
                .expect("count value");
        assert_eq!(count_before, 1);

        sqlx::query("DELETE FROM threads WHERE id = ?")
            .bind(thread_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .expect("delete thread");

        let count_after =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count after delete")
                .try_get::<i64, _>("count")
                .expect("count value");
        assert_eq!(count_after, 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_stage1_job_succeeded_no_output_skips_phase2_when_output_was_already_absent() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim stage1");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded_no_output(thread_id, ownership_token.as_str())
                .await
                .expect("mark stage1 succeeded without output"),
            "stage1 no-output success should complete the job"
        );

        let output_row_count =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("load stage1 output count")
                .try_get::<i64, _>("count")
                .expect("stage1 output count");
        assert_eq!(
            output_row_count, 0,
            "stage1 no-output success should not persist empty stage1 outputs"
        );

        let up_to_date = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim stage1 up-to-date");
        assert_eq!(up_to_date, Stage1JobClaimOutcome::SkippedUpToDate);

        let global_job_row_count = sqlx::query("SELECT COUNT(*) AS count FROM jobs WHERE kind = ?")
            .bind("memory_consolidate_global")
            .fetch_one(runtime.pool.as_ref())
            .await
            .expect("load phase2 job row count")
            .try_get::<i64, _>("count")
            .expect("phase2 job row count");
        assert_eq!(
            global_job_row_count, 0,
            "no-output without an existing stage1 output should not enqueue phase2"
        );

        let claim_phase2 = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        assert_eq!(
            claim_phase2,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "phase2 should remain clean when no-output deleted nothing"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_stage1_job_succeeded_no_output_enqueues_phase2_when_deleting_output() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let first_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim initial stage1");
        let first_token = match first_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected initial stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_id, first_token.as_str(), 100, "raw", "sum", None)
                .await
                .expect("mark initial stage1 succeeded"),
            "initial stage1 success should create stage1 output"
        );

        let phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 after initial output");
        let (phase2_token, phase2_input_watermark) = match phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim after initial output: {other:?}"),
        };
        assert_eq!(phase2_input_watermark, 100);
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    phase2_token.as_str(),
                    phase2_input_watermark,
                    &[],
                )
                .await
                .expect("mark initial phase2 succeeded"),
            "initial phase2 success should clear global dirty state"
        );

        let no_output_claim = runtime
            .try_claim_stage1_job(thread_id, owner_b, 101, 3600, 64)
            .await
            .expect("claim stage1 for no-output delete");
        let no_output_token = match no_output_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected no-output stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded_no_output(thread_id, no_output_token.as_str())
                .await
                .expect("mark stage1 no-output after existing output"),
            "no-output should succeed when deleting an existing stage1 output"
        );

        let output_row_count =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("load stage1 output count after delete")
                .try_get::<i64, _>("count")
                .expect("stage1 output count");
        assert_eq!(output_row_count, 0);

        let claim_phase2 = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 after no-output deletion");
        let (phase2_token, phase2_input_watermark) = match claim_phase2 {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim after no-output deletion: {other:?}"),
        };
        assert_eq!(phase2_input_watermark, 101);
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    phase2_token.as_str(),
                    phase2_input_watermark,
                    &[],
                )
                .await
                .expect("mark phase2 succeeded after no-output delete")
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_retry_exhaustion_does_not_block_newer_watermark() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        for attempt in 0..3 {
            let claim = runtime
                .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
                .await
                .expect("claim stage1 for retry exhaustion");
            let ownership_token = match claim {
                Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!(
                    "attempt {} should claim stage1 before retries are exhausted: {other:?}",
                    attempt + 1
                ),
            };
            assert!(
                runtime
                    .mark_stage1_job_failed(thread_id, ownership_token.as_str(), "boom", 0)
                    .await
                    .expect("mark stage1 failed"),
                "attempt {} should decrement retry budget",
                attempt + 1
            );
        }

        let exhausted_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
            .await
            .expect("claim stage1 after retry exhaustion");
        assert_eq!(
            exhausted_claim,
            Stage1JobClaimOutcome::SkippedRetryExhausted
        );

        let newer_source_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 101, 3_600, 64)
            .await
            .expect("claim stage1 with newer source watermark");
        assert!(
            matches!(newer_source_claim, Stage1JobClaimOutcome::Claimed { .. }),
            "newer source watermark should reset retry budget and be claimable"
        );

        let job_row = sqlx::query(
            "SELECT retry_remaining, input_watermark FROM jobs WHERE kind = ? AND job_key = ?",
        )
        .bind("memory_stage1")
        .bind(thread_id.to_string())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("load stage1 job row after newer-source claim");
        assert_eq!(
            job_row
                .try_get::<i64, _>("retry_remaining")
                .expect("retry_remaining"),
            3
        );
        assert_eq!(
            job_row
                .try_get::<i64, _>("input_watermark")
                .expect("input_watermark"),
            101
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_consolidation_reruns_when_watermark_advances() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        runtime
            .enqueue_global_consolidation(100)
            .await
            .expect("enqueue global consolidation");

        let claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (ownership_token, input_watermark) = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(ownership_token.as_str(), input_watermark, &[],)
                .await
                .expect("mark phase2 succeeded"),
            "phase2 success should finalize for current token"
        );

        let claim_up_to_date = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 up-to-date");
        assert_eq!(claim_up_to_date, Phase2JobClaimOutcome::SkippedNotDirty);

        runtime
            .enqueue_global_consolidation(101)
            .await
            .expect("enqueue global consolidation again");

        let claim_rerun = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 rerun");
        assert!(
            matches!(claim_rerun, Phase2JobClaimOutcome::Claimed { .. }),
            "advanced watermark should be claimable"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn list_stage1_outputs_for_global_returns_latest_outputs() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let claim = runtime
            .try_claim_stage1_job(thread_id_a, owner, 100, 3600, 64)
            .await
            .expect("claim stage1 a");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id_a,
                    ownership_token.as_str(),
                    100,
                    "raw memory a",
                    "summary a",
                    None,
                )
                .await
                .expect("mark stage1 succeeded a"),
            "stage1 success should persist output a"
        );

        let claim = runtime
            .try_claim_stage1_job(thread_id_b, owner, 101, 3600, 64)
            .await
            .expect("claim stage1 b");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id_b,
                    ownership_token.as_str(),
                    101,
                    "raw memory b",
                    "summary b",
                    Some("rollout-b"),
                )
                .await
                .expect("mark stage1 succeeded b"),
            "stage1 success should persist output b"
        );

        let outputs = runtime
            .list_stage1_outputs_for_global(10)
            .await
            .expect("list stage1 outputs for global");
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].thread_id, thread_id_b);
        assert_eq!(outputs[0].rollout_summary, "summary b");
        assert_eq!(outputs[0].rollout_slug.as_deref(), Some("rollout-b"));
        assert_eq!(outputs[0].cwd, codex_home.join("workspace-b"));
        assert_eq!(outputs[1].thread_id, thread_id_a);
        assert_eq!(outputs[1].rollout_summary, "summary a");
        assert_eq!(outputs[1].rollout_slug, None);
        assert_eq!(outputs[1].cwd, codex_home.join("workspace-a"));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn list_stage1_outputs_for_global_skips_empty_payloads() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_non_empty =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_empty =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_non_empty,
                codex_home.join("workspace-non-empty"),
            ))
            .await
            .expect("upsert non-empty thread");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_empty,
                codex_home.join("workspace-empty"),
            ))
            .await
            .expect("upsert empty thread");

        sqlx::query(
            r#"
INSERT INTO stage1_outputs (thread_id, source_updated_at, raw_memory, rollout_summary, generated_at)
VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id_non_empty.to_string())
        .bind(100_i64)
        .bind("raw memory")
        .bind("summary")
        .bind(100_i64)
        .execute(runtime.pool.as_ref())
        .await
        .expect("insert non-empty stage1 output");
        sqlx::query(
            r#"
INSERT INTO stage1_outputs (thread_id, source_updated_at, raw_memory, rollout_summary, generated_at)
VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id_empty.to_string())
        .bind(101_i64)
        .bind("")
        .bind("")
        .bind(101_i64)
        .execute(runtime.pool.as_ref())
        .await
        .expect("insert empty stage1 output");

        let outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list stage1 outputs for global");
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].thread_id, thread_id_non_empty);
        assert_eq!(outputs[0].rollout_summary, "summary");
        assert_eq!(outputs[0].cwd, codex_home.join("workspace-non-empty"));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_phase2_input_selection_reports_added_retained_and_removed_rows() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_c = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        for (thread_id, workspace) in [
            (thread_id_a, "workspace-a"),
            (thread_id_b, "workspace-b"),
            (thread_id_c, "workspace-c"),
        ] {
            runtime
                .upsert_thread(&test_thread_metadata(
                    &codex_home,
                    thread_id,
                    codex_home.join(workspace),
                ))
                .await
                .expect("upsert thread");
        }

        for (thread_id, updated_at, slug) in [
            (thread_id_a, 100, Some("rollout-a")),
            (thread_id_b, 101, Some("rollout-b")),
            (thread_id_c, 102, Some("rollout-c")),
        ] {
            let claim = runtime
                .try_claim_stage1_job(thread_id, owner, updated_at, 3600, 64)
                .await
                .expect("claim stage1");
            let ownership_token = match claim {
                Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!("unexpected stage1 claim outcome: {other:?}"),
            };
            assert!(
                runtime
                    .mark_stage1_job_succeeded(
                        thread_id,
                        ownership_token.as_str(),
                        updated_at,
                        &format!("raw-{updated_at}"),
                        &format!("summary-{updated_at}"),
                        slug,
                    )
                    .await
                    .expect("mark stage1 succeeded"),
                "stage1 success should persist output"
            );
        }

        let claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (ownership_token, input_watermark) = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        assert_eq!(input_watermark, 102);
        let selected_outputs = runtime
            .list_stage1_outputs_for_global(10)
            .await
            .expect("list stage1 outputs for global")
            .into_iter()
            .filter(|output| output.thread_id == thread_id_c || output.thread_id == thread_id_a)
            .collect::<Vec<_>>();
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    ownership_token.as_str(),
                    input_watermark,
                    &selected_outputs,
                )
                .await
                .expect("mark phase2 success with selection"),
            "phase2 success should persist selected rows"
        );

        let selection = runtime
            .get_phase2_input_selection(2)
            .await
            .expect("load phase2 input selection");

        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.previous_selected.len(), 2);
        assert_eq!(selection.selected[0].thread_id, thread_id_c);
        assert_eq!(
            selection.selected[0].rollout_path,
            codex_home.join(format!("rollout-{thread_id_c}.jsonl"))
        );
        assert_eq!(selection.selected[1].thread_id, thread_id_b);
        assert_eq!(selection.retained_thread_ids, vec![thread_id_c]);

        assert_eq!(selection.removed.len(), 1);
        assert_eq!(selection.removed[0].thread_id, thread_id_a);
        assert_eq!(
            selection.removed[0].rollout_slug.as_deref(),
            Some("rollout-a")
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_phase2_input_selection_treats_regenerated_selected_rows_as_added() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let first_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim initial stage1");
        let first_token = match first_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    first_token.as_str(),
                    100,
                    "raw-100",
                    "summary-100",
                    Some("rollout-100"),
                )
                .await
                .expect("mark initial stage1 success"),
            "initial stage1 success should persist output"
        );

        let phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (phase2_token, input_watermark) = match phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        let selected_outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list selected outputs");
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    phase2_token.as_str(),
                    input_watermark,
                    &selected_outputs,
                )
                .await
                .expect("mark phase2 success"),
            "phase2 success should persist selected rows"
        );

        let refreshed_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 101, 3600, 64)
            .await
            .expect("claim refreshed stage1");
        let refreshed_token = match refreshed_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    refreshed_token.as_str(),
                    101,
                    "raw-101",
                    "summary-101",
                    Some("rollout-101"),
                )
                .await
                .expect("mark refreshed stage1 success"),
            "refreshed stage1 success should persist output"
        );

        let selection = runtime
            .get_phase2_input_selection(1)
            .await
            .expect("load phase2 input selection");
        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.previous_selected.len(), 1);
        assert_eq!(selection.selected[0].thread_id, thread_id);
        assert_eq!(selection.selected[0].source_updated_at.timestamp(), 101);
        assert!(selection.retained_thread_ids.is_empty());
        assert!(selection.removed.is_empty());

        let (selected_for_phase2, selected_for_phase2_source_updated_at) =
            sqlx::query_as::<_, (i64, Option<i64>)>(
                "SELECT selected_for_phase2, selected_for_phase2_source_updated_at FROM stage1_outputs WHERE thread_id = ?",
            )
        .bind(thread_id.to_string())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("load selected_for_phase2");
        assert_eq!(selected_for_phase2, 1);
        assert_eq!(selected_for_phase2_source_updated_at, Some(100));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_phase2_input_selection_reports_regenerated_previous_selection_as_removed() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread a");
        let thread_id_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread b");
        let thread_id_c = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread c");
        let thread_id_d = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread d");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        for (thread_id, workspace) in [
            (thread_id_a, "workspace-a"),
            (thread_id_b, "workspace-b"),
            (thread_id_c, "workspace-c"),
            (thread_id_d, "workspace-d"),
        ] {
            runtime
                .upsert_thread(&test_thread_metadata(
                    &codex_home,
                    thread_id,
                    codex_home.join(workspace),
                ))
                .await
                .expect("upsert thread");
        }

        for (thread_id, updated_at, slug) in [
            (thread_id_a, 100, Some("rollout-a-100")),
            (thread_id_b, 101, Some("rollout-b-101")),
            (thread_id_c, 99, Some("rollout-c-99")),
            (thread_id_d, 98, Some("rollout-d-98")),
        ] {
            let claim = runtime
                .try_claim_stage1_job(thread_id, owner, updated_at, 3600, 64)
                .await
                .expect("claim initial stage1");
            let ownership_token = match claim {
                Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!("unexpected stage1 claim outcome: {other:?}"),
            };
            assert!(
                runtime
                    .mark_stage1_job_succeeded(
                        thread_id,
                        ownership_token.as_str(),
                        updated_at,
                        &format!("raw-{updated_at}"),
                        &format!("summary-{updated_at}"),
                        slug,
                    )
                    .await
                    .expect("mark stage1 succeeded"),
                "stage1 success should persist output"
            );
        }

        let phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (phase2_token, input_watermark) = match phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        let selected_outputs = runtime
            .list_stage1_outputs_for_global(2)
            .await
            .expect("list selected outputs");
        assert_eq!(
            selected_outputs
                .iter()
                .map(|output| output.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_id_b, thread_id_a]
        );
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    phase2_token.as_str(),
                    input_watermark,
                    &selected_outputs,
                )
                .await
                .expect("mark phase2 success"),
            "phase2 success should persist selected rows"
        );

        for (thread_id, updated_at, slug) in [
            (thread_id_a, 102, Some("rollout-a-102")),
            (thread_id_c, 103, Some("rollout-c-103")),
            (thread_id_d, 104, Some("rollout-d-104")),
        ] {
            let claim = runtime
                .try_claim_stage1_job(thread_id, owner, updated_at, 3600, 64)
                .await
                .expect("claim refreshed stage1");
            let ownership_token = match claim {
                Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!("unexpected stage1 claim outcome: {other:?}"),
            };
            assert!(
                runtime
                    .mark_stage1_job_succeeded(
                        thread_id,
                        ownership_token.as_str(),
                        updated_at,
                        &format!("raw-{updated_at}"),
                        &format!("summary-{updated_at}"),
                        slug,
                    )
                    .await
                    .expect("mark refreshed stage1 success"),
                "refreshed stage1 success should persist output"
            );
        }

        let selection = runtime
            .get_phase2_input_selection(2)
            .await
            .expect("load phase2 input selection");
        assert_eq!(
            selection
                .selected
                .iter()
                .map(|output| output.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_id_d, thread_id_c]
        );
        assert_eq!(
            selection
                .previous_selected
                .iter()
                .map(|output| output.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_id_a, thread_id_b]
        );
        assert!(selection.retained_thread_ids.is_empty());
        assert_eq!(
            selection
                .removed
                .iter()
                .map(|output| (output.thread_id, output.source_updated_at.timestamp()))
                .collect::<Vec<_>>(),
            vec![(thread_id_a, 102), (thread_id_b, 101)]
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_global_phase2_job_succeeded_updates_selected_snapshot_timestamp() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let initial_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim initial stage1");
        let initial_token = match initial_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    initial_token.as_str(),
                    100,
                    "raw-100",
                    "summary-100",
                    Some("rollout-100"),
                )
                .await
                .expect("mark initial stage1 success"),
            "initial stage1 success should persist output"
        );

        let first_phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim first phase2");
        let (first_phase2_token, first_input_watermark) = match first_phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected first phase2 claim outcome: {other:?}"),
        };
        let first_selected_outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list first selected outputs");
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    first_phase2_token.as_str(),
                    first_input_watermark,
                    &first_selected_outputs,
                )
                .await
                .expect("mark first phase2 success"),
            "first phase2 success should persist selected rows"
        );

        let refreshed_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 101, 3600, 64)
            .await
            .expect("claim refreshed stage1");
        let refreshed_token = match refreshed_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected refreshed stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    refreshed_token.as_str(),
                    101,
                    "raw-101",
                    "summary-101",
                    Some("rollout-101"),
                )
                .await
                .expect("mark refreshed stage1 success"),
            "refreshed stage1 success should persist output"
        );

        let second_phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim second phase2");
        let (second_phase2_token, second_input_watermark) = match second_phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected second phase2 claim outcome: {other:?}"),
        };
        let second_selected_outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list second selected outputs");
        assert_eq!(
            second_selected_outputs[0].source_updated_at.timestamp(),
            101
        );
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    second_phase2_token.as_str(),
                    second_input_watermark,
                    &second_selected_outputs,
                )
                .await
                .expect("mark second phase2 success"),
            "second phase2 success should persist selected rows"
        );

        let selection = runtime
            .get_phase2_input_selection(1)
            .await
            .expect("load phase2 input selection after refresh");
        assert_eq!(selection.retained_thread_ids, vec![thread_id]);

        let (selected_for_phase2, selected_for_phase2_source_updated_at) =
            sqlx::query_as::<_, (i64, Option<i64>)>(
                "SELECT selected_for_phase2, selected_for_phase2_source_updated_at FROM stage1_outputs WHERE thread_id = ?",
            )
            .bind(thread_id.to_string())
            .fetch_one(runtime.pool.as_ref())
            .await
            .expect("load selected snapshot after phase2");
        assert_eq!(selected_for_phase2, 1);
        assert_eq!(selected_for_phase2_source_updated_at, Some(101));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_global_phase2_job_succeeded_only_marks_exact_selected_snapshots() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let initial_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim initial stage1");
        let initial_token = match initial_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    initial_token.as_str(),
                    100,
                    "raw-100",
                    "summary-100",
                    Some("rollout-100"),
                )
                .await
                .expect("mark initial stage1 success"),
            "initial stage1 success should persist output"
        );

        let phase2_claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (phase2_token, input_watermark) = match phase2_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        let selected_outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list selected outputs");
        assert_eq!(selected_outputs[0].source_updated_at.timestamp(), 100);

        let refreshed_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 101, 3600, 64)
            .await
            .expect("claim refreshed stage1");
        let refreshed_token = match refreshed_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id,
                    refreshed_token.as_str(),
                    101,
                    "raw-101",
                    "summary-101",
                    Some("rollout-101"),
                )
                .await
                .expect("mark refreshed stage1 success"),
            "refreshed stage1 success should persist output"
        );

        assert!(
            runtime
                .mark_global_phase2_job_succeeded(
                    phase2_token.as_str(),
                    input_watermark,
                    &selected_outputs,
                )
                .await
                .expect("mark phase2 success"),
            "phase2 success should still complete"
        );

        let (selected_for_phase2, selected_for_phase2_source_updated_at) =
            sqlx::query_as::<_, (i64, Option<i64>)>(
                "SELECT selected_for_phase2, selected_for_phase2_source_updated_at FROM stage1_outputs WHERE thread_id = ?",
            )
            .bind(thread_id.to_string())
            .fetch_one(runtime.pool.as_ref())
            .await
            .expect("load selected_for_phase2");
        assert_eq!(selected_for_phase2, 0);
        assert_eq!(selected_for_phase2_source_updated_at, None);

        let selection = runtime
            .get_phase2_input_selection(1)
            .await
            .expect("load phase2 input selection");
        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.selected[0].source_updated_at.timestamp(), 101);
        assert!(selection.retained_thread_ids.is_empty());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn record_stage1_output_usage_updates_usage_metadata() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id a");
        let thread_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id b");
        let missing = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("missing id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let claim_a = runtime
            .try_claim_stage1_job(thread_a, owner, 100, 3600, 64)
            .await
            .expect("claim stage1 a");
        let token_a = match claim_a {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for a: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_a, token_a.as_str(), 100, "raw a", "sum a", None)
                .await
                .expect("mark stage1 succeeded a")
        );

        let claim_b = runtime
            .try_claim_stage1_job(thread_b, owner, 101, 3600, 64)
            .await
            .expect("claim stage1 b");
        let token_b = match claim_b {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for b: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_b, token_b.as_str(), 101, "raw b", "sum b", None)
                .await
                .expect("mark stage1 succeeded b")
        );

        let updated_rows = runtime
            .record_stage1_output_usage(&[thread_a, thread_a, thread_b, missing])
            .await
            .expect("record stage1 output usage");
        assert_eq!(updated_rows, 3);

        let row_a =
            sqlx::query("SELECT usage_count, last_usage FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_a.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("load stage1 usage row a");
        let row_b =
            sqlx::query("SELECT usage_count, last_usage FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_b.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("load stage1 usage row b");

        assert_eq!(
            row_a
                .try_get::<i64, _>("usage_count")
                .expect("usage_count a"),
            2
        );
        assert_eq!(
            row_b
                .try_get::<i64, _>("usage_count")
                .expect("usage_count b"),
            1
        );

        let last_usage_a = row_a.try_get::<i64, _>("last_usage").expect("last_usage a");
        let last_usage_b = row_b.try_get::<i64, _>("last_usage").expect("last_usage b");
        assert_eq!(last_usage_a, last_usage_b);
        assert!(last_usage_a > 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_stage1_job_succeeded_enqueues_global_consolidation() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id a");
        let thread_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id b");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let claim_a = runtime
            .try_claim_stage1_job(thread_a, owner, 100, 3600, 64)
            .await
            .expect("claim stage1 a");
        let token_a = match claim_a {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for thread a: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_a,
                    token_a.as_str(),
                    100,
                    "raw-a",
                    "summary-a",
                    None,
                )
                .await
                .expect("mark stage1 succeeded a"),
            "stage1 success should persist output for thread a"
        );

        let claim_b = runtime
            .try_claim_stage1_job(thread_b, owner, 101, 3600, 64)
            .await
            .expect("claim stage1 b");
        let token_b = match claim_b {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for thread b: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_b,
                    token_b.as_str(),
                    101,
                    "raw-b",
                    "summary-b",
                    None,
                )
                .await
                .expect("mark stage1 succeeded b"),
            "stage1 success should persist output for thread b"
        );

        let claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim global consolidation");
        let input_watermark = match claim {
            Phase2JobClaimOutcome::Claimed {
                input_watermark, ..
            } => input_watermark,
            other => panic!("unexpected global consolidation claim outcome: {other:?}"),
        };
        assert_eq!(input_watermark, 101);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_lock_allows_only_one_fresh_runner() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(200)
            .await
            .expect("enqueue global consolidation");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");

        let running_claim = runtime
            .try_claim_global_phase2_job(owner_a, 3600)
            .await
            .expect("claim global lock");
        assert!(
            matches!(running_claim, Phase2JobClaimOutcome::Claimed { .. }),
            "first owner should claim global lock"
        );

        let second_claim = runtime
            .try_claim_global_phase2_job(owner_b, 3600)
            .await
            .expect("claim global lock from second owner");
        assert_eq!(second_claim, Phase2JobClaimOutcome::SkippedRunning);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_lock_stale_lease_allows_takeover() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(300)
            .await
            .expect("enqueue global consolidation");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");

        let initial_claim = runtime
            .try_claim_global_phase2_job(owner_a, 3600)
            .await
            .expect("claim initial global lock");
        let token_a = match initial_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected initial claim outcome: {other:?}"),
        };

        sqlx::query("UPDATE jobs SET lease_until = ? WHERE kind = ? AND job_key = ?")
            .bind(Utc::now().timestamp() - 1)
            .bind("memory_consolidate_global")
            .bind("global")
            .execute(runtime.pool.as_ref())
            .await
            .expect("expire global consolidation lease");

        let takeover_claim = runtime
            .try_claim_global_phase2_job(owner_b, 3600)
            .await
            .expect("claim stale global lock");
        let (token_b, input_watermark) = match takeover_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected takeover claim outcome: {other:?}"),
        };
        assert_ne!(token_a, token_b);
        assert_eq!(input_watermark, 300);

        assert_eq!(
            runtime
                .mark_global_phase2_job_succeeded(token_a.as_str(), 300, &[])
                .await
                .expect("mark stale owner success result"),
            false,
            "stale owner should lose finalization ownership after takeover"
        );
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(token_b.as_str(), 300, &[])
                .await
                .expect("mark takeover owner success"),
            "takeover owner should finalize consolidation"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_backfilled_inputs_below_last_success_still_become_dirty() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(500)
            .await
            .expect("enqueue initial consolidation");
        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let claim_a = runtime
            .try_claim_global_phase2_job(owner_a, 3_600)
            .await
            .expect("claim initial consolidation");
        let token_a = match claim_a {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => {
                assert_eq!(input_watermark, 500);
                ownership_token
            }
            other => panic!("unexpected initial phase2 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(token_a.as_str(), 500, &[])
                .await
                .expect("mark initial phase2 success"),
            "initial phase2 success should finalize"
        );

        runtime
            .enqueue_global_consolidation(400)
            .await
            .expect("enqueue backfilled consolidation");

        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");
        let claim_b = runtime
            .try_claim_global_phase2_job(owner_b, 3_600)
            .await
            .expect("claim backfilled consolidation");
        match claim_b {
            Phase2JobClaimOutcome::Claimed {
                input_watermark, ..
            } => {
                assert!(
                    input_watermark > 500,
                    "backfilled enqueue should advance dirty watermark beyond last success"
                );
            }
            other => panic!("unexpected backfilled phase2 claim outcome: {other:?}"),
        }

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_failure_fallback_updates_unowned_running_job() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(400)
            .await
            .expect("enqueue global consolidation");

        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner");
        let claim = runtime
            .try_claim_global_phase2_job(owner, 3_600)
            .await
            .expect("claim global consolidation");
        let ownership_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };

        sqlx::query("UPDATE jobs SET ownership_token = NULL WHERE kind = ? AND job_key = ?")
            .bind("memory_consolidate_global")
            .bind("global")
            .execute(runtime.pool.as_ref())
            .await
            .expect("clear ownership token");

        assert_eq!(
            runtime
                .mark_global_phase2_job_failed(ownership_token.as_str(), "lost", 3_600)
                .await
                .expect("mark phase2 failed with strict ownership"),
            false,
            "strict failure update should not match unowned running job"
        );
        assert!(
            runtime
                .mark_global_phase2_job_failed_if_unowned(ownership_token.as_str(), "lost", 3_600)
                .await
                .expect("fallback failure update should match unowned running job"),
            "fallback failure update should transition the unowned running job"
        );

        let claim = runtime
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after fallback failure");
        assert_eq!(claim, Phase2JobClaimOutcome::SkippedNotDirty);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
