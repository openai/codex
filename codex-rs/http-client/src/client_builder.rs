//! HTTP client construction that makes outbound proxy policy explicit.
//!
//! Product traffic should normally enter through [`HttpClientFactory`] for a fixed destination or
//! [`crate::RouteAwareClientPool`] when request and redirect URLs can vary. The direct and
//! transport-default terminal methods exist only for narrow exceptional or legacy compatibility
//! paths.

use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::BuildCustomCaTransportError;
use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientFactory;
use crate::HttpClientTlsConfig;
use crate::OutboundProxyRoute;
use crate::chatgpt_cloudflare_cookies::ChatGptCookieStore;
use crate::client::HttpClientBackend;
use crate::client::RequestLogging;
use crate::client::TransportClient;
use crate::custom_ca::build_reqwest_client_with_custom_ca;
use crate::with_chatgpt_cloudflare_cookie_store;

/// Configures an [`HttpClient`] without exposing the underlying HTTP implementation.
///
/// Product traffic should prefer [`HttpClientFactory::build_client`] or finish this builder with
/// [`Self::build_respecting_outbound_proxy_policy`]. The other terminal methods deliberately
/// bypass the factory and are restricted to documented exceptional or legacy compatibility paths.
#[derive(Clone)]
pub struct HttpClientBuilder {
    http2_prior_knowledge: bool,
    pub(crate) default_headers: Option<HeaderMap>,
    follow_redirects: bool,
    pub(crate) redirect_observed: Option<Arc<AtomicBool>>,
    connect_timeout: Option<Duration>,
    chatgpt_cloudflare_cookie_store: bool,
    chatgpt_cookie_store: Option<Arc<ChatGptCookieStore>>,
    pub(crate) request_logging: RequestLogging,
    tls_backend: TlsBackend,
    tls: HttpClientTlsConfig,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TlsBackend {
    #[default]
    TransportDefault,
    Rustls,
}

impl HttpClientFactory {
    /// Builds an HTTP client for one fixed destination using the configured proxy policy.
    ///
    /// This is the preferred construction path for product traffic that uses a fixed destination.
    /// Use [`crate::RouteAwareClientPool`] instead when request or redirect URLs can vary.
    pub fn build_client(
        &self,
        request_url: &str,
        route_class: ClientRouteClass,
    ) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
        HttpClientBuilder::new().build_respecting_outbound_proxy_policy(
            self,
            request_url,
            route_class,
        )
    }

    /// Builds a policy-aware client without request URL or response-header diagnostics.
    ///
    /// This has the same routing guidance as [`Self::build_client`].
    pub fn build_client_without_request_logging(
        &self,
        request_url: &str,
        route_class: ClientRouteClass,
    ) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
        HttpClientBuilder::new()
            .without_request_logging()
            .build_respecting_outbound_proxy_policy(self, request_url, route_class)
    }
}

impl HttpClientBuilder {
    /// Builds a strict pooled client with explicit TLS settings and the factory's proxy policy.
    /// Route and transport construction failures are returned when sending a request.
    pub fn build_with_tls(
        mut self,
        http_client_factory: &HttpClientFactory,
        route_class: ClientRouteClass,
        tls: HttpClientTlsConfig,
    ) -> HttpClient {
        self.tls = tls;
        crate::RouteAwareClientPool::with_builder(http_client_factory.clone(), route_class, self)
            .with_tls_backend_fallback()
            .into_client()
    }

