use super::*;
use crate::sandboxing::SandboxPermissions;
use codex_network_proxy::BlockedRequestArgs;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::PathBufExt;
use core_test_support::test_path_buf;
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
    source.sync_session_approved_hosts_to(&seeded).await;

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
async fn sync_session_approved_hosts_to_replaces_existing_target_hosts() {
    let source = NetworkApprovalService::default();
    {
        let mut approved_hosts = source.session_approved_hosts.lock().await;
        approved_hosts.insert(HostApprovalKey {
            host: "source.example.com".to_string(),
            protocol: "https",
            port: 443,
        });
    }

    let target = NetworkApprovalService::default();
    {
        let mut approved_hosts = target.session_approved_hosts.lock().await;
        approved_hosts.insert(HostApprovalKey {
            host: "stale.example.com".to_string(),
            protocol: "https",
            port: 8443,
        });
    }

    source.sync_session_approved_hosts_to(&target).await;

    let copied = target
        .session_approved_hosts
        .lock()
        .await
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        copied,
        vec![HostApprovalKey {
            host: "source.example.com".to_string(),
            protocol: "https",
            port: 443,
        }]
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

#[test]
fn network_approval_flow_is_limited_to_restricted_sandbox_modes() {
    assert!(sandbox_policy_allows_network_approval_flow(
        &SandboxPolicy::new_read_only_policy()
    ));
    assert!(sandbox_policy_allows_network_approval_flow(
        &SandboxPolicy::new_workspace_write_policy()
    ));
    assert!(!sandbox_policy_allows_network_approval_flow(
        &SandboxPolicy::DangerFullAccess
    ));
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

async fn register_test_call(service: &NetworkApprovalService, registration_id: &str) {
    service
        .register_call(
            registration_id.to_string(),
            "turn-1".to_string(),
            GuardianNetworkAccessTrigger {
                call_id: "call-1".to_string(),
                tool_name: "shell".to_string(),
                command: vec!["curl".to_string(), "https://example.com".to_string()],
                cwd: test_path_buf("/tmp").abs(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
                justification: None,
                tty: None,
            },
        )
        .await;
}

#[tokio::test]
async fn active_call_preserves_triggering_command_context() {
    let service = NetworkApprovalService::default();
    let expected = GuardianNetworkAccessTrigger {
        call_id: "call-1".to_string(),
        tool_name: "shell".to_string(),
        command: vec!["curl".to_string(), "https://example.com".to_string()],
        cwd: test_path_buf("/repo").abs(),
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("fetch release metadata".to_string()),
        tty: None,
    };

    service
        .register_call(
            "registration-1".to_string(),
            "turn-1".to_string(),
            expected.clone(),
        )
        .await;

    let call = service
        .resolve_single_active_call()
        .await
        .expect("single active call should resolve");

    assert_eq!(&call.trigger, &expected);
}

#[tokio::test]
async fn record_blocked_request_sets_policy_outcome_for_owner_call() {
    let service = NetworkApprovalService::default();
    register_test_call(&service, "registration-1").await;

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
    register_test_call(&service, "registration-1").await;

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
    register_test_call(&service, "registration-1").await;
    register_test_call(&service, "registration-2").await;

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert_eq!(service.take_call_outcome("registration-1").await, None);
    assert_eq!(service.take_call_outcome("registration-2").await, None);
}
