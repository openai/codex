//! Conservative outbound proxy selection for resolver-aware clients.
//!
//! When enabled, platform system discovery is tried first, explicit environment
//! proxies are the fallback, and the final fallback is a direct connection.
//! When disabled, callers retain the existing reqwest builder behavior.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use crate::custom_ca::BuildCustomCaTransportError;
use crate::custom_ca::build_reqwest_client_with_custom_ca;
use thiserror::Error;

const SYSTEM_PROXY_SUCCESS_CACHE_TTL: Duration = Duration::from_secs(60);

/// Coarse semantic bucket for the HTTP or WebSocket client being constructed.
///
/// This is not the selected proxy route or a concrete endpoint. It labels the
/// product path that owns the client so proxy-resolution diagnostics can
/// distinguish auth, API, WebSocket, and miscellaneous traffic without exposing
/// endpoint details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRouteClass {
    /// Login, token refresh/revoke, PAT, and agent identity auth traffic.
    Auth,
    /// First-party API traffic that is not part of the auth flow.
    Api,
    /// WebSocket traffic.
    WebSocket,
    /// Call sites without a more specific route class.
    Other,
}

impl fmt::Display for ClientRouteClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auth => "auth",
            Self::Api => "api",
            Self::WebSocket => "wss",
            Self::Other => "other",
        })
    }
}

/// Coarse failure class for route selection errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteFailureClass {
    ProxyResolutionUnavailable,
    ConnectTimeout,
    ProxyAuthenticationRequired,
    TlsError,
    InvalidProxyConfig,
    UnsupportedProxyScheme,
    ResolverError,
}

impl fmt::Display for RouteFailureClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProxyResolutionUnavailable => "proxy_resolution_unavailable",
            Self::ConnectTimeout => "connect_timeout",
            Self::ProxyAuthenticationRequired => "proxy_407",
            Self::TlsError => "tls_error",
            Self::InvalidProxyConfig => "invalid_proxy_config",
            Self::UnsupportedProxyScheme => "unsupported_proxy_scheme",
            Self::ResolverError => "resolver_error",
        })
    }
}

/// Marker enabling fixed system/PAC/WPAD, environment, then direct routing.
/// Resolved endpoints and platform details remain internal to the client builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboundProxyConfig;

impl OutboundProxyConfig {
    pub const fn respect_system_proxy() -> Self {
        Self
    }
}

