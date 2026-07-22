mod chatgpt_cloudflare_cookies;
mod chatgpt_hosts;
mod client;
mod client_builder;
mod custom_ca;
mod error;
mod outbound_proxy;
mod request;
mod route_aware_client_pool;
mod route_aware_redirect;
mod transport;

pub use crate::chatgpt_cloudflare_cookies::with_chatgpt_cloudflare_cookie_store;
pub use crate::chatgpt_hosts::is_allowed_chatgpt_host;
pub use crate::client::HttpClient;
pub use crate::client::HttpError;
pub use crate::client::HttpResponse;
pub use crate::client::RequestBuilder;
pub use crate::client_builder::HttpClientBuilder;
pub use crate::custom_ca::BuildCustomCaTransportError;
/// Test-only subprocess hook for custom CA coverage.
///
/// This stays public only so the `custom_ca_probe` binary target can reuse the shared helper. It
/// is hidden from normal docs because ordinary callers should use
/// [`build_reqwest_client_with_custom_ca`] instead.
#[doc(hidden)]
pub use crate::custom_ca::build_reqwest_client_for_subprocess_tests;
pub use crate::custom_ca::build_reqwest_client_with_custom_ca;
pub use crate::custom_ca::build_rustls_client_config_with_custom_ca;
pub use crate::custom_ca::maybe_build_rustls_client_config_with_custom_ca;
pub use crate::error::StreamError;
pub use crate::error::TransportError;
pub use crate::outbound_proxy::BuildRouteAwareHttpClientError;
pub use crate::outbound_proxy::ClientRouteClass;
pub use crate::outbound_proxy::HttpClientFactory;
pub use crate::outbound_proxy::OutboundProxyPolicy;
pub use crate::outbound_proxy::OutboundProxyRoute;
pub use crate::outbound_proxy::RouteFailureClass;
#[doc(hidden)]
pub use crate::outbound_proxy::cache_system_proxy_route_for_test;
pub use crate::request::EncodedJsonBody;
pub use crate::request::PreparedRequestBody;
pub use crate::request::Request;
pub use crate::request::RequestBody;
pub use crate::request::RequestCompression;
pub use crate::request::Response;
pub use crate::route_aware_client_pool::RouteAwareClientPool;
pub use crate::route_aware_client_pool::RouteAwareClientPoolError;
pub use crate::route_aware_client_pool::RouteAwareRequestBuilder;
pub use crate::route_aware_client_pool::RouteAwareRequestError;
pub use crate::transport::ByteStream;
pub use crate::transport::HttpTransport;
pub use crate::transport::ReqwestTransport;
pub use crate::transport::StreamResponse;
