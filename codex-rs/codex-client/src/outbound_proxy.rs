//! Conservative outbound proxy selection for resolver-aware clients.
//!
//! This module keeps the default path as close as possible to the existing
//! reqwest builder behavior. For `Auto`, platform system discovery is tried
//! first, explicit environment proxies are delegated back to reqwest as the
//! fallback, and the final fallback is the legacy builder behavior.

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

/// Environment kill switch reserved for system proxy discovery.
///
/// Values such as `off`, `false`, `0`, `no`, or `disabled` disable system/PAC
/// discovery while still allowing explicit environment proxies to be honored by
/// resolver-aware clients.
const CODEX_SYSTEM_PROXY_ENV: &str = "CODEX_SYSTEM_PROXY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemProxyEnvOverride {
    Default,
    Disabled,
}

impl SystemProxyEnvOverride {
    fn from_value(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::Default;
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" | "no" | "disabled" => Self::Disabled,
            _ => Self::Default,
        }
    }

    fn from_env() -> Self {
        Self::from_value(
            /*value*/ std::env::var(CODEX_SYSTEM_PROXY_ENV).ok().as_deref(),
        )
    }

    const fn system_discovery_enabled(self) -> bool {
        matches!(self, Self::Default)
    }
}

/// High-level client path being routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRouteClass {
    Auth,
    Api,
    WebSocket,
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

/// How a resolver-aware client should choose an outbound proxy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OutboundProxyMode {
    /// Try system discovery where supported, then explicit env proxy settings, then legacy behavior.
    #[default]
    Auto,
    /// Preserve only the existing reqwest/env proxy path.
    Env,
    /// Require supported system discovery, including WPAD auto-detect on supported platforms.
    System,
    /// Disable proxy use for this client.
    Direct,
}

/// Optional route-selection inputs for resolver-aware clients.
///
/// This is intentionally policy-only. Resolved proxy endpoints, PAC URLs, and
/// platform resolver details stay internal to the client builder.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct OutboundProxyConfig {
    mode: OutboundProxyMode,
}

impl OutboundProxyConfig {
    pub const fn new(mode: OutboundProxyMode) -> Self {
        Self { mode }
    }

    pub const fn mode(&self) -> OutboundProxyMode {
        self.mode
    }
}

impl fmt::Debug for OutboundProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboundProxyConfig")
            .field("mode", &self.mode)
            .finish()
    }
}

