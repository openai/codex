use super::*;
use crate::model::AgentJobItemRow;
use crate::model::AgentJobKind;
use crate::model::EnqueueAgentJobItemOutcome;
use crate::model::EnqueueAgentJobItemsResult;

impl StateRuntime {
    pub async fn create_agent_job(
        &self,
        params: &AgentJobCreateParams,
        items: &[AgentJobItemCreateParams],
    ) -> anyhow::Result<AgentJob> {
        let now = Utc::now().timestamp();
        let input_headers_json = serde_json::to_string(&params.input_headers)?;
        let output_schema_json = params
            .output_schema_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let max_items = params
            .max_items
            .map(i64::try_from)
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid max_items value"))?;
        let max_runtime_seconds = params
            .max_runtime_seconds
            .map(i64::try_from)
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid max_runtime_seconds value"))?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
INSERT INTO agent_jobs (
    id,
    name,
    kind,
    status,
    instruction,
    auto_export,
    max_items,
    max_runtime_seconds,
    output_schema_json,
    input_headers_json,
    input_csv_path,
    output_csv_path,
    created_at,
    updated_at,
    started_at,
    completed_at,
    last_error
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
            "#,
        )
        .bind(params.id.as_str())
        .bind(params.name.as_str())
        .bind(params.kind.as_str())
        .bind(AgentJobStatus::Pending.as_str())
        .bind(params.instruction.as_str())
        .bind(i64::from(params.auto_export))
        .bind(max_items)
        .bind(max_runtime_seconds)
        .bind(output_schema_json)
        .bind(input_headers_json)
        .bind(params.input_csv_path.as_str())
        .bind(params.output_csv_path.as_str())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        for item in items {
            let row_json = serde_json::to_string(&item.row_json)?;
            sqlx::query(
                r#"
INSERT INTO agent_job_items (
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, 0, NULL, NULL, ?, ?, NULL, NULL)
                "#,
            )
            .bind(params.id.as_str())
            .bind(item.item_id.as_str())
            .bind(item.parent_item_id.as_deref())
            .bind(item.row_index)
            .bind(item.source_id.as_deref())
            .bind(item.dedupe_key.as_deref())
            .bind(row_json)
            .bind(AgentJobItemStatus::Pending.as_str())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        let job_id = params.id.as_str();
        self.get_agent_job(job_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to load created agent job {job_id}"))
    }

    pub async fn get_agent_job(&self, job_id: &str) -> anyhow::Result<Option<AgentJob>> {
        let row = sqlx::query_as::<_, AgentJobRow>(
            r#"
SELECT
    id,
    name,
    kind,
    status,
    instruction,
    auto_export,
    max_items,
    max_runtime_seconds,
    output_schema_json,
    input_headers_json,
    input_csv_path,
    output_csv_path,
    created_at,
    updated_at,
    started_at,
    completed_at,
    last_error
FROM agent_jobs
WHERE id = ?
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(AgentJob::try_from).transpose()
    }

    pub async fn list_agent_job_items(
        &self,
        job_id: &str,
        status: Option<AgentJobItemStatus>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<AgentJobItem>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
SELECT
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
FROM agent_job_items
WHERE job_id = 
            "#,
        );
        builder.push_bind(job_id);
        if let Some(status) = status {
            builder.push(" AND status = ");
            builder.push_bind(status.as_str());
        }
        builder.push(" ORDER BY row_index ASC");
        if let Some(limit) = limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit as i64);
        }
        let rows: Vec<AgentJobItemRow> = builder
            .build_query_as::<AgentJobItemRow>()
            .fetch_all(self.pool.as_ref())
            .await?;
        rows.into_iter().map(AgentJobItem::try_from).collect()
    }

