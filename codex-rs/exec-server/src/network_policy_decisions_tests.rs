use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyRequest;
use codex_network_proxy::NetworkPolicyRequestArgs;
use codex_network_proxy::NetworkProtocol;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::*;
use crate::protocol::NETWORK_POLICY_REQUEST_METHOD;
use crate::rpc::RpcServerOutboundMessage;

const NOTIFICATION_CHANNEL_CAPACITY: usize = 8;

fn network_policy_request(host: &str) -> NetworkPolicyRequest {
    NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
        protocol: NetworkProtocol::HttpsConnect,
        host: host.to_string(),
        port: 443,
        environment_id: None,
        client_addr: None,
        method: None,
        command: None,
        exec_policy_hint: None,
    })
}

async fn next_network_policy_request(
    outgoing_rx: &mut mpsc::Receiver<RpcServerOutboundMessage>,
) -> NetworkPolicyRequestNotification {
    let outbound = timeout(Duration::from_secs(1), outgoing_rx.recv())
        .await
        .expect("policy request notification should arrive")
        .expect("policy request notification");
    let RpcServerOutboundMessage::Notification(notification) = outbound else {
        panic!("expected policy request notification");
    };
    assert_eq!(notification.method, NETWORK_POLICY_REQUEST_METHOD);
    serde_json::from_value(
        notification
            .params
            .expect("policy request notification params"),
    )
    .expect("deserialize policy request notification")
}

fn network_policy_relay() -> (
    Arc<NetworkPolicyDecisionRelay>,
    Arc<RwLock<Option<RpcNotificationSender>>>,
    mpsc::Receiver<RpcServerOutboundMessage>,
) {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(NOTIFICATION_CHANNEL_CAPACITY);
    (
        Arc::new(NetworkPolicyDecisionRelay::default()),
        Arc::new(RwLock::new(Some(RpcNotificationSender::new(outgoing_tx)))),
        outgoing_rx,
    )
}

fn bounded_network_policy_relay(
    decision_timeout: Duration,
    max_pending: usize,
) -> (
    Arc<NetworkPolicyDecisionRelay>,
    Arc<RwLock<Option<RpcNotificationSender>>>,
    mpsc::Receiver<RpcServerOutboundMessage>,
) {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(NOTIFICATION_CHANNEL_CAPACITY);
    (
        Arc::new(NetworkPolicyDecisionRelay::with_limits(
            decision_timeout,
            max_pending,
        )),
        Arc::new(RwLock::new(Some(RpcNotificationSender::new(outgoing_tx)))),
        outgoing_rx,
    )
}

fn request_decision(
    relay: &Arc<NetworkPolicyDecisionRelay>,
    notifications: &Arc<RwLock<Option<RpcNotificationSender>>>,
    process_id: ProcessId,
    host: &str,
) -> JoinHandle<NetworkDecision> {
    let decider = relay.decider(process_id, Arc::clone(notifications));
    let request = network_policy_request(host);
    tokio::spawn(async move { decider.decide(request).await })
}

async fn await_decision(decision: JoinHandle<NetworkDecision>) -> NetworkDecision {
    timeout(Duration::from_secs(1), decision)
        .await
        .expect("network policy decision should resolve")
        .expect("network policy decision task")
}