/// Error while building a resolver-aware reqwest client.
#[derive(Debug, Error)]
pub enum BuildRouteAwareHttpClientError {
    #[error(transparent)]
    CustomCa(#[from] BuildCustomCaTransportError),

    #[error("Failed to configure outbound proxy selected for {route_class}")]
    InvalidProxyConfig { route_class: ClientRouteClass },

    #[error("System proxy resolution for {route_class} failed: {failure}")]
    SystemProxyUnavailable {
        route_class: ClientRouteClass,
        failure: RouteFailureClass,
    },
}

impl From<BuildRouteAwareHttpClientError> for io::Error {
    fn from(error: BuildRouteAwareHttpClientError) -> Self {
        match error {
            BuildRouteAwareHttpClientError::CustomCa(error) => error.into(),
            BuildRouteAwareHttpClientError::InvalidProxyConfig { .. }
            | BuildRouteAwareHttpClientError::SystemProxyUnavailable { .. } => {
                io::Error::other(error)
            }
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
    let builder = configure_proxy_for_route(builder, request_url, route_class, config)?;
    build_reqwest_client_with_custom_ca(builder).map_err(Into::into)
}

fn configure_proxy_for_route(
    builder: reqwest::ClientBuilder,
    request_url: &str,
    route_class: ClientRouteClass,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    let Some(config) = config else {
        return Ok(builder);
    };
    let origin = RequestOrigin::parse(request_url);

    if config.mode == OutboundProxyMode::Direct {
        return Ok(builder.no_proxy());
    }

    // Route contract: OS/PAC bypass decisions are returned by platform
    // resolution as `Direct` and disable proxy use for this client. Env
    // `NO_PROXY` is an env-derived direct decision; reqwest remains the
    // authority for applying the env proxy contract to its own env proxies.
    if origin.as_ref().is_some_and(no_proxy_env_matches_origin) {
        if config.mode == OutboundProxyMode::Env {
            return Ok(builder.no_proxy());
        }
        return Ok(builder);
    }

    if config.mode == OutboundProxyMode::Env {
        return configure_env_proxy_handling(
            builder,
            origin.as_ref(),
            route_class,
            EnvProxyHandling::ResolveConcreteOrNoProxy,
        );
    }

    if !SystemProxyEnvOverride::from_env().system_discovery_enabled() {
        if config.mode == OutboundProxyMode::System {
            return Err(BuildRouteAwareHttpClientError::SystemProxyUnavailable {
                route_class,
                failure: RouteFailureClass::ProxyResolutionUnavailable,
            });
        }
        return configure_env_proxy_handling(
            builder,
            origin.as_ref(),
            route_class,
            EnvProxyHandling::DelegateToReqwest,
        );
    }

    if !system_proxy_supported() {
        if config.mode == OutboundProxyMode::System {
            return Err(BuildRouteAwareHttpClientError::SystemProxyUnavailable {
                route_class,
                failure: RouteFailureClass::ProxyResolutionUnavailable,
            });
        }
        return configure_env_proxy_handling(
            builder,
            origin.as_ref(),
            route_class,
            EnvProxyHandling::DelegateToReqwest,
        );
    }

    let Some(origin) = origin.as_ref() else {
        return if config.mode == OutboundProxyMode::System {
            Err(BuildRouteAwareHttpClientError::SystemProxyUnavailable {
                route_class,
                failure: RouteFailureClass::InvalidProxyConfig,
            })
        } else {
            configure_env_proxy_handling(
                builder,
                /*origin*/ None,
                route_class,
                EnvProxyHandling::DelegateToReqwest,
            )
        };
    };

    let include_auto_detect = matches!(
        config.mode,
        OutboundProxyMode::Auto | OutboundProxyMode::System
    );
    match resolve_system_proxy(request_url, origin, include_auto_detect) {
        SystemProxyDecision::Direct => Ok(builder.no_proxy()),
        SystemProxyDecision::Proxy { url } => configure_concrete_proxy(builder, route_class, &url),
        SystemProxyDecision::Unavailable { failure } => {
            if config.mode == OutboundProxyMode::System {
                Err(BuildRouteAwareHttpClientError::SystemProxyUnavailable {
                    route_class,
                    failure,
                })
            } else {
                configure_env_proxy_handling(
                    builder,
                    Some(origin),
                    route_class,
                    EnvProxyHandling::DelegateToReqwest,
                )
            }
        }
    }
}

const fn system_proxy_supported() -> bool {
    false
}

fn configure_concrete_proxy(
    builder: reqwest::ClientBuilder,
    route_class: ClientRouteClass,
    proxy_url: &str,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    let proxy = match reqwest::Proxy::all(proxy_url) {
        Ok(proxy) => proxy,
        Err(_source) => {
            return Err(BuildRouteAwareHttpClientError::InvalidProxyConfig { route_class });
        }
    };
    let proxy = proxy.no_proxy(reqwest::NoProxy::from_env());
    Ok(builder.proxy(proxy))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvProxyHandling {
    ResolveConcreteOrNoProxy,
    DelegateToReqwest,
}

fn configure_env_proxy_handling(
    builder: reqwest::ClientBuilder,
    origin: Option<&RequestOrigin>,
    route_class: ClientRouteClass,
    handling: EnvProxyHandling,
) -> Result<reqwest::ClientBuilder, BuildRouteAwareHttpClientError> {
    if let Some(origin) = origin
        && let Some(proxy_url) = env_proxy_for_origin(origin)
    {
        if handling == EnvProxyHandling::ResolveConcreteOrNoProxy {
            return configure_concrete_proxy(builder, route_class, &proxy_url);
        }
        return Ok(builder);
    }

    if conventional_proxy_env_present() {
        return match handling {
            EnvProxyHandling::ResolveConcreteOrNoProxy => Ok(builder.no_proxy()),
            EnvProxyHandling::DelegateToReqwest => Ok(builder),
        };
    }

    match handling {
        EnvProxyHandling::ResolveConcreteOrNoProxy => Ok(builder.no_proxy()),
        EnvProxyHandling::DelegateToReqwest => Ok(builder),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        let port = uri
            .port_u16()
            .or_else(|| default_port_for_scheme(&scheme))?;
        Some(Self { scheme, host, port })
    }
}

fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum SystemProxyDecision {
    Direct,
    Proxy { url: String },
    Unavailable { failure: RouteFailureClass },
}

fn resolve_system_proxy(
    request_url: &str,
    origin: &RequestOrigin,
    include_auto_detect: bool,
) -> SystemProxyDecision {
    if let Some(decision) = cached_system_proxy_decision(request_url, include_auto_detect) {
        return decision;
    }

    let decision = resolve_platform_system_proxy(request_url, origin, include_auto_detect);
    cache_system_proxy_decision(request_url, include_auto_detect, decision.clone());
    decision
}

fn resolve_platform_system_proxy(
    _request_url: &str,
    _origin: &RequestOrigin,
    _include_auto_detect: bool,
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

fn cached_system_proxy_decision(
    request_url: &str,
    include_auto_detect: bool,
) -> Option<SystemProxyDecision> {
    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().ok()?;
    let key = system_proxy_cache_key(request_url, include_auto_detect);
    let cached = cache.get(&key)?;
    if cached.expires_at > Instant::now() {
        return Some(cached.decision.clone());
    }
    cache.remove(&key);
    None
}

fn cache_system_proxy_decision(
    request_url: &str,
    include_auto_detect: bool,
    decision: SystemProxyDecision,
) {
    if matches!(decision, SystemProxyDecision::Unavailable { .. }) {
        return;
    }

    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        let now = Instant::now();
        cache.retain(|_, cached| cached.expires_at > now);
        cache.insert(
            system_proxy_cache_key(request_url, include_auto_detect),
            CachedSystemProxyDecision {
                decision,
                expires_at: now + SYSTEM_PROXY_SUCCESS_CACHE_TTL,
            },
        );
    }
}

fn system_proxy_cache_key(request_url: &str, include_auto_detect: bool) -> String {
    format!("{request_url}:auto_detect={include_auto_detect}")
}

fn conventional_proxy_env_present() -> bool {
    proxy_env_value("HTTPS_PROXY").is_some()
        || proxy_env_value("HTTP_PROXY").is_some()
        || proxy_env_value("ALL_PROXY").is_some()
}

fn env_proxy_for_origin(origin: &RequestOrigin) -> Option<String> {
    if origin.scheme == "https" {
        proxy_env_value("HTTPS_PROXY").or_else(|| proxy_env_value("ALL_PROXY"))
    } else if origin.scheme == "http" {
        proxy_env_value("HTTP_PROXY").or_else(|| proxy_env_value("ALL_PROXY"))
    } else {
        proxy_env_value("ALL_PROXY")
    }
}

fn proxy_env_value(upper: &str) -> Option<String> {
    let lower = upper.to_ascii_lowercase();
    std::env::var(upper)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var(lower).ok().filter(|value| !value.is_empty()))
}

fn no_proxy_env_matches_origin(origin: &RequestOrigin) -> bool {
    let Some(no_proxy) = proxy_env_value("NO_PROXY") else {
        return false;
    };
    no_proxy_matches_origin(&no_proxy, origin)
}

fn no_proxy_matches_origin(no_proxy: &str, origin: &RequestOrigin) -> bool {
    no_proxy
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| no_proxy_entry_matches_origin(entry, origin))
}

fn no_proxy_entry_matches_origin(entry: &str, origin: &RequestOrigin) -> bool {
    if entry == "*" {
        return true;
    }

    let mut entry = entry
        .strip_prefix("http://")
        .or_else(|| entry.strip_prefix("https://"))
        .unwrap_or(entry)
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    let mut port = None;
    let parsed_host_port = entry.rsplit_once(':').and_then(|(host, candidate_port)| {
        if host.contains(':') {
            return None;
        }
        candidate_port
            .parse::<u16>()
            .ok()
            .map(|parsed_port| (host.to_string(), parsed_port))
    });
    if let Some((host, parsed_port)) = parsed_host_port {
        entry = host;
        port = Some(parsed_port);
    }
    if port.is_some_and(|port| port != origin.port) {
        return false;
    }

    if let Some(suffix) = entry.strip_prefix('.') {
        return origin.host == suffix || origin.host.ends_with(&format!(".{suffix}"));
    }

    if entry.contains('*') {
        return wildcard_host_match(&entry, &origin.host);
    }

    origin.host == entry || origin.host.ends_with(&format!(".{entry}"))
}

fn wildcard_host_match(pattern: &str, host: &str) -> bool {
    let mut remaining = host;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            continue;
        }
        if first && !pattern.starts_with('*') {
            let Some(stripped) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = stripped;
        } else {
            let Some(index) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[index + part.len()..];
        }
        first = false;
    }
    pattern.ends_with('*') || remaining.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_matches_exact_suffix_wildcard_and_port() {
        let origin = RequestOrigin {
            scheme: "https".to_string(),
            host: "auth.openai.com".to_string(),
            port: 443,
        };
        assert!(no_proxy_matches_origin("auth.openai.com", &origin));
        assert!(no_proxy_matches_origin(".openai.com", &origin));
        assert!(no_proxy_matches_origin("*.openai.com", &origin));
        assert!(no_proxy_matches_origin("auth.openai.com:443", &origin));
        assert!(!no_proxy_matches_origin("auth.openai.com:8443", &origin));
    }
}
