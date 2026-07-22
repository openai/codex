use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkPolicyRequest;
use codex_network_proxy::NetworkProtocol;
use tokio_util::sync::CancellationToken;

use crate::ProcessId;
use crate::protocol::ExecServerNetworkPolicyDecision;
use crate::protocol::ExecServerNetworkPolicyRequest;
use crate::protocol::ExecServerNetworkProtocol;
use crate::protocol::MAX_NETWORK_POLICY_HOST_BYTES;
use crate::protocol::MAX_NETWORK_POLICY_REASON_BYTES;
use crate::protocol::NETWORK_POLICY_REQUEST_METHOD;
use crate::protocol::NetworkPolicyRequestParams;
use crate::protocol::NetworkPolicyRequestResponse;
use crate::rpc_server_requests::RpcServerRequestSender;

// Leave transport overhead outside the client-side 95-second decision window.
const NETWORK_POLICY_REQUEST_TIMEOUT: Duration = Duration::from_secs(100);

pub(crate) fn network_policy_decider(
    process_id: ProcessId,
    requests: Arc<RwLock<Option<RpcServerRequestSender>>>,
    process_shutdown: CancellationToken,
) -> Arc<dyn NetworkPolicyDecider> {
    Arc::new(move |request: NetworkPolicyRequest| {
        let process_id = process_id.clone();
        let requests = Arc::clone(&requests);
        let process_shutdown = process_shutdown.clone();
        async move {
            let host = request.host.as_str();
            if host.is_empty()
                || host.len() > MAX_NETWORK_POLICY_HOST_BYTES
                || host.chars().any(char::is_control)
                || host.chars().any(char::is_whitespace)
            {
                return NetworkDecision::deny("not_allowed");
            }
            let requests = requests
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let Some(requests) = requests else {
                return NetworkDecision::deny("not_allowed");
            };
            let params = NetworkPolicyRequestParams {
                process_id,
                request: ExecServerNetworkPolicyRequest {
                    protocol: match request.protocol {
                        NetworkProtocol::Http => ExecServerNetworkProtocol::Http,
                        NetworkProtocol::HttpsConnect => ExecServerNetworkProtocol::HttpsConnect,
                        NetworkProtocol::Socks5Tcp => ExecServerNetworkProtocol::Socks5Tcp,
                        NetworkProtocol::Socks5Udp => ExecServerNetworkProtocol::Socks5Udp,
                    },
                    host: request.host,
                    port: request.port,
                },
            };
            tokio::select! {
                biased;
                _ = process_shutdown.cancelled() => NetworkDecision::deny("not_allowed"),
                response = requests.call_with_timeout::<_, NetworkPolicyRequestResponse>(
                    NETWORK_POLICY_REQUEST_METHOD,
                    &params,
                    NETWORK_POLICY_REQUEST_TIMEOUT,
                ) => response
                    .map(|response| match response.decision {
                        ExecServerNetworkPolicyDecision::Allow => NetworkDecision::Allow,
                        ExecServerNetworkPolicyDecision::Deny { reason }
                        | ExecServerNetworkPolicyDecision::Ask { reason }
                            if reason.len() > MAX_NETWORK_POLICY_REASON_BYTES
                                || reason.chars().any(char::is_control) =>
                        {
                            NetworkDecision::deny("not_allowed")
                        }
                        ExecServerNetworkPolicyDecision::Deny { reason } => {
                            NetworkDecision::deny(reason)
                        }
                        ExecServerNetworkPolicyDecision::Ask { reason } => {
                            NetworkDecision::ask(reason)
                        }
                    })
                    .unwrap_or_else(|_| NetworkDecision::deny("not_allowed")),
            }
        }
    })
}

#[cfg(test)]
#[path = "network_policy_decisions_tests.rs"]
mod tests;
