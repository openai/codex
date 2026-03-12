use super::*;
use codex_network_proxy::BlockedRequestArgs;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn pending_approvals_are_deduped_per_host_protocol_and_port() {
    let service = NetworkApprovalService::default();
    let key = HostApprovalKey {
        host: "example.com".to_string(),
        protocol: "http",
        port: 443,
    };

    let (first, first_is_owner) = service.get_or_create_pending_approval(key.clone()).await;
    let (second, second_is_owner) = service.get_or_create_pending_approval(key).await;

    assert!(first_is_owner);
    assert!(!second_is_owner);
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn pending_approvals_do_not_dedupe_across_ports() {
    let service = NetworkApprovalService::default();
    let first_key = HostApprovalKey {
        host: "example.com".to_string(),
        protocol: "https",
        port: 443,
    };
    let second_key = HostApprovalKey {
        host: "example.com".to_string(),
        protocol: "https",
        port: 8443,
    };

    let (first, first_is_owner) = service.get_or_create_pending_approval(first_key).await;
    let (second, second_is_owner) = service.get_or_create_pending_approval(second_key).await;

    assert!(first_is_owner);
    assert!(second_is_owner);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn session_approved_hosts_preserve_protocol_and_port_scope() {
    let source = NetworkApprovalService::default();
    {
        let mut approved_hosts = source.session_approved_hosts.lock().await;
        approved_hosts.extend([
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "https",
                port: 443,
            },
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "https",
                port: 8443,
            },
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "http",
                port: 80,
            },
        ]);
    }

    let seeded = NetworkApprovalService::default();
    source.copy_session_approved_hosts_to(&seeded).await;

    let mut copied = seeded
        .session_approved_hosts
        .lock()
        .await
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    copied.sort_by(|a, b| (&a.host, a.protocol, a.port).cmp(&(&b.host, b.protocol, b.port)));

    assert_eq!(
        copied,
        vec![
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "http",
                port: 80,
            },
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "https",
                port: 443,
            },
            HostApprovalKey {
                host: "example.com".to_string(),
                protocol: "https",
                port: 8443,
            },
        ]
    );
}

#[tokio::test]
async fn pending_waiters_receive_owner_decision() {
    let pending = Arc::new(PendingHostApproval::new());

    let waiter = {
        let pending = Arc::clone(&pending);
        tokio::spawn(async move { pending.wait_for_decision().await })
    };

    pending
        .set_decision(PendingApprovalDecision::AllowOnce)
        .await;

    let decision = waiter.await.expect("waiter should complete");
    assert_eq!(decision, PendingApprovalDecision::AllowOnce);
}

#[test]
fn allow_once_and_allow_for_session_both_allow_network() {
    assert_eq!(
        PendingApprovalDecision::AllowOnce.to_network_decision(),
        NetworkDecision::Allow
    );
    assert_eq!(
        PendingApprovalDecision::AllowForSession.to_network_decision(),
        NetworkDecision::Allow
    );
}

#[test]
fn only_never_policy_disables_network_approval_flow() {
    assert!(!allows_network_approval_flow(AskForApproval::Never));
    assert!(allows_network_approval_flow(AskForApproval::OnRequest));
    assert!(allows_network_approval_flow(AskForApproval::OnFailure));
    assert!(allows_network_approval_flow(AskForApproval::UnlessTrusted));
}

fn denied_blocked_request(host: &str) -> BlockedRequest {
    BlockedRequest::new(BlockedRequestArgs {
        host: host.to_string(),
        reason: "not_allowed".to_string(),
        client: None,
        method: None,
        mode: None,
        protocol: "http".to_string(),
        decision: Some("deny".to_string()),
        source: Some("decider".to_string()),
        port: Some(80),
    })
}

#[tokio::test]
async fn record_blocked_request_sets_policy_outcome_for_owner_call() {
    let service = NetworkApprovalService::default();
    service
        .register_call(
            "registration-1".to_string(),
            "turn-1".to_string(),
            "call-1".to_string(),
        )
        .await;

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert_eq!(
            service.take_call_outcome("registration-1").await,
            Some(NetworkApprovalOutcome::DeniedByPolicy(
                "Network access to \"example.com\" was blocked: domain is not on the allowlist for the current sandbox mode.".to_string()
            ))
        );
}

#[tokio::test]
async fn blocked_request_policy_does_not_override_user_denial_outcome() {
    let service = NetworkApprovalService::default();
    service
        .register_call(
            "registration-1".to_string(),
            "turn-1".to_string(),
            "call-1".to_string(),
        )
        .await;

    service
        .record_call_outcome("registration-1", NetworkApprovalOutcome::DeniedByUser)
        .await;
    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert_eq!(
        service.take_call_outcome("registration-1").await,
        Some(NetworkApprovalOutcome::DeniedByUser)
    );
}

#[tokio::test]
async fn record_blocked_request_ignores_ambiguous_unattributed_blocked_requests() {
    let service = NetworkApprovalService::default();
    service
        .register_call(
            "registration-1".to_string(),
            "turn-1".to_string(),
            "call-1".to_string(),
        )
        .await;
    service
        .register_call(
            "registration-2".to_string(),
            "turn-1".to_string(),
            "call-2".to_string(),
        )
        .await;

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert_eq!(service.take_call_outcome("registration-1").await, None);
    assert_eq!(service.take_call_outcome("registration-2").await, None);
}
