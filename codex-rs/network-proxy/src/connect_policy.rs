use crate::network_policy::BlockDecisionAuditEventArgs;
use crate::network_policy::NetworkDecisionSource;
use crate::network_policy::NetworkPolicyDecision;
use crate::network_policy::emit_block_decision_audit_event;
use crate::policy::is_non_public_ip;
use crate::reasons::REASON_POLICY_DENIED;
use crate::runtime::HostAuthorization;
use crate::runtime::HostBlockDecision;
use crate::state::BlockedRequest;
use crate::state::BlockedRequestArgs;
use crate::state::NetworkProxyState;
#[cfg(target_os = "macos")]
use crate::system_dns::SystemDnsResolver;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::error::ErrorExt as _;
use rama_core::error::OpaqueError;
use rama_core::extensions::ExtensionsMut;
use rama_net::address::Host;
use rama_net::address::HostWithPort;
use rama_net::address::ProxyAddress;
use rama_net::client::EstablishedClientConnection;
use rama_net::transport::TryRefIntoTransportContext;
use rama_tcp::TcpStream;
use rama_tcp::client::TcpStreamConnector;
use rama_tcp::client::service::TcpConnector;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

const BLOCKED: u8 = 1;
const DIALED: u8 = 2;

#[derive(Debug, thiserror::Error)]
#[error("all resolved addresses were blocked by network policy")]
pub(crate) struct ResolvedTargetsDenied;

impl ResolvedTargetsDenied {
    pub(crate) fn caused(error: &(dyn Error + 'static)) -> bool {
        let mut cause = Some(error);
        while let Some(error) = cause {
            if error.is::<Self>() {
                return true;
            }
            cause = error.source();
        }
        false
    }
}

/// Metadata from the trusted admission decision, scoped to its logical destination.
#[derive(Clone, Debug)]
pub(crate) struct DialPolicy {
    target: HostWithPort,
    authorization: HostAuthorization,
    protocol: crate::network_policy::NetworkProtocol,
    client: Option<String>,
    method: Option<String>,
}
impl DialPolicy {
    pub(crate) fn new(
        target: &HostWithPort,
        authorization: HostAuthorization,
        request: &crate::network_policy::NetworkPolicyRequest,
    ) -> Self {
        Self {
            target: target.clone(),
            authorization,
            protocol: request.protocol,
            client: request.client_addr.clone(),
            method: request.method.clone(),
        }
    }
    // MITM can drop the DNS root dot while reconstructing the dial authority.
    fn for_target<'a>(
        extensions: &'a rama_core::extensions::Extensions,
        target: &HostWithPort,
    ) -> Option<&'a Self> {
        extensions.get::<Self>().filter(|policy| {
            policy.target.port == target.port
                && match (&policy.target.host, &target.host) {
                    (Host::Name(a), Host::Name(b)) => a
                        .as_str()
                        .trim_end_matches('.')
                        .eq_ignore_ascii_case(b.as_str().trim_end_matches('.')),
                    (a, b) => a == b,
                }
        })
    }

    pub(crate) fn authorization(
        extensions: &rama_core::extensions::Extensions,
        target: &HostWithPort,
    ) -> HostAuthorization {
        Self::for_target(extensions, target)
            .map_or(HostAuthorization::RequireAllowlist, |p| p.authorization)
    }
}

/// Connects traffic whose host policy has already allowed the request.
/// Apply address-specific restrictions at the actual direct dial, including DNS results.
#[derive(Clone)]
pub(crate) struct TargetCheckedTcpConnector {
    state: Arc<NetworkProxyState>,
}

impl TargetCheckedTcpConnector {
    pub(crate) fn new(state: Arc<NetworkProxyState>) -> Self {
        Self { state }
    }
}

impl<Input> Service<Input> for TargetCheckedTcpConnector
where
    Input: TryRefIntoTransportContext + Send + ExtensionsMut + 'static,
    Input::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = EstablishedClientConnection<TcpStream, Input>;
    type Error = BoxError;

    async fn serve(&self, input: Input) -> Result<Self::Output, Self::Error> {
        let connector = TcpConnector::new();
        #[cfg(target_os = "macos")]
        let connector = connector.with_dns(SystemDnsResolver);

        if input.extensions().get::<ProxyAddress>().is_some() {
            return connector.serve(input).await;
        }

        let target = input
            .try_ref_into_transport_ctx()
            .map_err(|err| OpaqueError::from_boxed(err.into()).context("read network target"))?
            .host_with_port()
            .ok_or_else(|| OpaqueError::from_display("network target is missing a port"))?;

        let policy = DialPolicy::for_target(input.extensions(), &target).cloned();
        let attempts = Arc::new(AtomicU8::new(0));
        let result = connector
            .with_connector(TargetCheckedStreamConnector {
                state: self.state.clone(),
                authorization: policy
                    .as_ref()
                    .map_or(HostAuthorization::RequireAllowlist, |policy| {
                        policy.authorization
                    }),
                attempts: attempts.clone(),
                target,
            })
            .serve(input)
            .await;
        if result.is_err()
            && attempts.load(Ordering::Relaxed) == BLOCKED
            && let Some(policy) = policy
        {
            let host = crate::policy::normalize_host(&policy.target.host.to_string());
            emit_block_decision_audit_event(
                &self.state,
                BlockDecisionAuditEventArgs {
                    source: NetworkDecisionSource::BaselinePolicy,
                    reason: REASON_POLICY_DENIED,
                    protocol: policy.protocol,
                    server_address: &host,
                    server_port: policy.target.port,
                    method: policy.method.as_deref(),
                    client_addr: policy.client.as_deref(),
                },
            );
            let _ = self
                .state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host,
                    port: Some(policy.target.port),
                    client: policy.client,
                    method: policy.method,
                    mode: None,
                    protocol: policy.protocol.as_policy_protocol().into(),
                    reason: REASON_POLICY_DENIED.into(),
                    decision: Some(NetworkPolicyDecision::Deny.as_str().into()),
                    source: Some(NetworkDecisionSource::BaselinePolicy.as_str().into()),
                }))
                .await;
            return Err(ResolvedTargetsDenied.into());
        }
        result
    }
}

