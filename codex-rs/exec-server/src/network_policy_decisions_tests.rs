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
use crate::protocol::ExecServerNetworkPolicyDecision;
use crate::protocol::NetworkPolicyRequestParams;
use crate::protocol::NetworkPolicyRequestResponse;
use crate::rpc::RpcServerOutboundMessage;

struct DeciderHarness {
    requests: RpcServerRequestSender,
    outgoing: mpsc::Receiver<RpcServerOutboundMessage>,
    process_shutdown: CancellationToken,
}

impl DeciderHarness {
    fn new() -> Self {
        let (outgoing_tx, outgoing) = mpsc::channel(/*buffer*/ 8);
        Self {
            requests: RpcServerRequestSender::new(outgoing_tx),
            outgoing,
            process_shutdown: CancellationToken::new(),
        }
    }

    fn request(&self, host: &str) -> JoinHandle<NetworkDecision> {
        let decider = network_policy_decider(
            ProcessId::from("process"),
            Arc::new(RwLock::new(Some(self.requests.clone()))),
            self.process_shutdown.clone(),
        );
        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::HttpsConnect,
            host: host.to_string(),
            port: 443,
            environment_id: None,
            client_addr: None,
            method: None,
            command: None,
            exec_policy_hint: None,
        });
        tokio::spawn(async move { decider.decide(request).await })
    }

    async fn next_request(
        &mut self,
    ) -> (
        codex_exec_server_protocol::RequestId,
        NetworkPolicyRequestParams,
    ) {
        let outbound = timeout(Duration::from_secs(1), self.outgoing.recv())
            .await
            .expect("policy request should arrive")
            .expect("policy request");
        let RpcServerOutboundMessage::Request(request) = outbound else {
            panic!("expected policy request");
        };
        assert_eq!(request.method, NETWORK_POLICY_REQUEST_METHOD);
        let params = serde_json::from_value(request.params.expect("request params"))
            .expect("deserialize policy request");
        (request.id, params)
    }
}

async fn await_decision(decision: JoinHandle<NetworkDecision>) -> NetworkDecision {
    timeout(Duration::from_secs(1), decision)
        .await
        .expect("network policy decision should resolve")
        .expect("network policy decision task")
}

#[tokio::test]
async fn returns_client_policy_decision() {
    let mut harness = DeciderHarness::new();
    let decision = harness.request("example.com");
    let (request_id, params) = harness.next_request().await;
    assert_eq!(params.process_id, ProcessId::from("process"));
    assert_eq!(params.request.host, "example.com");
    harness.requests.complete(
        request_id,
        Ok(serde_json::to_value(NetworkPolicyRequestResponse {
            decision: ExecServerNetworkPolicyDecision::Allow,
        })
        .expect("serialize policy response")),
    );

    assert_eq!(await_decision(decision).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn policy_response_reasons_are_bounded_and_fail_closed() {
    let mut harness = DeciderHarness::new();
    let boundary_reason = "d".repeat(MAX_NETWORK_POLICY_REASON_BYTES);
    let cases = [
        (
            ExecServerNetworkPolicyDecision::Deny {
                reason: boundary_reason.clone(),
            },
            NetworkDecision::deny(boundary_reason),
        ),
        (
            ExecServerNetworkPolicyDecision::Deny {
                reason: "d".repeat(MAX_NETWORK_POLICY_REASON_BYTES + 1),
            },
            NetworkDecision::deny("not_allowed"),
        ),
        (
            ExecServerNetworkPolicyDecision::Ask {
                reason: "ask permission".to_string(),
            },
            NetworkDecision::ask("ask permission"),
        ),
        (
            ExecServerNetworkPolicyDecision::Ask {
                reason: "ask\npermission".to_string(),
            },
            NetworkDecision::deny("not_allowed"),
        ),
    ];

    for (response, expected) in cases {
        let decision = harness.request("example.com");
        let (request_id, _) = harness.next_request().await;
        harness.requests.complete(
            request_id,
            Ok(
                serde_json::to_value(NetworkPolicyRequestResponse { decision: response })
                    .expect("serialize policy response"),
            ),
        );
        assert_eq!(await_decision(decision).await, expected);
    }
}

#[tokio::test]
async fn boundary_host_is_relayed() {
    let mut harness = DeciderHarness::new();
    let host = "h".repeat(MAX_NETWORK_POLICY_HOST_BYTES);
    let decision = harness.request(&host);
    let (request_id, params) = harness.next_request().await;
    assert_eq!(params.request.host, host);
    harness.requests.complete(
        request_id,
        Ok(serde_json::to_value(NetworkPolicyRequestResponse {
            decision: ExecServerNetworkPolicyDecision::Allow,
        })
        .expect("serialize policy response")),
    );

    assert_eq!(await_decision(decision).await, NetworkDecision::Allow);
}

#[tokio::test]
async fn invalid_hosts_fail_closed_before_reverse_rpc() {
    let mut harness = DeciderHarness::new();
    let invalid_hosts = [
        String::new(),
        "host name".to_string(),
        "host\u{0000}name".to_string(),
        "h".repeat(MAX_NETWORK_POLICY_HOST_BYTES + 1),
    ];

    for host in invalid_hosts {
        assert_eq!(
            await_decision(harness.request(&host)).await,
            NetworkDecision::deny("not_allowed")
        );
        assert!(harness.outgoing.try_recv().is_err());
        assert_eq!(harness.requests.pending_request_count(), 0);
    }
}

#[tokio::test]
async fn process_exit_and_disconnect_fail_closed() {
    let mut process_exit = DeciderHarness::new();
    let process_decision = process_exit.request("process-exit.example.com");
    process_exit.next_request().await;
    process_exit.process_shutdown.cancel();
    assert_eq!(
        await_decision(process_decision).await,
        NetworkDecision::deny("not_allowed")
    );
    assert_eq!(process_exit.requests.pending_request_count(), 0);

    let mut disconnect = DeciderHarness::new();
    let disconnect_decision = disconnect.request("disconnect.example.com");
    disconnect.next_request().await;
    disconnect.requests.close();
    assert_eq!(
        await_decision(disconnect_decision).await,
        NetworkDecision::deny("not_allowed")
    );
    assert_eq!(disconnect.requests.pending_request_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn decision_timeout_fails_closed() {
    let mut harness = DeciderHarness::new();
    let decision = harness.request("timeout.example.com");
    harness.next_request().await;

    tokio::time::advance(Duration::from_secs(99)).await;
    assert!(!decision.is_finished());

    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        await_decision(decision).await,
        NetworkDecision::deny("not_allowed")
    );
    assert_eq!(harness.requests.pending_request_count(), 0);
}
