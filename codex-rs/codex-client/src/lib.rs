mod chatgpt_cloudflare_cookies;
mod chatgpt_hosts;
mod custom_ca;
mod default_client;
mod error;
mod outbound_proxy;
mod request;
mod retry;
mod route_diagnostics;
mod sse;
mod telemetry;
mod transport;

pub use crate::chatgpt_cloudflare_cookies::with_chatgpt_cloudflare_cookie_store;
pub use crate::chatgpt_hosts::is_allowed_chatgpt_host;
pub use crate::custom_ca::BuildCustomCaTransportError;
/// Test-only subprocess hook for custom CA coverage.
///
/// This stays public only so the `custom_ca_probe` binary target can reuse the shared helper. It
/// is hidden from normal docs because ordinary callers should use
/// [`build_reqwest_client_with_custom_ca`] instead.
#[doc(hidden)]
pub use crate::custom_ca::build_reqwest_client_for_subprocess_tests;
pub use crate::custom_ca::build_reqwest_client_with_custom_ca;
pub use crate::custom_ca::maybe_build_rustls_client_config_with_custom_ca;
pub use crate::default_client::CodexHttpClient;
pub use crate::default_client::CodexRequestBuilder;
pub use crate::error::StreamError;
pub use crate::error::TransportError;
pub use crate::outbound_proxy::BuildProxiedHttpClientError;
pub use crate::outbound_proxy::OutboundProxyConfig;
pub use crate::outbound_proxy::OutboundProxyMode;
pub use crate::outbound_proxy::build_reqwest_client_for_route;
pub use crate::request::PreparedRequestBody;
pub use crate::request::Request;
pub use crate::request::RequestBody;
pub use crate::request::RequestCompression;
pub use crate::request::Response;
pub use crate::retry::RetryOn;
pub use crate::retry::RetryPolicy;
pub use crate::retry::backoff;
pub use crate::retry::run_with_retry;
pub use crate::route_diagnostics::CODEX_NETWORK_DIAGNOSTICS_ENV;
pub use crate::route_diagnostics::CODEX_SYSTEM_PROXY_ENV;
pub use crate::route_diagnostics::RedactedProxyEndpoint;
pub use crate::route_diagnostics::RouteDecision;
pub use crate::route_diagnostics::RouteDiagnostic;
pub use crate::route_diagnostics::RouteFailureClass;
pub use crate::route_diagnostics::RouteSource;
pub use crate::route_diagnostics::RouteTarget;
pub use crate::route_diagnostics::SystemProxyEnvOverride;
pub use crate::route_diagnostics::emit_auth_http_status;
pub use crate::route_diagnostics::emit_auth_network_environment_snapshot;
pub use crate::route_diagnostics::emit_auth_transport_failure;
pub use crate::route_diagnostics::network_diagnostics_enabled;
pub use crate::sse::sse_stream;
pub use crate::telemetry::RequestTelemetry;
pub use crate::transport::ByteStream;
pub use crate::transport::HttpTransport;
pub use crate::transport::ReqwestTransport;
pub use crate::transport::StreamResponse;