#[derive(Clone)]
struct TargetCheckedStreamConnector {
    authorization: HostAuthorization,
    attempts: Arc<AtomicU8>,
    state: Arc<NetworkProxyState>,
    target: HostWithPort,
}

impl TcpStreamConnector for TargetCheckedStreamConnector {
    type Error = BoxError;

    async fn connect(&self, addr: SocketAddr) -> Result<TcpStream, Self::Error> {
        if self
            .state
            .resolved_target_blocked(&self.target.host.to_string(), addr.ip(), self.authorization)
            .await?
            || (is_non_public_ip(addr.ip()) && !self.allows_non_public_target(addr).await?)
        {
            self.attempts.fetch_or(BLOCKED, Ordering::Relaxed);
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "network target rejected by policy",
            )
            .into());
        }

        self.attempts.fetch_or(DIALED, Ordering::Relaxed);
        tokio::net::TcpStream::connect(addr)
            .await
            .map(TcpStream::from)
            .map_err(Into::into)
    }
}

impl TargetCheckedStreamConnector {
    async fn allows_non_public_target(&self, addr: SocketAddr) -> Result<bool, BoxError> {
        if self.state.allow_local_binding().await.map_err(|err| {
            let err: BoxError = err.into();
            OpaqueError::from_boxed(err)
                .context("read network proxy config")
                .into_boxed()
        })? {
            return Ok(true);
        }

        if !target_matches_non_public_addr(&self.target.host, addr.ip()) {
            return Ok(false);
        }

        self.state
            .host_blocked_with_local_binding(
                &self.target.host.to_string(),
                self.target.port,
                /*allow_local_binding*/ None,
                self.authorization,
            )
            .await
            .map(|decision| decision == HostBlockDecision::Allowed)
            .map_err(|err| {
                let err: BoxError = err.into();
                OpaqueError::from_boxed(err)
                    .context("evaluate network proxy target")
                    .into_boxed()
            })
    }
}

pub(crate) fn is_non_public_target(host: &Host) -> bool {
    match host {
        Host::Address(ip) => is_non_public_ip(*ip),
        Host::Name(name) => name
            .as_str()
            .trim_end_matches('.')
            .eq_ignore_ascii_case("localhost"),
    }
}