    pub async fn get_agent_job_item(
        &self,
        job_id: &str,
        item_id: &str,
    ) -> anyhow::Result<Option<AgentJobItem>> {
        let row: Option<AgentJobItemRow> = sqlx::query_as::<_, AgentJobItemRow>(
            r#"
SELECT
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
FROM agent_job_items
WHERE job_id = ? AND item_id = ?
            "#,
        )
        .bind(job_id)
        .bind(item_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(AgentJobItem::try_from).transpose()
    }

    pub async fn enqueue_agent_job_items(
        &self,
        job_id: &str,
        parent_item_id: &str,
        reporting_thread_id: &str,
        items: &[AgentJobItemCreateParams],
    ) -> anyhow::Result<EnqueueAgentJobItemsResult> {
        let now = Utc::now().timestamp();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        let job_row = sqlx::query_as::<_, AgentJobRow>(
            r#"
SELECT
    id,
    name,
    kind,
    status,
    instruction,
    auto_export,
    max_items,
    max_runtime_seconds,
    output_schema_json,
    input_headers_json,
    input_csv_path,
    output_csv_path,
    created_at,
    updated_at,
    started_at,
    completed_at,
    last_error
FROM agent_jobs
WHERE id = ?
            "#,
        )
        .bind(job_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent job {job_id} not found"))?;
        let job = AgentJob::try_from(job_row)?;

        if job.kind != AgentJobKind::DynamicQueue {
            return Err(anyhow::anyhow!(
                "agent job {job_id} does not support queue item enqueue"
            ));
        }
        if job.status.is_final() {
            return Err(anyhow::anyhow!(
                "agent job {job_id} is already {}",
                job.status.as_str()
            ));
        }

        let parent_owner = sqlx::query(
            r#"
SELECT 1
FROM agent_job_items
WHERE
    job_id = ?
    AND item_id = ?
    AND status = ?
    AND assigned_thread_id = ?
            "#,
        )
        .bind(job_id)
        .bind(parent_item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .bind(reporting_thread_id)
        .fetch_optional(&mut *tx)
        .await?;
        if parent_owner.is_none() {
            return Err(anyhow::anyhow!(
                "agent job parent item {parent_item_id} is not owned by thread {reporting_thread_id}"
            ));
        }

        let mut total_items: i64 = sqlx::query_scalar(
            r#"
SELECT COUNT(*)
FROM agent_job_items
WHERE job_id = ?
            "#,
        )
        .bind(job_id)
        .fetch_one(&mut *tx)
        .await?;
        let mut next_row_index: i64 = sqlx::query_scalar(
            r#"
SELECT COALESCE(MAX(row_index), -1)
FROM agent_job_items
WHERE job_id = ?
            "#,
        )
        .bind(job_id)
        .fetch_one(&mut *tx)
        .await?;
        next_row_index = next_row_index.saturating_add(1);

        let max_items = job
            .max_items
            .map(i64::try_from)
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid max_items value"))?;
        let mut inserted_any = false;
        let mut reserved_item_ids = BTreeSet::new();
        let mut outcomes = Vec::with_capacity(items.len());

        for item in items {
            let resolved_parent_item_id = match item.parent_item_id.as_deref() {
                Some(item_parent_item_id) if item_parent_item_id != parent_item_id => {
                    return Err(anyhow::anyhow!(
                        "enqueue item {} parent_item_id mismatch: expected {parent_item_id}, got {item_parent_item_id}",
                        item.item_id
                    ));
                }
                Some(_) | None => Some(parent_item_id.to_string()),
            };

            if let Some(dedupe_key) = item.dedupe_key.as_deref() {
                let existing_row = sqlx::query_as::<_, AgentJobItemRow>(
                    r#"
SELECT
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
FROM agent_job_items
WHERE job_id = ? AND dedupe_key = ?
                    "#,
                )
                .bind(job_id)
                .bind(dedupe_key)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(existing_row) = existing_row {
                    outcomes.push(EnqueueAgentJobItemOutcome::Deduped {
                        item: AgentJobItem::try_from(existing_row)?,
                    });
                    continue;
                }
            }

            if max_items.is_some_and(|max_items| total_items >= max_items) {
                outcomes.push(EnqueueAgentJobItemOutcome::MaxItemsReached {
                    requested_item_id: item.item_id.clone(),
                    parent_item_id: resolved_parent_item_id,
                    dedupe_key: item.dedupe_key.clone(),
                });
                continue;
            }

            let base_item_id = item.item_id.as_str();
            let mut resolved_item_id = base_item_id.to_string();
            let mut suffix = 2usize;
            loop {
                if reserved_item_ids.contains(resolved_item_id.as_str()) {
                    resolved_item_id = format!("{base_item_id}-{suffix}");
                    suffix = suffix.saturating_add(1);
                    continue;
                }

                let existing_item = sqlx::query(
                    r#"
SELECT 1
FROM agent_job_items
WHERE job_id = ? AND item_id = ?
                    "#,
                )
                .bind(job_id)
                .bind(resolved_item_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
                if existing_item.is_none() {
                    break;
                }

                resolved_item_id = format!("{base_item_id}-{suffix}");
                suffix = suffix.saturating_add(1);
            }
            reserved_item_ids.insert(resolved_item_id.clone());

            let row_json = serde_json::to_string(&item.row_json)?;
            sqlx::query(
                r#"
INSERT INTO agent_job_items (
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, 0, NULL, NULL, ?, ?, NULL, NULL)
                "#,
            )
            .bind(job_id)
            .bind(resolved_item_id.as_str())
            .bind(resolved_parent_item_id.as_deref())
            .bind(next_row_index)
            .bind(item.source_id.as_deref())
            .bind(item.dedupe_key.as_deref())
            .bind(row_json)
            .bind(AgentJobItemStatus::Pending.as_str())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            let inserted_row = sqlx::query_as::<_, AgentJobItemRow>(
                r#"
SELECT
    job_id,
    item_id,
    parent_item_id,
    row_index,
    source_id,
    dedupe_key,
    row_json,
    status,
    assigned_thread_id,
    attempt_count,
    result_json,
    last_error,
    created_at,
    updated_at,
    completed_at,
    reported_at
FROM agent_job_items
WHERE job_id = ? AND item_id = ?
                "#,
            )
            .bind(job_id)
            .bind(resolved_item_id.as_str())
            .fetch_one(&mut *tx)
            .await?;
            outcomes.push(EnqueueAgentJobItemOutcome::Enqueued {
                item: AgentJobItem::try_from(inserted_row)?,
            });

            inserted_any = true;
            total_items = total_items.saturating_add(1);
            next_row_index = next_row_index.saturating_add(1);
        }

        if inserted_any {
            sqlx::query(
                r#"
UPDATE agent_jobs
SET updated_at = ?
WHERE id = ?
                "#,
            )
            .bind(now)
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(EnqueueAgentJobItemsResult { outcomes })
    }

    pub async fn mark_agent_job_running(&self, job_id: &str) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
UPDATE agent_jobs
SET
    status = ?,
    updated_at = ?,
    started_at = COALESCE(started_at, ?),
    completed_at = NULL,
    last_error = NULL
WHERE id = ?
            "#,
        )
        .bind(AgentJobStatus::Running.as_str())
        .bind(now)
        .bind(now)
        .bind(job_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn mark_agent_job_completed(&self, job_id: &str) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
UPDATE agent_jobs
SET status = ?, updated_at = ?, completed_at = ?, last_error = NULL
WHERE id = ?
            "#,
        )
        .bind(AgentJobStatus::Completed.as_str())
        .bind(now)
        .bind(now)
        .bind(job_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn mark_agent_job_failed(
        &self,
        job_id: &str,
        error_message: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
UPDATE agent_jobs
SET status = ?, updated_at = ?, completed_at = ?, last_error = ?
WHERE id = ?
            "#,
        )
        .bind(AgentJobStatus::Failed.as_str())
        .bind(now)
        .bind(now)
        .bind(error_message)
        .bind(job_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn mark_agent_job_cancelled(
        &self,
        job_id: &str,
        reason: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_jobs
SET status = ?, updated_at = ?, completed_at = ?, last_error = ?
WHERE id = ? AND status IN (?, ?)
            "#,
        )
        .bind(AgentJobStatus::Cancelled.as_str())
        .bind(now)
        .bind(now)
        .bind(reason)
        .bind(job_id)
        .bind(AgentJobStatus::Pending.as_str())
        .bind(AgentJobStatus::Running.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn is_agent_job_cancelled(&self, job_id: &str) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
SELECT status
FROM agent_jobs
WHERE id = ?
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let status: String = row.try_get("status")?;
        Ok(AgentJobStatus::parse(status.as_str())? == AgentJobStatus::Cancelled)
    }

    pub async fn mark_agent_job_item_running(
        &self,
        job_id: &str,
        item_id: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    status = ?,
    assigned_thread_id = NULL,
    attempt_count = attempt_count + 1,
    updated_at = ?,
    last_error = NULL
WHERE job_id = ? AND item_id = ? AND status = ?
            "#,
        )
        .bind(AgentJobItemStatus::Running.as_str())
        .bind(now)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Pending.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_agent_job_item_running_with_thread(
        &self,
        job_id: &str,
        item_id: &str,
        thread_id: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    status = ?,
    assigned_thread_id = ?,
    attempt_count = attempt_count + 1,
    updated_at = ?,
    last_error = NULL
WHERE job_id = ? AND item_id = ? AND status = ?
            "#,
        )
        .bind(AgentJobItemStatus::Running.as_str())
        .bind(thread_id)
        .bind(now)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Pending.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_agent_job_item_pending(
        &self,
        job_id: &str,
        item_id: &str,
        error_message: Option<&str>,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    status = ?,
    assigned_thread_id = NULL,
    updated_at = ?,
    last_error = ?
WHERE job_id = ? AND item_id = ? AND status = ?
            "#,
        )
        .bind(AgentJobItemStatus::Pending.as_str())
        .bind(now)
        .bind(error_message)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_agent_job_item_thread(
        &self,
        job_id: &str,
        item_id: &str,
        thread_id: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET assigned_thread_id = ?, updated_at = ?
WHERE job_id = ? AND item_id = ? AND status = ?
            "#,
        )
        .bind(thread_id)
        .bind(now)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn report_agent_job_item_result(
        &self,
        job_id: &str,
        item_id: &str,
        reporting_thread_id: &str,
        result_json: &Value,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let serialized = serde_json::to_string(result_json)?;
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    result_json = ?,
    reported_at = ?,
    updated_at = ?,
    last_error = NULL
WHERE
    job_id = ?
    AND item_id = ?
    AND status = ?
    AND assigned_thread_id = ?
            "#,
        )
        .bind(serialized)
        .bind(now)
        .bind(now)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .bind(reporting_thread_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_agent_job_item_completed(
        &self,
        job_id: &str,
        item_id: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    status = ?,
    completed_at = ?,
    updated_at = ?,
    assigned_thread_id = NULL
WHERE
    job_id = ?
    AND item_id = ?
    AND status = ?
    AND result_json IS NOT NULL
            "#,
        )
        .bind(AgentJobItemStatus::Completed.as_str())
        .bind(now)
        .bind(now)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_agent_job_item_failed(
        &self,
        job_id: &str,
        item_id: &str,
        error_message: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE agent_job_items
SET
    status = ?,
    completed_at = ?,
    updated_at = ?,
    last_error = ?,
    assigned_thread_id = NULL
WHERE
    job_id = ?
    AND item_id = ?
    AND status = ?
            "#,
        )
        .bind(AgentJobItemStatus::Failed.as_str())
        .bind(now)
        .bind(now)
        .bind(error_message)
        .bind(job_id)
        .bind(item_id)
        .bind(AgentJobItemStatus::Running.as_str())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_agent_job_progress(&self, job_id: &str) -> anyhow::Result<AgentJobProgress> {
        let row = sqlx::query(
            r#"
SELECT
    COUNT(*) AS total_items,
    SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS pending_items,
    SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS running_items,
    SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS completed_items,
    SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) AS failed_items
FROM agent_job_items
WHERE job_id = ?
            "#,
        )
        .bind(AgentJobItemStatus::Pending.as_str())
        .bind(AgentJobItemStatus::Running.as_str())
        .bind(AgentJobItemStatus::Completed.as_str())
        .bind(AgentJobItemStatus::Failed.as_str())
        .bind(job_id)
        .fetch_one(self.pool.as_ref())
        .await?;

        let total_items: i64 = row.try_get("total_items")?;
        let pending_items: Option<i64> = row.try_get("pending_items")?;
        let running_items: Option<i64> = row.try_get("running_items")?;
        let completed_items: Option<i64> = row.try_get("completed_items")?;
        let failed_items: Option<i64> = row.try_get("failed_items")?;
        Ok(AgentJobProgress {
            total_items: usize::try_from(total_items).unwrap_or_default(),
            pending_items: usize::try_from(pending_items.unwrap_or_default()).unwrap_or_default(),
            running_items: usize::try_from(running_items.unwrap_or_default()).unwrap_or_default(),
            completed_items: usize::try_from(completed_items.unwrap_or_default())
                .unwrap_or_default(),
            failed_items: usize::try_from(failed_items.unwrap_or_default()).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn queue_job(job_id: &str, max_items: Option<u64>) -> AgentJobCreateParams {
        AgentJobCreateParams {
            id: job_id.to_string(),
            name: format!("queue-job-{job_id}"),
            kind: AgentJobKind::DynamicQueue,
            instruction: "process {value}".to_string(),
            auto_export: false,
            max_items,
            max_runtime_seconds: None,
            output_schema_json: None,
            input_headers: vec!["value".to_string()],
            input_csv_path: "seed.csv".to_string(),
            output_csv_path: "seed-out.csv".to_string(),
        }
    }

    fn item(
        item_id: &str,
        row_index: i64,
        parent_item_id: Option<&str>,
        dedupe_key: Option<&str>,
        value: &str,
    ) -> AgentJobItemCreateParams {
        AgentJobItemCreateParams {
            item_id: item_id.to_string(),
            parent_item_id: parent_item_id.map(str::to_string),
            row_index,
            source_id: Some(format!("source-{item_id}")),
            dedupe_key: dedupe_key.map(str::to_string),
            row_json: json!({"value": value}),
        }
    }

    #[tokio::test]
    async fn create_agent_job_persists_dynamic_queue_fields() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home, "test-provider".to_string())
            .await
            .expect("state db should initialize");

        let job = runtime
            .create_agent_job(
                &queue_job("job-queue-persist", Some(5)),
                &[
                    item("root", 0, None, None, "root"),
                    item("child", 1, Some("root"), Some("child-dedupe"), "child"),
                ],
            )
            .await
            .expect("create agent job should succeed");
        let items = runtime
            .list_agent_job_items(job.id.as_str(), None, None)
            .await
            .expect("list agent job items should succeed");

        assert_eq!(job.kind, AgentJobKind::DynamicQueue);
        assert_eq!(job.max_items, Some(5));
        let item_summary: Vec<_> = items
            .iter()
            .map(|item| {
                (
                    item.item_id.clone(),
                    item.parent_item_id.clone(),
                    item.dedupe_key.clone(),
                    item.row_index,
                )
            })
            .collect();
        assert_eq!(
            item_summary,
            vec![
                ("root".to_string(), None, None, 0),
                (
                    "child".to_string(),
                    Some("root".to_string()),
                    Some("child-dedupe".to_string()),
                    1,
                ),
            ]
        );
    }

    #[tokio::test]
    async fn enqueue_agent_job_items_dedupes_suffixes_and_caps() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home, "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let job = runtime
            .create_agent_job(
                &queue_job("job-queue-enqueue", Some(4)),
                &[
                    item("root", 0, None, None, "root"),
                    item(
                        "existing",
                        1,
                        Some("root"),
                        Some("existing-dedupe"),
                        "existing",
                    ),
                ],
            )
            .await
            .expect("create queue job should succeed");
        assert!(
            runtime
                .mark_agent_job_item_running_with_thread(
                    job.id.as_str(),
                    "root",
                    "thread-owned-parent",
                )
                .await
                .expect("mark parent running should succeed"),
            "parent item should become running"
        );

        let result = runtime
            .enqueue_agent_job_items(
                job.id.as_str(),
                "root",
                "thread-owned-parent",
                &[
                    item("foo", -1, None, Some("new-a"), "foo-a"),
                    item(
                        "ignored",
                        -1,
                        Some("root"),
                        Some("existing-dedupe"),
                        "deduped",
                    ),
                    item("foo", -1, None, Some("new-b"), "foo-b"),
                    item("baz", -1, None, Some("new-c"), "baz"),
                ],
            )
            .await
            .expect("enqueue agent job items should succeed");

        let outcome_summary: Vec<_> = result
            .outcomes
            .iter()
            .map(|outcome| match outcome {
                EnqueueAgentJobItemOutcome::Enqueued { item } => (
                    "enqueued",
                    item.item_id.clone(),
                    item.parent_item_id.clone(),
                    item.dedupe_key.clone(),
                    Some(item.row_index),
                ),
                EnqueueAgentJobItemOutcome::Deduped { item } => (
                    "deduped",
                    item.item_id.clone(),
                    item.parent_item_id.clone(),
                    item.dedupe_key.clone(),
                    Some(item.row_index),
                ),
                EnqueueAgentJobItemOutcome::MaxItemsReached {
                    requested_item_id,
                    parent_item_id,
                    dedupe_key,
                } => (
                    "max_items_reached",
                    requested_item_id.clone(),
                    parent_item_id.clone(),
                    dedupe_key.clone(),
                    None,
                ),
            })
            .collect();
        assert_eq!(
            outcome_summary,
            vec![
                (
                    "enqueued",
                    "foo".to_string(),
                    Some("root".to_string()),
                    Some("new-a".to_string()),
                    Some(2),
                ),
                (
                    "deduped",
                    "existing".to_string(),
                    Some("root".to_string()),
                    Some("existing-dedupe".to_string()),
                    Some(1),
                ),
                (
                    "enqueued",
                    "foo-2".to_string(),
                    Some("root".to_string()),
                    Some("new-b".to_string()),
                    Some(3),
                ),
                (
                    "max_items_reached",
                    "baz".to_string(),
                    Some("root".to_string()),
                    Some("new-c".to_string()),
                    None,
                ),
            ]
        );

        let persisted_items = runtime
            .list_agent_job_items(job.id.as_str(), None, None)
            .await
            .expect("list agent job items should succeed");
        let persisted_summary: Vec<_> = persisted_items
            .iter()
            .map(|item| {
                (
                    item.item_id.clone(),
                    item.parent_item_id.clone(),
                    item.dedupe_key.clone(),
                    item.row_index,
                )
            })
            .collect();
        assert_eq!(
            persisted_summary,
            vec![
                ("root".to_string(), None, None, 0),
                (
                    "existing".to_string(),
                    Some("root".to_string()),
                    Some("existing-dedupe".to_string()),
                    1,
                ),
                (
                    "foo".to_string(),
                    Some("root".to_string()),
                    Some("new-a".to_string()),
                    2,
                ),
                (
                    "foo-2".to_string(),
                    Some("root".to_string()),
                    Some("new-b".to_string()),
                    3,
                ),
            ]
        );
    }

    #[tokio::test]
    async fn enqueue_agent_job_items_rejects_unowned_parent() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home, "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let job = runtime
            .create_agent_job(
                &queue_job("job-queue-parent-check", Some(8)),
                &[item("root", 0, None, None, "root")],
            )
            .await
            .expect("create queue job should succeed");
        assert!(
            runtime
                .mark_agent_job_item_running_with_thread(job.id.as_str(), "root", "thread-a")
                .await
                .expect("mark parent running should succeed"),
            "parent item should become running"
        );

        let err = runtime
            .enqueue_agent_job_items(
                job.id.as_str(),
                "root",
                "thread-b",
                &[item("child", -1, None, Some("child-a"), "child")],
            )
            .await
            .expect_err("enqueue should reject unowned parent");
        assert!(
            err.to_string().contains("not owned by thread thread-b"),
            "unexpected error: {err:?}"
        );
    }
}
