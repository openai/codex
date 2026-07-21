use crate::policy::is_non_public_ip;
use crate::runtime::HostBlockDecision;
use crate::state::NetworkProxyState;
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
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

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
        if input.extensions().get::<ProxyAddress>().is_some() {
            return TcpConnector::new().serve(input).await;
        }

        let target = input
            .try_ref_into_transport_ctx()
            .map_err(|err| OpaqueError::from_boxed(err.into()).context("read network target"))?
            .host_with_port()
            .ok_or_else(|| OpaqueError::from_display("network target is missing a port"))?;

        TcpConnector::new()
            .with_connector(TargetCheckedStreamConnector {
                state: self.state.clone(),
                target,
            })
            .serve(input)
            .await
    }
}

#[derive(Clone)]
struct TargetCheckedStreamConnector {
    state: Arc<NetworkProxyState>,
    target: HostWithPort,
}

impl TcpStreamConnector for TargetCheckedStreamConnector {
    type Error = BoxError;

    async fn connect(&self, addr: SocketAddr) -> Result<TcpStream, Self::Error> {
        if is_non_public_ip(addr.ip()) && !self.allows_non_public_target(addr).await? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "network target rejected by policy",
            )
            .into());
        }

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
            .host_blocked(&self.target.host.to_string(), self.target.port)
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
                allow_local_binding: true,
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
        let connector =
            TargetCheckedTcpConnector::new(Arc::new(network_proxy_state_for_policy(config)));

        let request: rama_tcp::client::Request =
            rama_tcp::client::Request::new(HostWithPort::from(target));
        let result = Service::serve(&connector, request).await;

        assert!(
            result.is_ok(),
            "explicitly allowlisted local target should be allowed: {result:?}"
        );
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

    #[test]
    fn resolved_private_address_does_not_match_allowlisted_hostname() {
        let host = Host::Name("example.com".parse().expect("valid domain"));

        assert!(!target_matches_non_public_addr(
            &host,
            Ipv4Addr::LOCALHOST.into()
        ));
    }
}
