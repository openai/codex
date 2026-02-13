use crate::reasons::REASON_POLICY_DENIED;
use crate::runtime::HostBlockDecision;
use crate::runtime::HostBlockReason;
use crate::state::NetworkProxyState;
use anyhow::Result;
use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkProtocol {
    Http,
    HttpsConnect,
    Socks5Tcp,
    Socks5Udp,
}

impl NetworkProtocol {
    pub const fn as_policy_protocol(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::HttpsConnect => "https_connect",
            Self::Socks5Tcp => "socks5_tcp",
            Self::Socks5Udp => "socks5_udp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkPolicyDecision {
    Deny,
    Ask,
}

impl NetworkPolicyDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkDecisionSource {
    BaselinePolicy,
    ModeGuard,
    ProxyState,
    Decider,
}

impl NetworkDecisionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselinePolicy => "baseline_policy",
            Self::ModeGuard => "mode_guard",
            Self::ProxyState => "proxy_state",
            Self::Decider => "decider",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkPolicyRequest {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
    pub attempt_id: Option<String>,
}

pub struct NetworkPolicyRequestArgs {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
    pub attempt_id: Option<String>,
}

impl NetworkPolicyRequest {
    pub fn new(args: NetworkPolicyRequestArgs) -> Self {
        let NetworkPolicyRequestArgs {
            protocol,
            host,
            port,
            client_addr,
            method,
            command,
            exec_policy_hint,
            attempt_id,
        } = args;
        Self {
            protocol,
            host,
            port,
            client_addr,
            method,
            command,
            exec_policy_hint,
            attempt_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkDecision {
    Allow,
    Deny {
        reason: String,
        source: NetworkDecisionSource,
        decision: NetworkPolicyDecision,
    },
}

impl NetworkDecision {
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::deny_with_source(reason, NetworkDecisionSource::Decider)
    }

    pub fn ask(reason: impl Into<String>) -> Self {
        Self::ask_with_source(reason, NetworkDecisionSource::Decider)
    }

    pub fn deny_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        let reason = reason.into();
        let reason = if reason.is_empty() {
            REASON_POLICY_DENIED.to_string()
        } else {
            reason
        };
        Self::Deny {
            reason,
            source,
            decision: NetworkPolicyDecision::Deny,
        }
    }

    pub fn ask_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        let reason = reason.into();
        let reason = if reason.is_empty() {
            REASON_POLICY_DENIED.to_string()
        } else {
            reason
        };
        Self::Deny {
            reason,
            source,
            decision: NetworkPolicyDecision::Ask,
        }
    }
}

/// Decide whether a network request should be allowed.
///
/// If `command` or `exec_policy_hint` is provided, callers can map exec-policy
/// approvals to network access (e.g., allow all requests for commands matching
/// approved prefixes like `curl *`).
#[async_trait]
pub trait NetworkPolicyDecider: Send + Sync + 'static {
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision;
}

#[async_trait]
impl<D: NetworkPolicyDecider + ?Sized> NetworkPolicyDecider for Arc<D> {
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision {
        (**self).decide(req).await
    }
}

#[async_trait]
impl<F, Fut> NetworkPolicyDecider for F
where
    F: Fn(NetworkPolicyRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = NetworkDecision> + Send,
{
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision {
        (self)(req).await
    }
}

pub(crate) async fn evaluate_host_policy(
    state: &NetworkProxyState,
    decider: Option<&Arc<dyn NetworkPolicyDecider>>,
    request: &NetworkPolicyRequest,
) -> Result<NetworkDecision> {
    match state.host_blocked(&request.host, request.port).await? {
        HostBlockDecision::Allowed => Ok(NetworkDecision::Allow),
        HostBlockDecision::Blocked(HostBlockReason::NotAllowed) => {
            if let Some(decider) = decider {
                Ok(map_decider_decision(decider.decide(request.clone()).await))
            } else {
                Ok(NetworkDecision::deny_with_source(
                    HostBlockReason::NotAllowed.as_str(),
                    NetworkDecisionSource::BaselinePolicy,
                ))
            }
        }
        HostBlockDecision::Blocked(reason) => Ok(NetworkDecision::deny_with_source(
            reason.as_str(),
            NetworkDecisionSource::BaselinePolicy,
        )),
    }
}

fn map_decider_decision(decision: NetworkDecision) -> NetworkDecision {
    match decision {
        NetworkDecision::Allow => NetworkDecision::Allow,
        NetworkDecision::Deny {
            reason, decision, ..
        } => NetworkDecision::Deny {
            reason,
            source: NetworkDecisionSource::Decider,
            decision,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkProxySettings;
    use crate::reasons::REASON_DENIED;
    use crate::reasons::REASON_NOT_ALLOWED;
    use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
    use crate::state::network_proxy_state_for_policy;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn evaluate_host_policy_invokes_decider_for_not_allowed() {
        let state = network_proxy_state_for_policy(NetworkProxySettings::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                // The default policy denies all; the decider is consulted for not_allowed
                // requests and can override that decision.
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "example.com".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
            attempt_id: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(decision, NetworkDecision::Allow);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn evaluate_host_policy_skips_decider_for_denied() {
        let state = network_proxy_state_for_policy(NetworkProxySettings {
            allowed_domains: vec!["example.com".to_string()],
            denied_domains: vec!["blocked.com".to_string()],
            ..NetworkProxySettings::default()
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "blocked.com".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
            attempt_id: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(
            decision,
            NetworkDecision::Deny {
                reason: REASON_DENIED.to_string(),
                source: NetworkDecisionSource::BaselinePolicy,
                decision: NetworkPolicyDecision::Deny,
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn evaluate_host_policy_skips_decider_for_not_allowed_local() {
        let state = network_proxy_state_for_policy(NetworkProxySettings {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkProxySettings::default()
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
            attempt_id: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(
            decision,
            NetworkDecision::Deny {
                reason: REASON_NOT_ALLOWED_LOCAL.to_string(),
                source: NetworkDecisionSource::BaselinePolicy,
                decision: NetworkPolicyDecision::Deny,
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ask_uses_decider_source_and_ask_decision() {
        assert_eq!(
            NetworkDecision::ask(REASON_NOT_ALLOWED),
            NetworkDecision::Deny {
                reason: REASON_NOT_ALLOWED.to_string(),
                source: NetworkDecisionSource::Decider,
                decision: NetworkPolicyDecision::Ask,
            }
        );
    }
}
