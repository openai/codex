use url::Host;
use url::Url;

mod chatgpt_cloudflare_cookies;
mod chatgpt_hosts;
mod chatgpt_routing_affinity;
mod client;
mod client_builder;
mod client_tls;
mod custom_ca;
mod error;
mod native_roots;
mod network_policy;
mod openai_endpoints;
mod openai_headers;
mod outbound_proxy;
mod request;
mod request_builder;
mod request_draft;
mod response;
mod retry_after;
mod route_aware_client_pool;
mod route_aware_redirect;
mod tls_backend_fallback;
mod transport;

pub use crate::chatgpt_cloudflare_cookies::with_chatgpt_cloudflare_cookie_store;
pub use crate::chatgpt_hosts::is_allowed_chatgpt_host;
pub use crate::chatgpt_routing_affinity::ChatGptRoutingAffinity;
pub use crate::client::HttpClient;
pub use crate::client::HttpError;
pub use crate::client_builder::HttpClientBuilder;
pub use crate::client_tls::HttpClientTlsConfig;
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
pub use crate::network_policy::DestinationPolicy;
pub use crate::network_policy::NetworkPermit;
pub use crate::network_policy::NetworkPolicy;
pub use crate::network_policy::NetworkPolicyController;
pub use crate::network_policy::NetworkPolicyDenied;
pub use crate::network_policy::NetworkPolicyRevision;
pub use crate::openai_endpoints::OpenAiDestination;
pub use crate::openai_endpoints::classify_openai_api_url;
pub use crate::openai_endpoints::is_openai_api_url;
pub use crate::openai_headers::CHATGPT_ACCOUNT_ID_HEADER;
pub use crate::openai_headers::OPENAI_ORGANIZATION_HEADER;
pub use crate::openai_headers::OPENAI_PROJECT_HEADER;
pub use crate::outbound_proxy::BuildRouteAwareHttpClientError;
pub use crate::outbound_proxy::ClientRouteClass;
pub use crate::outbound_proxy::HttpClientFactory;
#[cfg(target_os = "macos")]
pub use crate::outbound_proxy::MacosSystemProxyConfiguration;
pub use crate::outbound_proxy::OutboundProxyPolicy;
pub use crate::outbound_proxy::OutboundProxyRoute;
pub use crate::outbound_proxy::RouteFailureClass;
#[doc(hidden)]
pub use crate::outbound_proxy::cache_system_proxy_route_for_test;
#[cfg(target_os = "macos")]
pub use crate::outbound_proxy::macos_system_proxy_configuration;
pub use crate::request::EncodedJsonBody;
pub use crate::request::PreparedRequestBody;
pub use crate::request::Request;
pub use crate::request::RequestBody;
pub use crate::request::RequestCompression;
pub use crate::request::Response;
pub use crate::request_builder::RequestBuilder;
pub use crate::response::HttpResponse;
pub use crate::retry_after::RetryAfter;
pub use crate::route_aware_client_pool::RouteAwareClientPool;
pub use crate::route_aware_client_pool::RouteAwareClientPoolError;
pub use crate::route_aware_client_pool::RouteAwareRequestError;
pub use crate::transport::ByteStream;
pub use crate::transport::HttpTransport;
pub use crate::transport::ReqwestTransport;
pub use crate::transport::StreamResponse;

#[cfg(windows)]
mod windows_proxy_auth;
#[cfg(windows)]
mod windows_tls;

#[cfg(windows)]
pub use crate::windows_proxy_auth::WindowsProxyTls;
#[cfg(windows)]
pub use crate::windows_proxy_auth::connect_windows_proxy;
#[cfg(windows)]
pub use crate::windows_tls::build_windows_platform_tls_config;

/// Validates HTTPS or loopback HTTP endpoints, rejecting userinfo and fragments.
/// Errors identify the caller-supplied field without including the URL.
pub fn validate_endpoint_url(value: &str, field: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| format!("invalid {field} URL"))?;
    let loopback = match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(host)) => host.is_loopback(),
        Some(Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    };
    if url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
    {
        return Err(format!(
            "{field} must use HTTPS (or loopback HTTP), without userinfo or a fragment"
        ));
    }
    Ok(url)
}
