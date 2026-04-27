use super::*;
#[cfg(test)]
use crate::SecurityEventKind;
use crate::SecurityEventResource;

impl StateRuntime {
    /// Persist a security-relevant audit event.
    pub async fn insert_security_event(
        &self,
        event: &SecurityEventCreateParams,
    ) -> anyhow::Result<()> {
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
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn security_events_round_trip_and_filter() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_security_event(&SecurityEventCreateParams {
                created_at: 1_700_000_000,
                kind: SecurityEventKind::SandboxViolation,
                thread_id: Some("thread-1".to_string()),
                turn_id: Some("turn-1".to_string()),
                call_id: Some("call-1".to_string()),
                tool_name: Some("shell".to_string()),
                resource: Some(SecurityEventResource::FileSystem),
                sandbox_type: Some("macos_seatbelt".to_string()),
                reason: Some("permission_denied".to_string()),
                path: Some("/private/var/db".to_string()),
                host: None,
                port: None,
                protocol: None,
                method: None,
                decision: None,
                source: None,
                review_id: None,
                reviewer: None,
                review_decision: None,
                details_json: None,
            })
            .await
            .expect("insert filesystem event");
        runtime
            .insert_security_event(&SecurityEventCreateParams {
                created_at: 1_700_000_001,
                kind: SecurityEventKind::SandboxViolation,
                thread_id: Some("thread-1".to_string()),
                turn_id: Some("turn-2".to_string()),
                call_id: Some("call-2".to_string()),
                tool_name: Some("shell".to_string()),
                resource: Some(SecurityEventResource::Network),
                sandbox_type: None,
                reason: Some("not_allowed".to_string()),
                path: None,
                host: Some("example.com".to_string()),
                port: Some(443),
                protocol: Some("https_connect".to_string()),
                method: Some("CONNECT".to_string()),
                decision: Some("deny".to_string()),
                source: Some("proxy_state".to_string()),
                review_id: None,
                reviewer: None,
                review_decision: None,
                details_json: None,
            })
            .await
            .expect("insert network event");
        runtime
            .insert_security_event(&SecurityEventCreateParams {
                created_at: 1_700_000_002,
                kind: SecurityEventKind::AutoReviewDecision,
                thread_id: Some("thread-2".to_string()),
                turn_id: Some("turn-3".to_string()),
                call_id: Some("call-3".to_string()),
                tool_name: Some("shell".to_string()),
                resource: None,
                sandbox_type: None,
                reason: None,
                path: None,
                host: None,
                port: None,
                protocol: None,
                method: None,
                decision: None,
                source: None,
                review_id: Some("review-1".to_string()),
                reviewer: Some("auto_review".to_string()),
                review_decision: Some("denied".to_string()),
                details_json: None,
            })
            .await
            .expect("insert auto review event");

        assert_eq!(
            runtime
                .list_security_events(&SecurityEventQuery {
                    thread_id: Some("thread-1".to_string()),
                    kind: Some(SecurityEventKind::SandboxViolation),
                    limit: None,
                })
                .await
                .expect("list sandbox events"),
            vec![
                SecurityEvent {
                    id: 2,
                    created_at: 1_700_000_001,
                    kind: SecurityEventKind::SandboxViolation,
                    thread_id: Some("thread-1".to_string()),
                    turn_id: Some("turn-2".to_string()),
                    call_id: Some("call-2".to_string()),
                    tool_name: Some("shell".to_string()),
                    resource: Some(SecurityEventResource::Network),
                    sandbox_type: None,
                    reason: Some("not_allowed".to_string()),
                    path: None,
                    host: Some("example.com".to_string()),
                    port: Some(443),
                    protocol: Some("https_connect".to_string()),
                    method: Some("CONNECT".to_string()),
                    decision: Some("deny".to_string()),
                    source: Some("proxy_state".to_string()),
                    review_id: None,
                    reviewer: None,
                    review_decision: None,
                    details_json: None,
                },
                SecurityEvent {
                    id: 1,
                    created_at: 1_700_000_000,
                    kind: SecurityEventKind::SandboxViolation,
                    thread_id: Some("thread-1".to_string()),
                    turn_id: Some("turn-1".to_string()),
                    call_id: Some("call-1".to_string()),
                    tool_name: Some("shell".to_string()),
                    resource: Some(SecurityEventResource::FileSystem),
                    sandbox_type: Some("macos_seatbelt".to_string()),
                    reason: Some("permission_denied".to_string()),
                    path: Some("/private/var/db".to_string()),
                    host: None,
                    port: None,
                    protocol: None,
                    method: None,
                    decision: None,
                    source: None,
                    review_id: None,
                    reviewer: None,
                    review_decision: None,
                    details_json: None,
                },
            ]
        );

        assert_eq!(
            runtime
                .list_security_events(&SecurityEventQuery {
                    thread_id: None,
                    kind: Some(SecurityEventKind::AutoReviewDecision),
                    limit: Some(1),
                })
                .await
                .expect("list auto review events"),
            vec![SecurityEvent {
                id: 3,
                created_at: 1_700_000_002,
                kind: SecurityEventKind::AutoReviewDecision,
                thread_id: Some("thread-2".to_string()),
                turn_id: Some("turn-3".to_string()),
                call_id: Some("call-3".to_string()),
                tool_name: Some("shell".to_string()),
                resource: None,
                sandbox_type: None,
                reason: None,
                path: None,
                host: None,
                port: None,
                protocol: None,
                method: None,
                decision: None,
                source: None,
                review_id: Some("review-1".to_string()),
                reviewer: Some("auto_review".to_string()),
                review_decision: Some("denied".to_string()),
                details_json: None,
            }]
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
