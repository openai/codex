use super::*;
use crate::sandboxing::SandboxPermissions;
use codex_network_proxy::BlockedRequestArgs;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use core_test_support::PathBufExt;
use core_test_support::test_path_buf;
use futures::poll;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

fn pending_key(host: HostApprovalKey, turn_id: &str, execution_id: &str) -> PendingHostApprovalKey {
    PendingHostApprovalKey {
        host,
        turn_id: turn_id.to_string(),
        execution_id: Some(execution_id.to_string()),
    }
}

#[test]
fn pending_approvals_are_deduped_within_one_execution() {
    let service = NetworkApprovalService::default();
    let key = pending_key(
        HostApprovalKey {
            environment_id: "local".to_string(),
            host: "example.com".to_string(),
            protocol: "http",
            port: 443,
        },
        "turn-1",
        "execution-1",
    );

    let (first, first_is_owner) = service.get_or_create_pending_approval(key.clone());
    let (second, second_is_owner) = service.get_or_create_pending_approval(key);

    assert!(first_is_owner);
    assert!(!second_is_owner);
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn pending_approvals_do_not_dedupe_across_ports() {
    let service = NetworkApprovalService::default();
    let first_host = HostApprovalKey {
        environment_id: "local".to_string(),
        host: "example.com".to_string(),
        protocol: "https",
        port: 443,
    };
    let second_host = HostApprovalKey {
        port: 8443,
        ..first_host.clone()
    };

    let (first, first_is_owner) =
        service.get_or_create_pending_approval(pending_key(first_host, "turn-1", "execution-1"));
    let (second, second_is_owner) =
        service.get_or_create_pending_approval(pending_key(second_host, "turn-1", "execution-1"));

    assert!(first_is_owner);
    assert!(second_is_owner);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn pending_approvals_do_not_dedupe_across_environments() {
    let service = NetworkApprovalService::default();
    let first_host = HostApprovalKey {
        environment_id: "local".to_string(),
        host: "example.com".to_string(),
        protocol: "https",
        port: 443,
    };
    let second_host = HostApprovalKey {
        environment_id: "remote".to_string(),
        ..first_host.clone()
    };

    let (first, first_is_owner) =
        service.get_or_create_pending_approval(pending_key(first_host, "turn-1", "execution-1"));
    let (second, second_is_owner) =
        service.get_or_create_pending_approval(pending_key(second_host, "turn-1", "execution-1"));

    assert!(first_is_owner);
    assert!(second_is_owner);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn pending_approvals_do_not_dedupe_across_execution_or_turn() {
    let service = NetworkApprovalService::default();
    let host = HostApprovalKey {
        environment_id: "remote".to_string(),
        host: "example.com".to_string(),
        protocol: "https",
        port: 443,
    };

    let (first, first_is_owner) =
        service.get_or_create_pending_approval(pending_key(host.clone(), "turn-1", "execution-1"));
    let (second, second_is_owner) =
        service.get_or_create_pending_approval(pending_key(host.clone(), "turn-1", "execution-2"));
    let (third, third_is_owner) =
        service.get_or_create_pending_approval(pending_key(host, "turn-2", "execution-1"));

    assert!(first_is_owner);
    assert!(second_is_owner);
    assert!(third_is_owner);
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(!Arc::ptr_eq(&first, &third));
}

#[tokio::test]
async fn session_approved_hosts_are_scoped_by_environment() {
    let service = NetworkApprovalService::default();
    let local_key = HostApprovalKey {
        environment_id: "local".to_string(),
        host: "example.com".to_string(),
        protocol: "https",
        port: 443,
    };
    let remote_key = HostApprovalKey {
        environment_id: "remote".to_string(),
        ..local_key.clone()
    };
    service
        .session_approved_hosts
        .lock()
        .await
        .insert(local_key);

    assert!(
        !service
            .session_approved_hosts
            .lock()
            .await
            .contains(&remote_key)
    );
}

#[tokio::test]
async fn session_approved_hosts_preserve_protocol_and_port_scope() {
    let source = NetworkApprovalService::default();
    {
        let mut approved_hosts = source.session_approved_hosts.lock().await;
        approved_hosts.extend([
            HostApprovalKey {
                environment_id: "local".to_string(),
                host: "example.com".to_string(),
                protocol: "https",
                port: 443,
            },
            HostApprovalKey {
                environment_id: "local".to_string(),
                host: "example.com".to_string(),
                protocol: "https",
                port: 8443,
            },
            HostApprovalKey {
                environment_id: "local".to_string(),
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
    copied.sort_by(|a, b| {
        (&a.environment_id, &a.host, a.protocol, a.port).cmp(&(
            &b.environment_id,
            &b.host,
            b.protocol,
            b.port,
        ))
    });

    assert_eq!(
        copied,
        vec![
            HostApprovalKey {
                environment_id: "local".to_string(),
                host: "example.com".to_string(),
                protocol: "http",
                port: 80,
            },
            HostApprovalKey {
                environment_id: "local".to_string(),
                host: "example.com".to_string(),
                protocol: "https",
                port: 443,
            },
            HostApprovalKey {
                environment_id: "local".to_string(),
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
            environment_id: "local".to_string(),
            host: "source.example.com".to_string(),
            protocol: "https",
            port: 443,
        });
    }

    let target = NetworkApprovalService::default();
    {
        let mut approved_hosts = target.session_approved_hosts.lock().await;
        approved_hosts.insert(HostApprovalKey {
            environment_id: "local".to_string(),
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
            environment_id: "local".to_string(),
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

    pending.set_decision(PendingApprovalDecision::AllowOnce);

    let decision = waiter.await.expect("waiter should complete");
    assert_eq!(decision, PendingApprovalDecision::AllowOnce);
}

#[tokio::test]
async fn dropping_pending_owner_denies_waiters_and_preserves_replacement() {
    let service = NetworkApprovalService::default();
    let execution_cancellation =
        register_call_with_default_shell_trigger(&service, "execution-1").await;
    let deferred = DeferredNetworkApproval {
        registration_id: "execution-1".to_string(),
        cancellation_token: execution_cancellation.clone(),
        finish_outcome: Arc::new(OnceCell::new()),
        _execution_proxy: None,
    };
    let key = pending_key(
        HostApprovalKey {
            environment_id: "remote".to_string(),
            host: "example.com".to_string(),
            protocol: "https",
            port: 443,
        },
        "turn-1",
        "execution-1",
    );
    let (pending, is_owner) = service.get_or_create_pending_approval(key.clone());
    assert!(is_owner);
    let owner = PendingHostApprovalOwner::new(
        &service,
        key.clone(),
        Arc::clone(&pending),
        Some(execution_cancellation.clone()),
    );

    let first_waiter = pending.wait_for_decision();
    let second_waiter = pending.wait_for_decision();
    tokio::pin!(first_waiter, second_waiter);
    assert!(poll!(first_waiter.as_mut()).is_pending());
    assert!(poll!(second_waiter.as_mut()).is_pending());
    drop(owner);

    let decisions = timeout(Duration::from_secs(1), async {
        tokio::join!(first_waiter, second_waiter)
    })
    .await
    .expect("coalesced waiters should fail closed when their owner is dropped");
    assert_eq!(
        decisions,
        (PendingApprovalDecision::Deny, PendingApprovalDecision::Deny)
    );
    assert!(execution_cancellation.is_cancelled());
    let error = deferred
        .finish(&service)
        .await
        .expect_err("abandoned approval should fail its execution closed");
    assert!(matches!(
        error,
        ToolError::Rejected(message) if message == ABANDONED_NETWORK_APPROVAL_MESSAGE
    ));

    let (replacement, is_owner) = service.get_or_create_pending_approval(key.clone());
    assert!(is_owner);
    assert!(!Arc::ptr_eq(&pending, &replacement));
    let replacement_owner = PendingHostApprovalOwner::new(
        &service,
        key.clone(),
        Arc::clone(&replacement),
        /*execution_cancellation*/ None,
    );

    let stale = Arc::new(PendingHostApproval::new());
    drop(PendingHostApprovalOwner::new(
        &service,
        key.clone(),
        Arc::clone(&stale),
        /*execution_cancellation*/ None,
    ));
    assert_eq!(
        stale.wait_for_decision().await,
        PendingApprovalDecision::Deny
    );

    let (current, is_owner) = service.get_or_create_pending_approval(key);
    assert!(!is_owner);
    assert!(Arc::ptr_eq(&current, &replacement));
    replacement_owner.complete(PendingApprovalDecision::AllowOnce);
    assert_eq!(
        replacement.wait_for_decision().await,
        PendingApprovalDecision::AllowOnce
    );
}

#[tokio::test]
async fn pending_owner_cancels_execution_only_for_denial() {
    let service = NetworkApprovalService::default();
    let key = pending_key(
        HostApprovalKey {
            environment_id: "remote".to_string(),
            host: "example.com".to_string(),
            protocol: "https",
            port: 443,
        },
        "turn-1",
        "execution-1",
    );
    let (pending, is_owner) = service.get_or_create_pending_approval(key.clone());
    assert!(is_owner);
    let execution_cancellation = CancellationToken::new();
    let mut owner = PendingHostApprovalOwner::new(
        &service,
        key.clone(),
        Arc::clone(&pending),
        Some(execution_cancellation.clone()),
    );
    owner.set_decision_on_drop(PendingApprovalDecision::AllowForSession);

    drop(owner);

    assert!(!execution_cancellation.is_cancelled());
    assert_eq!(
        pending.wait_for_decision().await,
        PendingApprovalDecision::AllowForSession
    );

    let (pending, is_owner) = service.get_or_create_pending_approval(key.clone());
    assert!(is_owner);
    let execution_cancellation = CancellationToken::new();
    PendingHostApprovalOwner::new(
        &service,
        key,
        Arc::clone(&pending),
        Some(execution_cancellation.clone()),
    )
    .complete(PendingApprovalDecision::Deny);

    assert!(execution_cancellation.is_cancelled());
    assert_eq!(
        pending.wait_for_decision().await,
        PendingApprovalDecision::Deny
    );
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
    assert!(allows_network_approval_flow(AskForApproval::UnlessTrusted));
}

#[test]
fn network_approval_flow_is_limited_to_restricted_sandbox_modes() {
    assert!(permission_profile_allows_network_approval_flow(
        &PermissionProfile::read_only()
    ));
    assert!(permission_profile_allows_network_approval_flow(
        &PermissionProfile::workspace_write()
    ));
    assert!(!permission_profile_allows_network_approval_flow(
        &PermissionProfile::Disabled
    ));
    assert!(!permission_profile_allows_network_approval_flow(
        &PermissionProfile::External {
            network: NetworkSandboxPolicy::Restricted,
        }
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

fn denied_blocked_request_for_execution(host: &str, execution_id: &str) -> BlockedRequest {
    let mut blocked = denied_blocked_request(host);
    blocked.execution_id = Some(execution_id.to_string());
    blocked
}

async fn register_call_with_default_shell_trigger(
    service: &NetworkApprovalService,
    registration_id: &str,
) -> CancellationToken {
    let cancellation_token = CancellationToken::new();
    service
        .register_call(
            registration_id.to_string(),
            "turn-1".to_string(),
            GuardianNetworkAccessTrigger {
                call_id: "call-1".to_string(),
                tool_name: "shell_command".to_string(),
                command: vec!["curl".to_string(), "https://example.com".to_string()],
                cwd: test_path_buf("/tmp").abs(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
                justification: None,
                tty: None,
            },
            "curl https://example.com".to_string(),
            "local".to_string(),
            cancellation_token.clone(),
        )
        .await;
    cancellation_token
}

#[tokio::test]
async fn active_call_preserves_triggering_command_context() {
    let service = NetworkApprovalService::default();
    let expected = GuardianNetworkAccessTrigger {
        call_id: "call-1".to_string(),
        tool_name: "shell_command".to_string(),
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
            "curl https://example.com".to_string(),
            "remote".to_string(),
            CancellationToken::new(),
        )
        .await;

    let call = service
        .resolve_single_active_call()
        .await
        .expect("single active call should resolve");

    assert_eq!(&call.trigger, &expected);
    assert_eq!(call.command, "curl https://example.com");
    assert_eq!(call.environment_id, "remote");
}

#[tokio::test]
async fn multiple_active_calls_are_ambiguous_even_in_the_same_environment() {
    let service = NetworkApprovalService::default();
    register_call_with_default_shell_trigger(&service, "registration-1").await;
    register_call_with_default_shell_trigger(&service, "registration-2").await;

    match service.resolve_active_call_attribution().await {
        ActiveNetworkApprovalAttribution::Ambiguous => {}
        ActiveNetworkApprovalAttribution::None | ActiveNetworkApprovalAttribution::Single(_) => {
            panic!("multiple active calls should be ambiguous")
        }
    }
}

#[tokio::test]
async fn record_blocked_request_sets_policy_outcome_for_owner_call() {
    let service = NetworkApprovalService::default();
    let cancellation_token =
        register_call_with_default_shell_trigger(&service, "registration-1").await;

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert!(cancellation_token.is_cancelled());
    assert_eq!(
            service.take_call_outcome("registration-1").await,
            Some(NetworkApprovalOutcome::DeniedByPolicy(
                "Network access to \"example.com\" was blocked: domain is not on the allowlist for the current sandbox mode.".to_string()
            ))
        );
}

#[tokio::test]
async fn blocked_request_does_not_override_recorded_approval_outcome() {
    let service = NetworkApprovalService::default();
    register_call_with_default_shell_trigger(&service, "registration-1").await;
    let rejection = "approval client unavailable";

    service
        .record_call_outcome(
            "registration-1",
            NetworkApprovalOutcome::DeniedByApproval(rejection.to_string()),
        )
        .await;
    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    let error =
        network_approval_outcome_to_result(service.take_call_outcome("registration-1").await)
            .expect_err("approval denial should remain an error");
    assert!(matches!(error, ToolError::Rejected(message) if message == rejection));
}

#[tokio::test]
async fn specific_approval_outcome_replaces_earlier_blocked_request() {
    let service = NetworkApprovalService::default();
    register_call_with_default_shell_trigger(&service, "registration-1").await;
    let rejection = "specific approval rejection";

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;
    service
        .record_call_outcome(
            "registration-1",
            NetworkApprovalOutcome::DeniedByApproval(rejection.to_string()),
        )
        .await;

    let error =
        network_approval_outcome_to_result(service.take_call_outcome("registration-1").await)
            .expect_err("specific approval denial should replace blocked policy denial");
    assert!(matches!(error, ToolError::Rejected(message) if message == rejection));
}

#[test]
fn approval_denial_messages_are_bounded_for_model_context() {
    let rejection = "x".repeat(40_000);

    let error = network_approval_outcome_to_result(Some(NetworkApprovalOutcome::DeniedByApproval(
        rejection,
    )))
    .expect_err("approval denial should remain an error");
    let ToolError::Rejected(message) = error else {
        panic!("approval denial should produce a rejected tool error");
    };

    assert!(codex_utils_string::approx_token_count(&message) < 1_000);
    assert!(message.contains("tokens truncated"));
}

#[tokio::test]
async fn finish_call_returns_denial_and_unregisters_active_call() {
    let service = NetworkApprovalService::default();
    let cancellation_token =
        register_call_with_default_shell_trigger(&service, "registration-1").await;

    service
        .record_call_outcome(
            "registration-1",
            NetworkApprovalOutcome::DeniedByPolicy("network denied".to_string()),
        )
        .await;

    let err = service
        .finish_call("registration-1", &cancellation_token)
        .await
        .expect_err("denial should be returned");

    assert!(matches!(err, ToolError::Rejected(message) if message == "network denied"));
    assert!(service.resolve_single_active_call().await.is_none());
    assert_eq!(service.take_call_outcome("registration-1").await, None);
}

#[tokio::test]
async fn finish_call_reports_abandoned_network_approval() {
    let service = NetworkApprovalService::default();
    let cancellation_token =
        register_call_with_default_shell_trigger(&service, "registration-1").await;
    cancellation_token.cancel();

    let err = service
        .finish_call("registration-1", &cancellation_token)
        .await
        .expect_err("abandoned approval should be returned");

    assert!(matches!(
        err,
        ToolError::Rejected(message) if message == ABANDONED_NETWORK_APPROVAL_MESSAGE
    ));
}

#[tokio::test]
async fn deferred_finish_reuses_denial_result_after_first_consumer() {
    let service = NetworkApprovalService::default();
    let cancellation_token =
        register_call_with_default_shell_trigger(&service, "registration-1").await;
    let deferred = DeferredNetworkApproval {
        registration_id: "registration-1".to_string(),
        cancellation_token,
        finish_outcome: Arc::new(OnceCell::new()),
        _execution_proxy: None,
    };
    service
        .record_call_outcome(
            "registration-1",
            NetworkApprovalOutcome::DeniedByPolicy("network denied".to_string()),
        )
        .await;

    let first = deferred
        .finish(&service)
        .await
        .expect_err("first consumer should see denial");
    let second = deferred
        .finish(&service)
        .await
        .expect_err("second consumer should reuse denial");

    assert!(matches!(first, ToolError::Rejected(message) if message == "network denied"));
    assert!(matches!(second, ToolError::Rejected(message) if message == "network denied"));
}

#[tokio::test]
async fn record_call_outcome_ignores_inactive_call() {
    let service = NetworkApprovalService::default();
    let cancellation_token =
        register_call_with_default_shell_trigger(&service, "registration-1").await;
    service.unregister_call("registration-1").await;

    service
        .record_call_outcome(
            "registration-1",
            NetworkApprovalOutcome::DeniedByPolicy("network denied".to_string()),
        )
        .await;

    assert!(!cancellation_token.is_cancelled());
    assert_eq!(service.take_call_outcome("registration-1").await, None);
}

#[tokio::test]
async fn record_blocked_request_ignores_ambiguous_unattributed_blocked_requests() {
    let service = NetworkApprovalService::default();
    register_call_with_default_shell_trigger(&service, "registration-1").await;
    register_call_with_default_shell_trigger(&service, "registration-2").await;

    service
        .record_blocked_request(denied_blocked_request("example.com"))
        .await;

    assert_eq!(service.take_call_outcome("registration-1").await, None);
    assert_eq!(service.take_call_outcome("registration-2").await, None);
}

#[tokio::test]
async fn attributed_blocked_request_targets_one_of_multiple_active_calls() {
    let service = NetworkApprovalService::default();
    let first = register_call_with_default_shell_trigger(&service, "registration-1").await;
    let second = register_call_with_default_shell_trigger(&service, "registration-2").await;

    service
        .record_blocked_request(denied_blocked_request_for_execution(
            "example.com",
            "registration-2",
        ))
        .await;

    assert!(!first.is_cancelled());
    assert!(second.is_cancelled());
    assert_eq!(service.take_call_outcome("registration-1").await, None);
    assert_eq!(
        service.take_call_outcome("registration-2").await,
        Some(NetworkApprovalOutcome::DeniedByPolicy(
            "Network access to \"example.com\" was blocked: domain is not on the allowlist for the current sandbox mode.".to_string()
        ))
    );
}