/// Error while building a resolver-aware reqwest client.
#[derive(Debug, Error)]
pub enum BuildRouteAwareHttpClientError {
    #[error(transparent)]
    CustomCa(#[from] BuildCustomCaTransportError),

    #[error("Failed to configure outbound proxy selected for {route_class}")]
    InvalidProxyConfig { route_class: ClientRouteClass },
}

impl From<BuildRouteAwareHttpClientError> for io::Error {
    fn from(error: BuildRouteAwareHttpClientError) -> Self {
        match error {
            BuildRouteAwareHttpClientError::CustomCa(error) => error.into(),
            BuildRouteAwareHttpClientError::InvalidProxyConfig { .. } => io::Error::other(error),
        }
    }
}

/// Builds a reqwest client with conservative route selection and shared CA handling.
pub fn build_reqwest_client_for_route(
    builder: reqwest::ClientBuilder,
    request_url: &str,
    route_class: ClientRouteClass,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::Client, BuildRouteAwareHttpClientError> {
    let builder =
        configure_proxy_for_route(&ProcessEnv, builder, request_url, route_class, config)?;
    build_reqwest_client_with_custom_ca(builder).map_err(Into::into)
}

fn configure_proxy_for_route(
    env: &dyn EnvSource,
    builder: reqwest::ClientBuilder,
    request_url: &str,
    route_class: ClientRouteClass,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    if config.is_none() {
        return Ok(builder);
    }
    let origin = RequestOrigin::parse(request_url);

    let Some(origin) = origin.as_ref() else {
        return configure_env_proxy_handling(env, builder, /*origin*/ None, route_class);
    };

    match resolve_system_proxy(request_url, origin) {
        SystemProxyDecision::Direct => Ok(builder.no_proxy()),
        SystemProxyDecision::Proxy { url } => {
            configure_concrete_proxy(builder, route_class, &url, /*no_proxy*/ None)
        }
        SystemProxyDecision::Unavailable { .. } => {
            configure_env_proxy_handling(env, builder, Some(origin), route_class)
        }
    }
}

fn configure_concrete_proxy(
    builder: reqwest::ClientBuilder,
    route_class: ClientRouteClass,
    proxy_url: &str,
    no_proxy: Option<reqwest::NoProxy>,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    let proxy = match reqwest::Proxy::all(proxy_url) {
        Ok(proxy) => proxy,
        Err(_source) => {
            return Err(BuildRouteAwareHttpClientError::InvalidProxyConfig { route_class });
        }
    };
    Ok(builder.proxy(proxy.no_proxy(no_proxy)))
}

fn configure_env_proxy_handling(
    env: &dyn EnvSource,
    builder: reqwest::ClientBuilder,
    origin: Option<&RequestOrigin>,
    route_class: ClientRouteClass,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    if let Some(origin) = origin {
        let proxy_url = match origin.scheme.as_str() {
            "https" => {
                proxy_env_value(env, "HTTPS_PROXY").or_else(|| proxy_env_value(env, "ALL_PROXY"))
            }
            "http" => {
                proxy_env_value(env, "HTTP_PROXY").or_else(|| proxy_env_value(env, "ALL_PROXY"))
            }
            _ => proxy_env_value(env, "ALL_PROXY"),
        };
        if let Some(proxy_url) = proxy_url {
            let no_proxy = proxy_env_value(env, "NO_PROXY")
                .and_then(|value| reqwest::NoProxy::from_string(&value));
            return configure_concrete_proxy(builder, route_class, &proxy_url, no_proxy);
        }
    }
    Ok(builder.no_proxy())
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
struct RequestOrigin {
    scheme: String,
    host: String,
    port: u16,
}

impl RequestOrigin {
    fn parse(request_url: &str) -> Option<Self> {
        let uri = request_url.parse::<http::Uri>().ok()?;
        let scheme = uri.scheme_str()?.to_ascii_lowercase();
        let host = uri.host()?.trim_matches(['[', ']']).to_ascii_lowercase();
        let port = uri.port_u16().or(match scheme.as_str() {
            "http" => Some(80),
            "https" => Some(443),
            _ => None,
        })?;
        Some(Self { scheme, host, port })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum SystemProxyDecision {
    Direct,
    Proxy { url: String },
    Unavailable { failure: RouteFailureClass },
}

fn resolve_system_proxy(request_url: &str, origin: &RequestOrigin) -> SystemProxyDecision {
    if let Some(decision) = cached_system_proxy_decision(request_url) {
        return decision;
    }

    let decision = resolve_platform_system_proxy(request_url, origin);
    cache_system_proxy_decision(request_url, decision.clone());
    decision
}

fn resolve_platform_system_proxy(
    _request_url: &str,
    _origin: &RequestOrigin,
) -> SystemProxyDecision {
    SystemProxyDecision::Unavailable {
        failure: RouteFailureClass::ProxyResolutionUnavailable,
    }
}

#[derive(Debug, Clone)]
struct CachedSystemProxyDecision {
    decision: SystemProxyDecision,
    expires_at: Instant,
}

static SYSTEM_PROXY_CACHE: OnceLock<Mutex<HashMap<String, CachedSystemProxyDecision>>> =
    OnceLock::new();

fn cached_system_proxy_decision(request_url: &str) -> Option<SystemProxyDecision> {
    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().ok()?;
    let cached = cache.get(request_url)?;
    if cached.expires_at > Instant::now() {
        return Some(cached.decision.clone());
    }
    cache.remove(request_url);
    None
}

fn cache_system_proxy_decision(request_url: &str, decision: SystemProxyDecision) {
    if matches!(decision, SystemProxyDecision::Unavailable { .. }) {
        return;
    }

    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        let now = Instant::now();
        cache.retain(|_, cached| cached.expires_at > now);
        cache.insert(
            request_url.to_string(),
            CachedSystemProxyDecision {
                decision,
                expires_at: now + SYSTEM_PROXY_SUCCESS_CACHE_TTL,
            },
        );
    }
}

trait EnvSource {
    fn var(&self, key: &str) -> Option<String>;
}

struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

fn proxy_env_value(env: &dyn EnvSource, upper: &str) -> Option<String> {
    let lower = upper.to_ascii_lowercase();
    env.var(upper)
        .filter(|value| !value.is_empty())
        .or_else(|| env.var(&lower).filter(|value| !value.is_empty()))
}

#[cfg(test)]
#[path = "outbound_proxy_tests.rs"]
mod tests;