    /// Uses HTTP/2 for SDKs such as gRPC that require framed bidirectional bodies.
    pub fn http2_prior_knowledge(mut self) -> Self {
        self.http2_prior_knowledge = true;
        self
    }
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies every configured value unless the request sets that header explicitly.
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = Some(headers);
        self
    }

    pub fn without_redirects(mut self) -> Self {
        self.follow_redirects = false;
        self
    }

    /// Marks the supplied flag when a redirect is encountered, preserving the default policy.
    /// Use a fresh flag for each operation whose retry safety depends on its redirect history.
    pub fn with_redirect_tracking(mut self, redirect_observed: Arc<AtomicBool>) -> Self {
        self.redirect_observed = Some(redirect_observed);
        self
    }

    pub(crate) fn follows_redirects(&self) -> bool {
        self.follow_redirects
    }

    pub(crate) fn request_logging_enabled(&self) -> bool {
        self.request_logging == RequestLogging::Enabled
    }

    pub(crate) fn with_rustls_tls(mut self) -> Self {
        self.tls_backend = TlsBackend::Rustls;
        self
    }

    /// Limits only connection establishment, not the request as a whole.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn with_chatgpt_cloudflare_cookie_store(mut self) -> Self {
        self.chatgpt_cloudflare_cookie_store = true;
        self
    }

    /// Uses the factory's configured ChatGPT cookies without changing proxy behavior.
    pub fn with_chatgpt_cookies(mut self, http_client_factory: &HttpClientFactory) -> Self {
        self.chatgpt_cloudflare_cookie_store = true;
        self.chatgpt_cookie_store = http_client_factory.chatgpt_cookie_store();
        self
    }

    /// Suppresses request URL and response-header diagnostics.
    pub fn without_request_logging(mut self) -> Self {
        self.request_logging = RequestLogging::Disabled;
        self
    }

    /// Builds a client that honors the [`HttpClientFactory`] outbound proxy policy.
    ///
    /// This is the preferred terminal method for product traffic. The request URL is used to
    /// resolve a concrete direct or proxy route when the factory is configured with
    /// [`crate::OutboundProxyPolicy::RespectSystemProxy`].
    pub fn build_respecting_outbound_proxy_policy(
        mut self,
        http_client_factory: &HttpClientFactory,
        request_url: &str,
        route_class: ClientRouteClass,
    ) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
        if http_client_factory.network_policy().is_managed() {
            return Ok(crate::RouteAwareClientPool::with_builder(
                http_client_factory.clone(),
                route_class,
                self,
            )
            .into_client());
        }
        self.chatgpt_cookie_store = http_client_factory.chatgpt_cookie_store();
        let (builder, request_logging, default_headers) = self.into_reqwest_parts();
        let inner = http_client_factory.build_reqwest_client(builder, request_url, route_class)?;
        Ok(HttpClient::from_parts(
            inner,
            request_logging,
            default_headers,
        ))
    }

    /// Builds a client for a route that was already resolved by a route-aware caller.
    pub(crate) fn build_for_resolved_route(
        mut self,
        http_client_factory: &HttpClientFactory,
        route_class: ClientRouteClass,
        route: &OutboundProxyRoute,
    ) -> Result<TransportClient, BuildRouteAwareHttpClientError> {
        self.chatgpt_cookie_store = http_client_factory.chatgpt_cookie_store();
        let explicit_roots = self.tls.root_certificate.is_some();
        let (builder, request_logging, default_headers) = self.into_reqwest_parts();
        let builder = crate::outbound_proxy::configure_builder_for_resolved_route(
            builder,
            route_class,
            route,
        )?;
        let inner = if explicit_roots {
            builder
                .build()
                .map_err(BuildRouteAwareHttpClientError::ExplicitTls)?
        } else {
            build_reqwest_client_with_custom_ca(builder)?
        };
        Ok(TransportClient::new(
            inner,
            request_logging,
            default_headers,
        ))
    }

    /// Builds a client using the transport's default proxy behavior.
    ///
    /// # Legacy compatibility only
    ///
    /// This bypasses [`HttpClientFactory`] and therefore does not honor its configured outbound
    /// proxy policy. New product traffic must use [`Self::build_respecting_outbound_proxy_policy`]
    /// or [`HttpClientFactory::build_client`].
    #[deprecated(
        note = "legacy compatibility only; use HttpClientFactory::build_client or build_respecting_outbound_proxy_policy"
    )]
    pub fn build_with_transport_default_proxy(
        self,
    ) -> Result<HttpClient, BuildCustomCaTransportError> {
        self.build_with_proxy_routing(ProxyRouting::TransportDefault)
    }

    /// Builds a client that connects directly without using a proxy.
    ///
    /// # Exceptional use only
    ///
    /// This bypasses [`HttpClientFactory`] and is appropriate only when bypassing proxy discovery
    /// is itself required: for example, a hermetic local test fixture, a localhost callback, or
    /// sandbox traffic whose egress routing is handled separately. Ordinary outbound product
    /// traffic must use [`Self::build_respecting_outbound_proxy_policy`] or
    /// [`HttpClientFactory::build_client`].
    pub fn build_direct(self) -> Result<HttpClient, BuildCustomCaTransportError> {
        self.build_with_proxy_routing(ProxyRouting::Direct)
    }

    /// Builds a transport-default client while preserving the legacy custom-CA fallback.
    ///
    /// # Legacy compatibility only
    ///
    /// This preserves call sites that historically logged a custom-CA error and continued with
    /// system roots. New product traffic must propagate construction errors through
    /// [`Self::build_respecting_outbound_proxy_policy`] or [`HttpClientFactory::build_client`].
    #[deprecated(
        note = "legacy custom-CA fallback only; use HttpClientFactory::build_client or build_respecting_outbound_proxy_policy"
    )]
    pub fn build_with_transport_default_proxy_and_custom_ca_fallback(self) -> HttpClient {
        HttpClient {
            backend: HttpClientBackend::Direct(
                self.build_with_custom_ca_fallback(ProxyRouting::TransportDefault),
            ),
        }
    }

    /// Builds a direct client while preserving the legacy custom-CA fallback.
    ///
    /// # Legacy compatibility only
    ///
    /// This combines the exceptional proxy bypass described by [`Self::build_direct`] with the
    /// historical behavior of logging a custom-CA error and continuing with system roots.
    #[deprecated(
        note = "legacy custom-CA fallback only; use build_direct and propagate construction errors"
    )]
    pub fn build_direct_with_custom_ca_fallback(self) -> HttpClient {
        HttpClient {
            backend: HttpClientBackend::Direct(
                self.build_with_custom_ca_fallback(ProxyRouting::Direct),
            ),
        }
    }

    fn build_with_proxy_routing(
        mut self,
        proxy_routing: ProxyRouting,
    ) -> Result<HttpClient, BuildCustomCaTransportError> {
        let request_logging = self.request_logging;
        let default_headers = self.default_headers.take().unwrap_or_default();
        build_reqwest_client_with_custom_ca(self.reqwest_builder(proxy_routing))
            .map(|inner| HttpClient::from_parts(inner, request_logging, default_headers))
    }

    pub(crate) fn build_with_custom_ca_fallback(
        self,
        proxy_routing: ProxyRouting,
    ) -> TransportClient {
        self.build_with_custom_ca_fallback_using(proxy_routing, build_reqwest_client_with_custom_ca)
    }

    fn build_with_custom_ca_fallback_using(
        mut self,
        proxy_routing: ProxyRouting,
        build_with_custom_ca: impl FnOnce(
            reqwest::ClientBuilder,
        )
            -> Result<reqwest::Client, BuildCustomCaTransportError>,
    ) -> TransportClient {
        let request_logging = self.request_logging;
        let default_headers = self.default_headers.take().unwrap_or_default();
        let inner = match build_with_custom_ca(self.clone().reqwest_builder(proxy_routing)) {
            Ok(inner) => inner,
            Err(error) => {
                tracing::event!(
                    target: "codex_otel.log_only",
                    tracing::Level::WARN,
                    event.name = "codex.http_client.custom_ca_fallback",
                    "HTTP client fell back to system root certificates"
                );
                tracing::warn!(error = %error, "failed to build HTTP client with custom CA");
                self.reqwest_builder(proxy_routing)
                    .build()
                    .unwrap_or_else(|fallback_error| {
                        tracing::warn!(
                            error = %fallback_error,
                            "failed to build fallback HTTP client"
                        );
                        reqwest::Client::new()
                    })
            }
        };
        TransportClient::new(inner, request_logging, default_headers)
    }

    fn into_reqwest_parts(mut self) -> (reqwest::ClientBuilder, RequestLogging, HeaderMap) {
        let request_logging = self.request_logging;
        let default_headers = self.default_headers.take().unwrap_or_default();
        (
            self.base_reqwest_builder(),
            request_logging,
            default_headers,
        )
    }

    fn reqwest_builder(self, proxy_routing: ProxyRouting) -> reqwest::ClientBuilder {
        let builder = self.base_reqwest_builder();
        match proxy_routing {
            ProxyRouting::TransportDefault => builder,
            ProxyRouting::Direct => builder.no_proxy(),
        }
    }

    fn base_reqwest_builder(self) -> reqwest::ClientBuilder {
        let mut builder = reqwest::Client::builder();
        if self.http2_prior_knowledge {
            builder = builder.http2_prior_knowledge();
        }
        if self.tls_backend == TlsBackend::Rustls || self.tls.client_identity.is_some() {
            ensure_rustls_crypto_provider();
            builder = builder.use_rustls_tls();
        }
        if let Some(certificate) = self.tls.root_certificate {
            builder = builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(certificate);
        }
        if let Some(identity) = self.tls.client_identity {
            builder = builder.identity(identity).https_only(true);
        }
        if !self.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        } else if let Some(redirect_observed) = self.redirect_observed {
            builder = builder.redirect(reqwest::redirect::Policy::custom(move |attempt| {
                redirect_observed.store(/*val*/ true, Ordering::Relaxed);
                reqwest::redirect::Policy::default().redirect(attempt)
            }));
        }
        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }
        if self.chatgpt_cloudflare_cookie_store {
            builder = match self.chatgpt_cookie_store {
                Some(store) => builder.cookie_provider(store),
                None => with_chatgpt_cloudflare_cookie_store(builder),
            };
        }
        builder
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            http2_prior_knowledge: false,
            default_headers: None,
            follow_redirects: true,
            redirect_observed: None,
            connect_timeout: None,
            chatgpt_cloudflare_cookie_store: false,
            chatgpt_cookie_store: None,
            request_logging: RequestLogging::Enabled,
            tls_backend: TlsBackend::TransportDefault,
            tls: HttpClientTlsConfig::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ProxyRouting {
    TransportDefault,
    Direct,
}

#[cfg(test)]
#[path = "client_builder_tests.rs"]
mod tests;
