use super::*;
use crate::SecurityEventResource;

impl StateRuntime {
    /// Persist a security-relevant audit event.
    pub async fn insert_security_event(
        &self,
        event: &SecurityEventCreateParams,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
INSERT INTO security_events (
    created_at,
    kind,
    thread_id,
    turn_id,
    call_id,
    tool_name,
    resource,
    sandbox_type,
    reason,
    path,
    host,
    port,
    protocol,
    method,
    decision,
    source,
    review_id,
    reviewer,
    review_decision,
    details_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event.created_at)
        .bind(event.kind.as_str())
        .bind(event.thread_id.as_deref())
        .bind(event.turn_id.as_deref())
        .bind(event.call_id.as_deref())
        .bind(event.tool_name.as_deref())
        .bind(event.resource.map(SecurityEventResource::as_str))
        .bind(event.sandbox_type.as_deref())
        .bind(event.reason.as_deref())
        .bind(event.path.as_deref())
        .bind(event.host.as_deref())
        .bind(event.port.map(i64::from))
        .bind(event.protocol.as_deref())
        .bind(event.method.as_deref())
        .bind(event.decision.as_deref())
        .bind(event.source.as_deref())
        .bind(event.review_id.as_deref())
        .bind(event.reviewer.as_deref())
        .bind(event.review_decision.as_deref())
        .bind(event.details_json.as_deref())
        .execute(&mut *tx)
        .await?;
        self.prune_security_events_after_insert(event, &mut tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Keep local security-event history bounded while preserving recent context per bucket.
    async fn prune_security_events_after_insert(
        &self,
        event: &SecurityEventCreateParams,
        tx: &mut SqliteConnection,
    ) -> anyhow::Result<()> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
DELETE FROM security_events
WHERE id IN (
    SELECT id
    FROM (
        SELECT
            id,
            ROW_NUMBER() OVER (
                ORDER BY created_at DESC, id DESC
            ) AS row_number
        FROM security_events
        WHERE
            "#,
        );
        if let Some(thread_id) = event.thread_id.as_deref() {
            builder.push("thread_id = ").push_bind(thread_id);
        } else {
            builder.push("thread_id IS NULL");
        }
        builder
            .push(" AND kind = ")
            .push_bind(event.kind.as_str())
            .push(
                r#"
    )
    WHERE row_number >
            "#,
            )
            .push_bind(SECURITY_EVENT_PARTITION_ROW_LIMIT)
            .push("\n)");
        builder.build().execute(&mut *tx).await?;
        Ok(())
    }

    pub(crate) async fn delete_security_events_before(
        &self,
        cutoff_ts: i64,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM security_events WHERE created_at < ?")
            .bind(cutoff_ts)
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn run_security_events_startup_maintenance(&self) -> anyhow::Result<()> {
        let Some(cutoff) =
            Utc::now().checked_sub_signed(chrono::Duration::days(SECURITY_EVENT_RETENTION_DAYS))
        else {
            return Ok(());
        };
        self.delete_security_events_before(cutoff.timestamp())
            .await?;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(self.pool.as_ref())
            .await?;
        sqlx::query("PRAGMA incremental_vacuum")
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    /// List security events with optional thread/kind filters.
    pub async fn list_security_events(
        &self,
        query: &SecurityEventQuery,
    ) -> anyhow::Result<Vec<SecurityEvent>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
SELECT
    id,
    created_at,
    kind,
    thread_id,
    turn_id,
    call_id,
    tool_name,
    resource,
    sandbox_type,
    reason,
    path,
    host,
    port,
    protocol,
    method,
    decision,
    source,
    review_id,
    reviewer,
    review_decision,
    details_json
FROM security_events
            "#,
        );
        let mut has_where = false;
        if let Some(thread_id) = query.thread_id.as_deref() {
            builder.push(" WHERE ");
            builder.push("thread_id = ").push_bind(thread_id);
            has_where = true;
        }
        if let Some(kind) = query.kind {
            if has_where {
                builder.push(" AND ");
            } else {
                builder.push(" WHERE ");
            }
            builder.push("kind = ").push_bind(kind.as_str());
        }
        builder.push(" ORDER BY created_at DESC, id DESC");
        if let Some(limit) = query.limit {
            builder.push(" LIMIT ").push_bind(i64::from(limit));
        }

        let rows = builder
            .build()
            .fetch_all(self.pool.as_ref())
            .await?
            .into_iter()
            .map(SecurityEvent::from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "security_events_tests.rs"]
mod tests;
