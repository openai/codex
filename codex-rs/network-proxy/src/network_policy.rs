use crate::state::AppState;
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

#[derive(Clone, Debug)]
pub struct NetworkPolicyRequest {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
}

impl NetworkPolicyRequest {
    #[must_use]
    pub fn new(
        protocol: NetworkProtocol,
        host: String,
        port: u16,
        client_addr: Option<String>,
        method: Option<String>,
        command: Option<String>,
        exec_policy_hint: Option<String>,
    ) -> Self {
        Self {
            protocol,
            host,
            port,
            client_addr,
            method,
            command,
            exec_policy_hint,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NetworkDecision {
    Allow,
    Deny { reason: String },
}

impl NetworkDecision {
    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        let reason = if reason.is_empty() {
            "policy_denied".to_string()
        } else {
            reason
        };
        Self::Deny { reason }
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
    state: &AppState,
    decider: Option<&Arc<dyn NetworkPolicyDecider>>,
    request: &NetworkPolicyRequest,
) -> Result<NetworkDecision> {
    let (blocked, reason) = state.host_blocked(&request.host, request.port).await?;
    if !blocked {
        return Ok(NetworkDecision::Allow);
    }

    if reason == "not_allowed"
        && let Some(decider) = decider
    {
        return Ok(decider.decide(request.clone()).await);
    }

    Ok(NetworkDecision::deny(reason))
}
