//! Mandatory next hop for opaque SOCKS TCP on executors provisioned with an HTTP CONNECT proxy.
//! Only the executor selects the endpoint. The sandbox supplies the already-authorized target.

use crate::connect_policy::TargetCheckedTcpConnector;
use crate::policy::is_non_public_ip;
use crate::policy::is_private_network_ip;
use crate::policy::normalize_host;
use crate::policy::numeric_host_address;
use crate::state::NetworkProxyState;
use anyhow::ensure;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_core::stream::rewind::Rewind;
use rama_http::Body;
use rama_http::Method;
use rama_http::Request;
use rama_http::Version;
use rama_http::header::HOST;
use rama_http::io::upgrade;
use rama_http_backend::client::HttpConnector;
use rama_net::Protocol;
use rama_net::address::HostWithPort;
use rama_net::address::ProxyAddress;
use rama_net::client::EstablishedClientConnection;
use rama_net::stream::ClientSocketInfo;
use rama_tcp::TcpStream;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// Trusted plaintext HTTP CONNECT listener, expressed as a hostname or IP and an explicit port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpUpstreamProxy(HostWithPort);

impl FromStr for TcpUpstreamProxy {
    type Err = anyhow::Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let endpoint: HostWithPort = raw.parse()?;
        ensure!(
            endpoint.port != 0,
            "TCP upstream proxy port must be nonzero"
        );
        Ok(Self(endpoint))
    }
}

impl TcpUpstreamProxy {
    pub(crate) async fn connect(
        &self,
        state: &Arc<NetworkProxyState>,
        target: &HostWithPort,
        authorization: crate::runtime::HostAuthorization,
    ) -> Result<Rewind<TcpStream>, BoxError> {
        if !state.allow_upstream_proxy().await? {
            return Err("upstream proxies are disabled by network policy".into());
        }

        let host = normalize_host(&target.host.to_string());
        let target_ip = numeric_host_address(&host);
        let same_port = target.port == self.0.port;
        if same_port && host == normalize_host(&self.0.host.to_string()) {
            return Err("cannot tunnel back into the TCP upstream proxy".into());
        }
        let authority = if let Some(ip) = target_ip {
            if state
                .resolved_target_blocked(&host, ip, authorization)
                .await?
                || (is_non_public_ip(ip)
                    && !(state.proxy_private_ips_via_upstream && is_private_network_ip(ip)))
            {
                return Err("IP destination cannot use the TCP upstream proxy".into());
            }
            HostWithPort::new(ip.into(), target.port).to_string()
        } else {
            if host == "localhost"
                || host.ends_with(".localhost")
                || !state.permits_remote_tcp_dns().await?
            {
                return Err("destination requires local address enforcement".into());
            }
            target.to_string()
        };
        let request = Request::builder()
            .method(Method::CONNECT)
            .version(Version::HTTP_11)
            .uri(&authority)
            .header(HOST, &authority)
            .extension(ProxyAddress {
                protocol: Some(Protocol::HTTP),
                address: self.0.clone(),
                credential: None,
            })
            .body(Body::empty())?;
        // A fresh request carries only the trusted next hop. Rama dials that endpoint and
        // retains bytes read past the CONNECT headers in the upgraded stream.
        tokio::time::timeout(Duration::from_secs(10), async {
            let connector = HttpConnector::new(TargetCheckedTcpConnector::new(state.clone()));
            let EstablishedClientConnection { input, conn } = connector.serve(request).await?;
            // Also reject the proxy's actual IP when provisioning supplied a hostname.
            if same_port && let Some(ip) = target_ip {
                let peer = conn
                    .extensions()
                    .get::<ClientSocketInfo>()
                    .ok_or("missing TCP upstream peer address")?
                    .0
                    .peer_addr();
                if ip.to_canonical() == peer.ip().to_canonical() {
                    return Err("cannot tunnel back into the TCP upstream proxy".into());
                }
            }
            let response = conn.serve(input).await?;
            if !response.status().is_success() {
                return Err(format!("TCP upstream refused CONNECT ({})", response.status()).into());
            }
            let stream = upgrade::handle_upgrade(response).await?;
            let parts = stream
                .downcast::<Pin<Box<TcpStream>>>()
                .map_err(|_| "unexpected transport for HTTP CONNECT")?;
            Ok(Rewind::new_buffered(
                *Pin::into_inner(parts.io),
                parts.read_buf,
            ))
        })
        .await?
    }
}

#[cfg(test)]
#[path = "tcp_upstream_tests.rs"]
mod tests;
