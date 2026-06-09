use crate::config::UpstreamProxyMode;
use crate::connect_policy::TargetCheckedTcpConnector;
use crate::state::NetworkProxyState;
use codex_client::RouteFailureClass;
use codex_client::SystemProxyRouteDecision;
use codex_client::resolve_system_proxy_for_url;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
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
use thiserror::Error;
use tracing::info;
use tracing::warn;

#[cfg(target_os = "macos")]
use rama_unix::client::UnixConnector;

#[derive(Debug, Error)]
pub(crate) enum UpstreamProxyError {
    #[error("system proxy resolution failed: {0}")]
    SystemProxyUnavailable(RouteFailureClass),

    #[error("system proxy returned invalid proxy URL: {url}")]
    InvalidSystemProxyUrl { url: String },

    #[error("system proxy returned unsupported proxy protocol: {url}")]
    UnsupportedSystemProxyProtocol { url: String },
}

#[derive(Clone, Default)]
struct EnvProxyConfig {
    http: Option<ProxyAddress>,
    https: Option<ProxyAddress>,
    all: Option<ProxyAddress>,
}

impl EnvProxyConfig {
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

#[derive(Clone)]
struct ProxyConfig {
    mode: UpstreamProxyMode,
    env: EnvProxyConfig,
}

impl ProxyConfig {
    fn direct() -> Self {
        Self {
            mode: UpstreamProxyMode::Direct,
            env: EnvProxyConfig::default(),
        }
    }

    fn from_mode(mode: UpstreamProxyMode) -> Self {
        Self {
            mode,
            env: EnvProxyConfig::from_env(),
        }
    }

    fn proxy_for_url(
        &self,
        request_url: &str,
        is_secure: bool,
    ) -> Result<Option<ProxyAddress>, UpstreamProxyError> {
        match self.mode {
            UpstreamProxyMode::Direct => Ok(None),
            UpstreamProxyMode::Env => Ok(self.env.proxy_for_protocol(is_secure)),
            UpstreamProxyMode::System => {
                system_proxy_for_url(request_url, /*include_auto_detect*/ true)
            }
            UpstreamProxyMode::Auto => {
                match system_proxy_for_url(request_url, /*include_auto_detect*/ true) {
                    Ok(proxy) => Ok(proxy),
                    Err(err) => {
                        warn!("system proxy unavailable; falling back to env proxy ({err})");
                        Ok(self.env.proxy_for_protocol(is_secure))
                    }
                }
            }
        }
    }
}

fn system_proxy_for_url(
    request_url: &str,
    include_auto_detect: bool,
) -> Result<Option<ProxyAddress>, UpstreamProxyError> {
    match resolve_system_proxy_for_url(request_url, include_auto_detect) {
        SystemProxyRouteDecision::Direct => Ok(None),
        SystemProxyRouteDecision::Proxy { url } => proxy_address_from_system_url(&url).map(Some),
        SystemProxyRouteDecision::Unavailable { failure } => {
            Err(UpstreamProxyError::SystemProxyUnavailable(failure))
        }
    }
}

fn proxy_address_from_system_url(proxy_url: &str) -> Result<ProxyAddress, UpstreamProxyError> {
    let proxy = ProxyAddress::try_from(proxy_url).map_err(|_| {
        UpstreamProxyError::InvalidSystemProxyUrl {
            url: proxy_url.to_string(),
        }
    })?;
    if proxy
        .protocol
        .as_ref()
        .map(rama_net::Protocol::is_http)
        .unwrap_or(true)
    {
        return Ok(proxy);
    }
    Err(UpstreamProxyError::UnsupportedSystemProxyProtocol {
        url: proxy_url.to_string(),
    })
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

pub(crate) fn proxy_for_connect(
    request_url: &str,
    mode: UpstreamProxyMode,
) -> Result<Option<ProxyAddress>, UpstreamProxyError> {
    ProxyConfig::from_mode(mode).proxy_for_url(request_url, /*is_secure*/ true)
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
        Self::new(ProxyConfig::direct(), TargetCheckedTcpConnector::new(state))
    }

    pub(crate) fn from_proxy_mode(state: Arc<NetworkProxyState>, mode: UpstreamProxyMode) -> Self {
        Self::new(
            ProxyConfig::from_mode(mode),
            TargetCheckedTcpConnector::new(state),
        )
    }

    pub(crate) fn direct_with_allow_local_binding(allow_local_binding: bool) -> Self {
        Self::new(
            ProxyConfig::direct(),
            TargetCheckedTcpConnector::from_allow_local_binding(allow_local_binding),
        )
    }

    pub(crate) fn from_proxy_mode_with_allow_local_binding(
        mode: UpstreamProxyMode,
        allow_local_binding: bool,
    ) -> Self {
        Self::new(
            ProxyConfig::from_mode(mode),
            TargetCheckedTcpConnector::from_allow_local_binding(allow_local_binding),
        )
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn unix_socket(path: &str) -> Self {
        let connector = build_unix_connector(path);
        Self {
            connector,
            proxy_config: ProxyConfig::direct(),
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
        let proxy = match request_context
            .as_ref()
            .map(|ctx| request_target_url(&req, ctx))
            .map(|request_url| {
                self.proxy_config.proxy_for_url(
                    &request_url,
                    request_context
                        .as_ref()
                        .map(|ctx| ctx.protocol.is_secure())
                        .unwrap_or(false),
                )
            })
            .transpose()
        {
            Ok(proxy) => proxy.flatten(),
            Err(err) => {
                warn!("HTTP upstream proxy resolution failed (target={authority}): {err}");
                return Err(OpaqueError::from_display(format!(
                    "upstream proxy resolution failed: {err}"
                )));
            }
        };
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

fn request_target_url(req: &Request<Body>, request_context: &RequestContext) -> String {
    if req.uri().scheme_str().is_some() && req.uri().authority().is_some() {
        return req.uri().to_string();
    }

    let scheme = if request_context.protocol.is_secure() {
        "https"
    } else {
        "http"
    };
    let authority = request_context.host_with_port();
    let path = req
        .uri()
        .path_and_query()
        .map(rama_http::uri::PathAndQuery::as_str)
        .unwrap_or("/");
    format!("{scheme}://{authority}{path}")
}

fn build_http_connector(
    transport: TargetCheckedTcpConnector,
) -> BoxService<
    Request<Body>,
    EstablishedClientConnection<HttpClientService<Body>, Request<Body>>,
    BoxError,
> {
    ensure_rustls_crypto_provider();
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
