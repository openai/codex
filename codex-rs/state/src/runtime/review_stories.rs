use super::*;

#[derive(Clone)]
pub struct ReviewStoryStore {
    pool: Arc<SqlitePool>,
}

impl ReviewStoryStore {
    pub(crate) fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewStoryRecord {
    pub story_snapshot_id: String,
    pub thread_id: String,
    pub source_fingerprint: String,
    pub status: String,
    pub title: String,
    pub step_count: i64,
    pub target_json: Value,
    pub snapshot_json: Value,
    pub previous_story_snapshot_id: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewStorySummaryRecord {
    pub story_snapshot_id: String,
    pub thread_id: String,
    pub source_fingerprint: String,
    pub status: String,
    pub title: String,
    pub step_count: i64,
    pub target_json: Value,
    pub previous_story_snapshot_id: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl ReviewStoryStore {
    pub async fn upsert_snapshot(&self, record: ReviewStoryRecord) -> anyhow::Result<()> {
        let ReviewStoryRecord {
            story_snapshot_id,
            thread_id,
            source_fingerprint,
            status,
            title,
            step_count,
            target_json,
            snapshot_json,
            previous_story_snapshot_id,
            created_at_ms,
            updated_at_ms,
        } = record;
        let target_json = serde_json::to_string(&target_json)?;
        let snapshot_json = serde_json::to_string(&snapshot_json)?;
        sqlx::query(
            r#"
INSERT INTO review_story_snapshots (
    story_snapshot_id,
    thread_id,
    source_fingerprint,
    status,
    title,
    step_count,
    target_json,
    snapshot_json,
    previous_story_snapshot_id,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(story_snapshot_id) DO UPDATE SET
    thread_id = excluded.thread_id,
    source_fingerprint = excluded.source_fingerprint,
    status = excluded.status,
    title = excluded.title,
    step_count = excluded.step_count,
    target_json = excluded.target_json,
    snapshot_json = excluded.snapshot_json,
    previous_story_snapshot_id = excluded.previous_story_snapshot_id,
    created_at_ms = excluded.created_at_ms,
    updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(story_snapshot_id)
        .bind(thread_id)
        .bind(source_fingerprint)
        .bind(status)
        .bind(title)
        .bind(step_count)
        .bind(target_json)
        .bind(snapshot_json)
        .bind(previous_story_snapshot_id)
        .bind(created_at_ms)
        .bind(updated_at_ms)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn get_snapshot(
        &self,
        thread_id: ThreadId,
        story_snapshot_id: &str,
    ) -> anyhow::Result<Option<ReviewStoryRecord>> {
        let row = sqlx::query(
            r#"
SELECT
    story_snapshot_id,
    thread_id,
    source_fingerprint,
    status,
    title,
    step_count,
    target_json,
    snapshot_json,
    previous_story_snapshot_id,
    created_at_ms,
    updated_at_ms
FROM review_story_snapshots
WHERE thread_id = ? AND story_snapshot_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .bind(story_snapshot_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| review_story_record_from_row(&row))
            .transpose()
    }

    pub async fn list_snapshots(
        &self,
        thread_id: ThreadId,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> anyhow::Result<(Vec<ReviewStorySummaryRecord>, Option<String>)> {
        let limit = limit
            .unwrap_or(/*default*/ 50)
            .clamp(/*min*/ 1, /*max*/ 100);
        let offset = cursor
            .as_deref()
            .and_then(|cursor| cursor.parse::<u32>().ok())
            .unwrap_or(/*default*/ 0);
        let rows = sqlx::query(
            r#"
SELECT
    story_snapshot_id,
    thread_id,
    source_fingerprint,
    status,
    title,
    step_count,
    target_json,
    previous_story_snapshot_id,
    created_at_ms,
    updated_at_ms
FROM review_story_snapshots
WHERE thread_id = ?
ORDER BY updated_at_ms DESC, story_snapshot_id DESC
LIMIT ? OFFSET ?
            "#,
        )
        .bind(thread_id.to_string())
        .bind(i64::from(limit) + 1)
        .bind(i64::from(offset))
        .fetch_all(self.pool.as_ref())
        .await?;

        let has_next = rows.len() > limit as usize;
        let rows = rows.into_iter().take(limit as usize);
        let records = rows
            .map(|row| review_story_summary_record_from_row(&row))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor = has_next.then(|| (offset + limit).to_string());
        Ok((records, next_cursor))
    }
}

fn review_story_record_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<ReviewStoryRecord> {
    let target_json: String = row.try_get("target_json")?;
    let snapshot_json: String = row.try_get("snapshot_json")?;
    Ok(ReviewStoryRecord {
        story_snapshot_id: row.try_get("story_snapshot_id")?,
        thread_id: row.try_get("thread_id")?,
        source_fingerprint: row.try_get("source_fingerprint")?,
        status: row.try_get("status")?,
        title: row.try_get("title")?,
        step_count: row.try_get("step_count")?,
        target_json: serde_json::from_str(&target_json)?,
        snapshot_json: serde_json::from_str(&snapshot_json)?,
        previous_story_snapshot_id: row.try_get("previous_story_snapshot_id")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn review_story_summary_record_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<ReviewStorySummaryRecord> {
    let target_json: String = row.try_get("target_json")?;
    Ok(ReviewStorySummaryRecord {
        story_snapshot_id: row.try_get("story_snapshot_id")?,
        thread_id: row.try_get("thread_id")?,
        source_fingerprint: row.try_get("source_fingerprint")?,
        status: row.try_get("status")?,
        title: row.try_get("title")?,
        step_count: row.try_get("step_count")?,
        target_json: serde_json::from_str(&target_json)?,
        previous_story_snapshot_id: row.try_get("previous_story_snapshot_id")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}
