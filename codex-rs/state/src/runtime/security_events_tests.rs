use super::StateRuntime;
use super::test_support::unique_temp_dir;
use super::*;
use crate::SecurityEventKind;
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

#[tokio::test]
async fn insert_security_event_prunes_old_rows_per_thread_and_kind() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");

    for offset in 0..=SECURITY_EVENT_PARTITION_ROW_LIMIT {
        runtime
            .insert_security_event(&SecurityEventCreateParams {
                created_at: 1_700_000_000 + offset,
                kind: SecurityEventKind::SandboxViolation,
                thread_id: Some("thread-1".to_string()),
                turn_id: None,
                call_id: None,
                tool_name: None,
                resource: Some(SecurityEventResource::FileSystem),
                sandbox_type: None,
                reason: None,
                path: None,
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
            .expect("insert sandbox event");
    }
    runtime
        .insert_security_event(&SecurityEventCreateParams {
            created_at: 1_700_100_000,
            kind: SecurityEventKind::AutoReviewDecision,
            thread_id: Some("thread-1".to_string()),
            turn_id: None,
            call_id: None,
            tool_name: None,
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
        .expect("insert review event");
    runtime
        .insert_security_event(&SecurityEventCreateParams {
            created_at: 1_700_200_000,
            kind: SecurityEventKind::SandboxViolation,
            thread_id: Some("thread-2".to_string()),
            turn_id: None,
            call_id: None,
            tool_name: None,
            resource: Some(SecurityEventResource::Network),
            sandbox_type: None,
            reason: None,
            path: None,
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
        .expect("insert other thread event");

    let sandbox_events = runtime
        .list_security_events(&SecurityEventQuery {
            thread_id: Some("thread-1".to_string()),
            kind: Some(SecurityEventKind::SandboxViolation),
            limit: None,
        })
        .await
        .expect("list sandbox events");
    assert_eq!(
        sandbox_events.len(),
        usize::try_from(SECURITY_EVENT_PARTITION_ROW_LIMIT).expect("row limit fits usize")
    );
    assert_eq!(
        sandbox_events.last().map(|event| event.created_at),
        Some(1_700_000_001)
    );

    assert_eq!(
        runtime
            .list_security_events(&SecurityEventQuery {
                thread_id: Some("thread-1".to_string()),
                kind: Some(SecurityEventKind::AutoReviewDecision),
                limit: None,
            })
            .await
            .expect("list review events")
            .len(),
        1
    );
    assert_eq!(
        runtime
            .list_security_events(&SecurityEventQuery {
                thread_id: Some("thread-2".to_string()),
                kind: Some(SecurityEventKind::SandboxViolation),
                limit: None,
            })
            .await
            .expect("list other thread events")
            .len(),
        1
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn delete_security_events_before_removes_old_rows() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");

    for created_at in [1_700_000_000, 1_700_000_100] {
        runtime
            .insert_security_event(&SecurityEventCreateParams {
                created_at,
                kind: SecurityEventKind::SandboxViolation,
                thread_id: Some("thread-1".to_string()),
                turn_id: None,
                call_id: None,
                tool_name: None,
                resource: Some(SecurityEventResource::FileSystem),
                sandbox_type: None,
                reason: None,
                path: None,
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
            .expect("insert security event");
    }

    assert_eq!(
        runtime
            .delete_security_events_before(1_700_000_050)
            .await
            .expect("delete old events"),
        1
    );
    assert_eq!(
        runtime
            .list_security_events(&SecurityEventQuery {
                thread_id: Some("thread-1".to_string()),
                kind: Some(SecurityEventKind::SandboxViolation),
                limit: None,
            })
            .await
            .expect("list remaining events")
            .into_iter()
            .map(|event| event.created_at)
            .collect::<Vec<_>>(),
        vec![1_700_000_100]
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}
