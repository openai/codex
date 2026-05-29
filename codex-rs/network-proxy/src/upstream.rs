use crate::connect_policy::TargetCheckedTcpConnector;
use crate::state::NetworkProxyState;
use rama_core::Layer;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::error::ErrorExt as _;
use rama_core::error::OpaqueError;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use rama_core::service::BoxService;
use rama_http::Body;
use rama_http::Request;
use rama_http::Response;
use rama_http::layer::version_adapter::RequestVersionAdapter;
use rama_http_backend::client::HttpClientService;
use rama_http_backend::client::HttpConnector;
use rama_http_backend::client::proxy::layer::HttpProxyConnectorLayer;
use rama_net::address::ProxyAddress;
use rama_net::client::EstablishedClientConnection;
use rama_net::http::RequestContext;
use rama_tls_rustls::client::TlsConnectorDataBuilder;
use rama_tls_rustls::client::TlsConnectorLayer;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use tracing::warn;

#[cfg(target_os = "macos")]
use rama_unix::client::UnixConnector;

#[derive(Clone, Default)]
struct ProxyConfig {
    http: Option<ProxyAddress>,
    https: Option<ProxyAddress>,
    all: Option<ProxyAddress>,
}

impl ProxyConfig {
    fn from_env() -> Self {
        let http = read_proxy_env(&["HTTP_PROXY", "http_proxy"]);
        let https = read_proxy_env(&["HTTPS_PROXY", "https_proxy"]);
        let all = read_proxy_env(&["ALL_PROXY", "all_proxy"]);
        Self { http, https, all }
    }

    fn proxy_for_protocol(&self, is_secure: bool) -> Option<ProxyAddress> {
        if is_secure {
            self.https
                .clone()
                .or_else(|| self.http.clone())
                .or_else(|| self.all.clone())
        } else {
            self.http.clone().or_else(|| self.all.clone())
        }
    }
}

fn read_proxy_env(keys: &[&str]) -> Option<ProxyAddress> {
    for key in keys {
        let Ok(value) = std::env::var(key) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match ProxyAddress::try_from(value) {
            Ok(proxy) => {
                if proxy
                    .protocol
                    .as_ref()
                    .map(rama_net::Protocol::is_http)
                    .unwrap_or(true)
                {
                    return Some(proxy);
                }
                warn!("ignoring {key}: non-http proxy protocol");
            }
            Err(err) => {
                warn!("ignoring {key}: invalid proxy address ({err})");
            }
        }
    }
    None
}

pub(crate) fn proxy_for_connect() -> Option<ProxyAddress> {
    ProxyConfig::from_env().proxy_for_protocol(/*is_secure*/ true)
}

#[derive(Clone)]
pub(crate) struct UpstreamClient {
    connector: BoxService<
        Request<Body>,
        EstablishedClientConnection<HttpClientService<Body>, Request<Body>>,
        BoxError,
    >,
    proxy_config: ProxyConfig,
}

impl UpstreamClient {
    pub(crate) fn direct(state: Arc<NetworkProxyState>) -> Self {
        Self::new(
            ProxyConfig::default(),
            TargetCheckedTcpConnector::new(state),
        )
    }

    pub(crate) fn from_env_proxy(state: Arc<NetworkProxyState>) -> Self {
        Self::new(
            ProxyConfig::from_env(),
            TargetCheckedTcpConnector::new(state),
        )
    }

    pub(crate) fn direct_with_allow_local_binding(allow_local_binding: bool) -> Self {
        Self::new(
            ProxyConfig::default(),
            TargetCheckedTcpConnector::from_allow_local_binding(allow_local_binding),
        )
    }

    pub(crate) fn from_env_proxy_with_allow_local_binding(allow_local_binding: bool) -> Self {
        Self::new(
            ProxyConfig::from_env(),
            TargetCheckedTcpConnector::from_allow_local_binding(allow_local_binding),
        )
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn unix_socket(path: &str) -> Self {
        let connector = build_unix_connector(path);
        Self {
            connector,
            proxy_config: ProxyConfig::default(),
        }
    }

    fn new(proxy_config: ProxyConfig, transport: TargetCheckedTcpConnector) -> Self {
        let connector = build_http_connector(transport);
        Self {
            connector,
            proxy_config,
        }
    }
}

impl Service<Request<Body>> for UpstreamClient {
    type Output = Response;
    type Error = OpaqueError;

    async fn serve(&self, mut req: Request<Body>) -> Result<Self::Output, Self::Error> {
        let request_context = RequestContext::try_from(&req).ok();
        let authority = request_context
            .as_ref()
            .map(|ctx| ctx.host_with_port().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let proxy = self.proxy_config.proxy_for_protocol(
            request_context
                .as_ref()
                .map(|ctx| ctx.protocol.is_secure())
                .unwrap_or(false),
        );
        match proxy.as_ref() {
            Some(proxy) => info!(
                "HTTP upstream route selected (target={authority}, route=upstream_proxy, proxy={})",
                proxy.address
            ),
            None => info!("HTTP upstream route selected (target={authority}, route=direct)"),
        }
        if let Some(proxy) = proxy {
            req.extensions_mut().insert(proxy);
        }

        let uri = req.uri().clone();
        let connect_started_at = Instant::now();
        let EstablishedClientConnection {
            input: mut req,
            conn: http_connection,
        } = match self.connector.serve(req).await {
            Ok(connection) => {
                info!(
                    "HTTP upstream connection established (target={authority}, elapsed_ms={})",
                    connect_started_at.elapsed().as_millis()
                );
                connection
            }
            Err(err) => {
                warn!(
                    "HTTP upstream connection failed (target={authority}, elapsed_ms={})",
                    connect_started_at.elapsed().as_millis()
                );
                return Err(OpaqueError::from_boxed(err));
            }
        };

        req.extensions_mut()
            .extend(http_connection.extensions().clone());

        let request_started_at = Instant::now();
        match http_connection.serve(req).await {
            Ok(resp) => {
                info!(
                    "HTTP upstream response headers received (target={authority}, elapsed_ms={})",
                    request_started_at.elapsed().as_millis()
                );
                Ok(resp)
            }
            Err(err) => {
                warn!(
                    "HTTP upstream response headers failed (target={authority}, elapsed_ms={})",
                    request_started_at.elapsed().as_millis()
                );
                Err(OpaqueError::from_boxed(err)
                    .context(format!("http request failure for uri: {uri}")))
            }
        }
    }
}

fn build_http_connector(
    transport: TargetCheckedTcpConnector,
) -> BoxService<
    Request<Body>,
    EstablishedClientConnection<HttpClientService<Body>, Request<Body>>,
    BoxError,
> {
    let proxy = HttpProxyConnectorLayer::optional().into_layer(transport);
    let tls_config = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .build();
    let tls = TlsConnectorLayer::auto()
        .with_connector_data(tls_config)
        .into_layer(proxy);
    let tls = RequestVersionAdapter::new(tls);
    let connector = HttpConnector::new(tls);
    connector.boxed()
}

#[cfg(target_os = "macos")]
fn build_unix_connector(
    path: &str,
) -> BoxService<
    Request<Body>,
    EstablishedClientConnection<HttpClientService<Body>, Request<Body>>,
    BoxError,
> {
    let transport = UnixConnector::fixed(path);
    let connector = HttpConnector::new(transport);
    connector.boxed()
}
