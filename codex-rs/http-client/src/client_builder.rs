//! HTTP client construction that makes outbound proxy policy explicit.
//!
//! Product traffic should normally enter through [`HttpClientFactory`] for a fixed destination or
//! [`crate::RouteAwareClientPool`] when request and redirect URLs can vary. The direct and
//! transport-default terminal methods exist only for narrow exceptional or legacy compatibility
//! paths.

use http::HeaderMap;
use std::time::Duration;

use crate::BuildCustomCaTransportError;
use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientFactory;
use crate::OutboundProxyRoute;
use crate::client::RequestLogging;
use crate::custom_ca::build_reqwest_client_with_custom_ca;
use crate::with_chatgpt_cloudflare_cookie_store;

/// Configures an [`HttpClient`] without exposing the underlying HTTP implementation.
///
/// Product traffic should prefer [`HttpClientFactory::build_client`] or finish this builder with
/// [`Self::build_respecting_outbound_proxy_policy`]. The other terminal methods deliberately
/// bypass the factory and are restricted to documented exceptional or legacy compatibility paths.
#[derive(Clone)]
pub struct HttpClientBuilder {
    default_headers: Option<HeaderMap>,
    follow_redirects: bool,
    connect_timeout: Option<Duration>,
    chatgpt_cloudflare_cookie_store: bool,
    request_logging: RequestLogging,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = Some(headers);
        self
    }

    pub fn without_redirects(mut self) -> Self {
        self.follow_redirects = false;
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
        self,
        http_client_factory: &HttpClientFactory,
        request_url: &str,
        route_class: ClientRouteClass,
    ) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
        let (builder, request_logging) = self.into_reqwest_parts();
        let inner = http_client_factory.build_reqwest_client(builder, request_url, route_class)?;
        Ok(HttpClient::from_parts(inner, request_logging))
    }

    /// Builds a client for a route that was already resolved by a route-aware caller.
    pub(crate) fn build_for_resolved_route(
        self,
        http_client_factory: &HttpClientFactory,
        route_class: ClientRouteClass,
        route: &OutboundProxyRoute,
    ) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
        let (builder, request_logging) = self.into_reqwest_parts();
        let inner = http_client_factory.build_reqwest_client_for_resolved_route(
            builder,
            route_class,
            route,
        )?;
        Ok(HttpClient::from_parts(inner, request_logging))
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
        self.build_with_custom_ca_fallback(ProxyRouting::TransportDefault)
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
        self.build_with_custom_ca_fallback(ProxyRouting::Direct)
    }

    fn build_with_proxy_routing(
        self,
        proxy_routing: ProxyRouting,
    ) -> Result<HttpClient, BuildCustomCaTransportError> {
        let request_logging = self.request_logging;
        build_reqwest_client_with_custom_ca(self.reqwest_builder(proxy_routing))
            .map(|inner| HttpClient::from_parts(inner, request_logging))
    }

    fn build_with_custom_ca_fallback(self, proxy_routing: ProxyRouting) -> HttpClient {
        self.build_with_custom_ca_fallback_using(proxy_routing, build_reqwest_client_with_custom_ca)
    }

    fn build_with_custom_ca_fallback_using(
        self,
        proxy_routing: ProxyRouting,
        build_with_custom_ca: impl FnOnce(
            reqwest::ClientBuilder,
        )
            -> Result<reqwest::Client, BuildCustomCaTransportError>,
    ) -> HttpClient {
        let request_logging = self.request_logging;
        match build_with_custom_ca(self.clone().reqwest_builder(proxy_routing)) {
            Ok(inner) => HttpClient::from_parts(inner, request_logging),
            Err(error) => {
                tracing::warn!(error = %error, "failed to build HTTP client with custom CA");
                self.reqwest_builder(proxy_routing)
                    .build()
                    .map(|inner| HttpClient::from_parts(inner, request_logging))
                    .unwrap_or_else(|fallback_error| {
                        tracing::warn!(
                            error = %fallback_error,
                            "failed to build fallback HTTP client"
                        );
                        HttpClient::from_parts(reqwest::Client::new(), request_logging)
                    })
            }
        }
    }

    fn into_reqwest_parts(self) -> (reqwest::ClientBuilder, RequestLogging) {
        let request_logging = self.request_logging;
        (self.base_reqwest_builder(), request_logging)
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
        if let Some(default_headers) = self.default_headers {
            builder = builder.default_headers(default_headers);
        }
        if !self.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }
        if self.chatgpt_cloudflare_cookie_store {
            builder = with_chatgpt_cloudflare_cookie_store(builder);
        }
        builder
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            default_headers: None,
            follow_redirects: true,
            connect_timeout: None,
            chatgpt_cloudflare_cookie_store: false,
            request_logging: RequestLogging::Enabled,
        }
    }
}

#[derive(Clone, Copy)]
enum ProxyRouting {
    TransportDefault,
    Direct,
}

#[cfg(test)]
#[path = "client_builder_tests.rs"]
mod tests;