fn target_matches_non_public_addr(host: &Host, addr: std::net::IpAddr) -> bool {
    match host {
        Host::Address(ip) => *ip == addr,
        Host::Name(name) => {
            name.as_str()
                .trim_end_matches('.')
                .eq_ignore_ascii_case("localhost")
                && addr.is_loopback()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkProxyConfig;
    use crate::state::network_proxy_state_for_policy;
    use pretty_assertions::assert_eq;
    use rama_core::extensions::ExtensionsRef;
    use rama_net::address::HostWithPort;
    use std::net::Ipv4Addr;
    use tokio::net::TcpListener;

    #[tokio::test(flavor = "current_thread")]
    async fn direct_connector_rejects_non_public_target_when_local_binding_disabled() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind local listener");
        let target = listener.local_addr().expect("local addr");
        let connector = TargetCheckedTcpConnector::new(Arc::new(network_proxy_state_for_policy(
            NetworkProxyConfig::default(),
        )));

        let request: rama_tcp::client::Request =
            rama_tcp::client::Request::new(HostWithPort::from(target));
        let err = Service::serve(&connector, request)
            .await
            .expect_err("local target should be rejected");

        assert!(
            format!("{err:?}").contains("network target rejected by policy"),
            "unexpected error: {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn direct_connector_allows_non_public_target_when_local_binding_enabled() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind local listener");
        let target = listener.local_addr().expect("local addr");
        let connector = TargetCheckedTcpConnector::new(Arc::new(network_proxy_state_for_policy(
            NetworkProxyConfig {
                allow_local_binding: Some(true),
                ..NetworkProxyConfig::default()
            },
        )));

        let request: rama_tcp::client::Request =
            rama_tcp::client::Request::new(HostWithPort::from(target));
        let result = Service::serve(&connector, request).await;

        assert!(result.is_ok(), "local target should be allowed: {result:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn direct_connector_allows_explicitly_allowlisted_non_public_target() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind local listener");
        let target = listener.local_addr().expect("local addr");
        let mut config = NetworkProxyConfig::default();
        config.set_allowed_domains(vec![target.ip().to_string()]);
        let state = Arc::new(network_proxy_state_for_policy(config));
        let connector = TargetCheckedTcpConnector::new(Arc::clone(&state));

        let request: rama_tcp::client::Request =
            rama_tcp::client::Request::new(HostWithPort::from(target));
        let result = Service::serve(&connector, request).await;

        assert!(
            result.is_ok(),
            "explicitly allowlisted local target should be allowed: {result:?}"
        );
        state
            .add_denied_domain(&target.ip().to_string())
            .await
            .unwrap();
        let request = rama_tcp::client::Request::new(HostWithPort::from(target));
        assert!(Service::serve(&connector, request).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn direct_connector_allows_explicitly_allowlisted_localhost_target() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind local listener");
        let target = listener.local_addr().expect("local addr");
        let mut config = NetworkProxyConfig::default();
        config.set_allowed_domains(vec!["localhost".to_string()]);
        let connector =
            TargetCheckedTcpConnector::new(Arc::new(network_proxy_state_for_policy(config)));

        let request: rama_tcp::client::Request =
            rama_tcp::client::Request::new(HostWithPort::new(Host::LOCALHOST_NAME, target.port()));
        let result = Service::serve(&connector, request).await;

        assert!(
            result.is_ok(),
            "explicitly allowlisted localhost target should be allowed: {result:?}"
        );
    }

    #[tokio::test]
    async fn policy_refresh_cannot_grant_a_missing_dial_approval() {
        use crate::network_policy::NetworkDecision;
        use crate::network_policy::NetworkPolicyDecider;
        use crate::network_policy::NetworkPolicyRequest;
        use crate::network_policy::NetworkPolicyRequestArgs;
        use crate::network_policy::NetworkProtocol;
        use crate::network_policy::evaluate_host_policy_for_dial;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let target = HostWithPort::new(Host::LOCALHOST_NAME, addr.port());
        let mut config = NetworkProxyConfig {
            allow_local_binding: Some(true),
            ..Default::default()
        };
        config.set_allowed_domains(vec!["localhost".into()]);
        config.review_domains = vec![addr.ip().to_string(), "::1".into()];
        let state = Arc::new(network_proxy_state_for_policy(config.clone()));
        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            host: "localhost".into(),
            port: addr.port(),
            protocol: NetworkProtocol::Socks5Tcp,
            environment_id: None,
            client_addr: None,
            method: None,
            command: None,
            exec_policy_hint: None,
        });
        let admitted_target =
            HostWithPort::new(Host::Name("LOCALHOST.".parse().unwrap()), addr.port());
        let admit = async |decider: Option<Arc<dyn NetworkPolicyDecider>>| {
            let mut req = rama_tcp::client::Request::new(target.clone());
            let decision = evaluate_host_policy_for_dial(
                &state,
                decider.as_ref(),
                &request,
                Some((&admitted_target, req.extensions_mut())),
            )
            .await
            .unwrap();
            assert_eq!(decision, NetworkDecision::Allow);
            req
        };
        let assert_addresses_blocked = async |authorization| {
            let dial = TargetCheckedStreamConnector {
                state: state.clone(),
                target: target.clone(),
                attempts: Arc::default(),
                authorization,
            };
            for ip in [addr.ip(), Ipv4Addr::LOCALHOST.to_ipv6_mapped().into()] {
                let error = dial
                    .connect(SocketAddr::new(ip, addr.port()))
                    .await
                    .unwrap_err();
                assert_eq!(
                    error.downcast_ref::<io::Error>().unwrap().kind(),
                    io::ErrorKind::PermissionDenied
                );
            }
        };
        let original = admit(None).await;
        assert_addresses_blocked(HostAuthorization::RequireAllowlist).await;
        config.review_domains.push("localhost".into());
        state
            .replace_config_state(
                crate::build_config_state(config, Default::default(), crate::Platform::native())
                    .unwrap(),
            )
            .await
            .unwrap();
        let approved = admit(Some(Arc::new(|_| async { NetworkDecision::Allow }))).await;
        let other_port = HostWithPort::new(target.host.clone(), addr.port() ^ 1);
        assert!(DialPolicy::for_target(approved.extensions(), &other_port).is_none());
        let connector = TargetCheckedTcpConnector::new(state.clone());
        assert!(connector.serve(original).await.is_err());
        assert!(connector.serve(approved).await.is_ok());
        state
            .add_denied_domain(&addr.ip().to_string())
            .await
            .unwrap();
        assert_addresses_blocked(HostAuthorization::Approved).await;
    }

    #[test]
    fn resolved_private_address_does_not_match_allowlisted_hostname() {
        let host = Host::Name("example.com".parse().expect("valid domain"));

        assert!(!target_matches_non_public_addr(
            &host,
            Ipv4Addr::LOCALHOST.into()
        ));
    }
}