#[tokio::test]
async fn decisions_correlate_concurrent_requests() {
    let (relay, notifications, mut outgoing_rx) = network_policy_relay();
    let process_a = ProcessId::from("network-decision-a");
    let process_b = ProcessId::from("network-decision-b");
    let decision_a = request_decision(&relay, &notifications, process_a.clone(), "a.example.com");
    let decision_b = request_decision(&relay, &notifications, process_b.clone(), "b.example.com");
    let first = next_network_policy_request(&mut outgoing_rx).await;
    let second = next_network_policy_request(&mut outgoing_rx).await;
    let (request_a, request_b) = if first.process_id == process_a {
        (first, second)
    } else {
        (second, first)
    };
    assert_eq!(request_a.request.host, "a.example.com");
    assert_eq!(request_b.request.host, "b.example.com");

    let error = relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: request_a.request_id.clone(),
            process_id: process_b.clone(),
            decision: NetworkDecision::Allow,
        })
        .expect_err("mismatched process must not resolve request");
    assert_eq!(
        error,
        "network policy decision process id does not match request"
    );

    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: request_b.request_id,
            process_id: process_b,
            decision: NetworkDecision::Allow,
        })
        .expect("resolve second network policy decision");
    let denied = NetworkDecision::deny("not_allowed");
    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: request_a.request_id,
            process_id: process_a,
            decision: denied.clone(),
        })
        .expect("resolve first network policy decision");
    assert_eq!(await_decision(decision_a).await, denied);
    assert_eq!(await_decision(decision_b).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn decisions_fail_closed_on_disconnect() {
    let (relay, notifications, mut outgoing_rx) = network_policy_relay();
    let decision = request_decision(
        &relay,
        &notifications,
        ProcessId::from("network-decision-disconnect"),
        "disconnect.example.com",
    );
    next_network_policy_request(&mut outgoing_rx).await;

    *notifications
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    relay.fail_pending(/*process_id*/ None);

    assert_eq!(
        await_decision(decision).await,
        NetworkDecision::deny("not_allowed")
    );
}

#[tokio::test]
async fn process_exit_fails_only_its_decisions() {
    let (relay, notifications, mut outgoing_rx) = network_policy_relay();
    let process_a = ProcessId::from("network-decision-exit-a");
    let process_b = ProcessId::from("network-decision-exit-b");
    let decision_a = request_decision(
        &relay,
        &notifications,
        process_a.clone(),
        "exit-a.example.com",
    );
    let mut decision_b = request_decision(
        &relay,
        &notifications,
        process_b.clone(),
        "exit-b.example.com",
    );
    let first = next_network_policy_request(&mut outgoing_rx).await;
    let second = next_network_policy_request(&mut outgoing_rx).await;
    let request_b = if first.process_id == process_b {
        first
    } else {
        second
    };

    relay.fail_pending(Some(&process_a));

    assert_eq!(
        await_decision(decision_a).await,
        NetworkDecision::deny("not_allowed")
    );
    assert!(
        timeout(Duration::from_millis(20), &mut decision_b)
            .await
            .is_err(),
        "another process decision should remain pending"
    );
    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: request_b.request_id,
            process_id: process_b,
            decision: NetworkDecision::Allow,
        })
        .expect("resolve remaining process decision");
    assert_eq!(await_decision(decision_b).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn pending_decisions_are_capacity_bounded() {
    let (relay, notifications, mut outgoing_rx) =
        bounded_network_policy_relay(Duration::from_secs(1), /*max_pending*/ 1);
    let process_id = ProcessId::from("network-decision-capacity");
    let first = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "first.example.com",
    );
    let first_request = next_network_policy_request(&mut outgoing_rx).await;

    let second = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "second.example.com",
    );
    assert_eq!(
        await_decision(second).await,
        NetworkDecision::deny("not_allowed")
    );
    assert!(
        timeout(Duration::from_millis(20), outgoing_rx.recv())
            .await
            .is_err(),
        "a rejected request must not emit a notification"
    );

    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: first_request.request_id,
            process_id,
            decision: NetworkDecision::Allow,
        })
        .expect("resolve first network policy decision");
    assert_eq!(await_decision(first).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn pending_decisions_time_out_and_release_capacity() {
    let (relay, notifications, mut outgoing_rx) =
        bounded_network_policy_relay(Duration::from_millis(20), /*max_pending*/ 1);
    let process_id = ProcessId::from("network-decision-timeout");
    let timed_out = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "timeout.example.com",
    );
    let timed_out_request = next_network_policy_request(&mut outgoing_rx).await;
    assert_eq!(
        await_decision(timed_out).await,
        NetworkDecision::deny("not_allowed")
    );
    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: timed_out_request.request_id,
            process_id: process_id.clone(),
            decision: NetworkDecision::Allow,
        })
        .expect("late decisions are ignored");

    let next = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "next.example.com",
    );
    let next_request = next_network_policy_request(&mut outgoing_rx).await;
    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: next_request.request_id,
            process_id,
            decision: NetworkDecision::Allow,
        })
        .expect("resolve next network policy decision");
    assert_eq!(await_decision(next).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn cancelling_a_pending_decision_releases_capacity() {
    let (relay, notifications, mut outgoing_rx) =
        bounded_network_policy_relay(Duration::from_secs(1), /*max_pending*/ 1);
    let process_id = ProcessId::from("network-decision-cancelled");
    let cancelled = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "cancelled.example.com",
    );
    next_network_policy_request(&mut outgoing_rx).await;
    cancelled.abort();
    let _ = cancelled.await;

    let next = request_decision(
        &relay,
        &notifications,
        process_id.clone(),
        "next.example.com",
    );
    let next_request = next_network_policy_request(&mut outgoing_rx).await;
    relay
        .resolve(NetworkPolicyDecisionNotification {
            request_id: next_request.request_id,
            process_id,
            decision: NetworkDecision::Allow,
        })
        .expect("resolve next network policy decision");
    assert_eq!(await_decision(next).await, NetworkDecision::Allow);
}
