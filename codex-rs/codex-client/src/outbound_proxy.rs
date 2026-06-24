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
#[cfg(any(target_os = "windows", target_os = "macos"))]
use sha2::Digest;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use sha2::Sha256;
use thiserror::Error;

const SYSTEM_PROXY_SUCCESS_CACHE_TTL: Duration = Duration::from_secs(60);
const SYSTEM_PROXY_UNAVAILABLE_CACHE_TTL: Duration = Duration::from_secs(5);
const SYSTEM_PROXY_CACHE_MAX_ENTRIES: usize = 256;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

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
///
/// Unavailable platform resolution falls back to environment proxies and then direct. Errors after
/// a route is selected are returned without trying another route. Ordered PAC candidates are
/// currently collapsed to one route on both Windows and macOS; later proxy or `DIRECT` candidates
/// are not retried after a connection failure.
pub fn build_reqwest_client_for_route(
    builder: reqwest::ClientBuilder,
    request_url: &str,
    route_class: ClientRouteClass,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::Client, BuildRouteAwareHttpClientError> {
    let builder = configure_proxy_for_route(
        &ProcessEnv,
        builder,
        request_url,
        route_class,
        config,
        resolve_system_proxy,
    )?;
    build_reqwest_client_with_custom_ca(builder).map_err(Into::into)
}

fn configure_proxy_for_route(
    env: &dyn EnvSource,
    builder: reqwest::ClientBuilder,
    request_url: &str,
    route_class: ClientRouteClass,
    config: Option<&OutboundProxyConfig>,
    resolve_system_proxy: impl FnOnce(&str, &RequestOrigin) -> SystemProxyDecision,
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

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos")),
    allow(
        dead_code,
        reason = "Direct and Proxy are constructed only by platform-specific resolvers"
    )
)]
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[cfg(target_os = "macos")]
fn resolve_platform_system_proxy(request_url: &str, origin: &RequestOrigin) -> SystemProxyDecision {
    macos::resolve(request_url, origin)
}

#[cfg(target_os = "windows")]
fn resolve_platform_system_proxy(request_url: &str, origin: &RequestOrigin) -> SystemProxyDecision {
    windows::resolve(request_url, origin)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
    let key = system_proxy_cache_key(request_url);
    let cached = cache.get(&key)?;
    if cached.expires_at > Instant::now() {
        return Some(cached.decision.clone());
    }
    cache.remove(&key);
    None
}

fn cache_system_proxy_decision(request_url: &str, decision: SystemProxyDecision) {
    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        let cache_key = system_proxy_cache_key(request_url);
        insert_system_proxy_cache_entry(&mut cache, &cache_key, decision, Instant::now());
    }
}

fn insert_system_proxy_cache_entry(
    cache: &mut HashMap<String, CachedSystemProxyDecision>,
    cache_key: &str,
    decision: SystemProxyDecision,
    now: Instant,
) {
    let ttl = match &decision {
        SystemProxyDecision::Direct | SystemProxyDecision::Proxy { .. } => {
            SYSTEM_PROXY_SUCCESS_CACHE_TTL
        }
        SystemProxyDecision::Unavailable { .. } => SYSTEM_PROXY_UNAVAILABLE_CACHE_TTL,
    };

    cache.retain(|_, cached| cached.expires_at > now);
    if cache.len() >= SYSTEM_PROXY_CACHE_MAX_ENTRIES
        && !cache.contains_key(cache_key)
        && let Some(cache_key_to_evict) = cache
            .iter()
            .min_by_key(|(_, cached)| cached.expires_at)
            .map(|(cache_key, _)| cache_key.clone())
    {
        cache.remove(&cache_key_to_evict);
    }
    cache.insert(
        cache_key.to_string(),
        CachedSystemProxyDecision {
            decision,
            expires_at: now + ttl,
        },
    );
}

fn system_proxy_cache_key(request_url: &str) -> String {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        // Keep URL-specific PAC decisions without retaining the raw routed URL.
        let mut hasher = Sha256::new();
        hasher.update(b"system-proxy-cache-v1\0");
        hasher.update(request_url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    request_url.to_string()
}

#[cfg(any(test, target_os = "windows"))]
fn no_proxy_matches_origin(no_proxy: &str, origin: &RequestOrigin) -> bool {
    no_proxy
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| no_proxy_entry_matches_origin(entry, origin))
}

#[cfg(any(test, target_os = "windows"))]
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

    origin.host == entry
}

#[cfg(any(test, target_os = "windows"))]
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

#[cfg(any(test, target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedProxyListDecision {
    Direct,
    Proxy(String),
    UnsupportedScheme,
    Unavailable,
}

#[cfg(any(test, target_os = "windows"))]
fn parse_proxy_list(input: &str, target_scheme: &str) -> ParsedProxyListDecision {
    let mut saw_unsupported = false;

    {
        let mut process_token = |token: &str| {
            let decision = parse_proxy_token(token, target_scheme);
            match decision {
                ParsedProxyListDecision::Direct => Some(ParsedProxyListDecision::Direct),
                ParsedProxyListDecision::Proxy(url) => Some(ParsedProxyListDecision::Proxy(url)),
                ParsedProxyListDecision::UnsupportedScheme => {
                    saw_unsupported = true;
                    None
                }
                ParsedProxyListDecision::Unavailable => None,
            }
        };

        for segment in input
            .split(';')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
        {
            let mut parts = segment.split_whitespace();
            let directive = parts.next();
            let hostport = parts.next();
            let extra = parts.next();
            let is_proxy_directive = matches!(
                directive.map(str::to_ascii_lowercase).as_deref(),
                Some("proxy" | "http" | "https" | "socks" | "socks4" | "socks5")
            ) && hostport.is_some()
                && extra.is_none();

            if is_proxy_directive {
                if let Some(decision) = process_token(segment) {
                    return decision;
                }
            } else {
                for token in segment.split_whitespace() {
                    if let Some(decision) = process_token(token) {
                        return decision;
                    }
                }
            }
        }
    }

    if saw_unsupported {
        ParsedProxyListDecision::UnsupportedScheme
    } else {
        ParsedProxyListDecision::Unavailable
    }
}

#[cfg(any(test, target_os = "windows"))]
fn parse_proxy_token(token: &str, target_scheme: &str) -> ParsedProxyListDecision {
    if token.eq_ignore_ascii_case("DIRECT") {
        return ParsedProxyListDecision::Direct;
    }

    if let Some(decision) = parse_proxy_key_token(token, target_scheme) {
        return decision;
    }
    if token.contains('=') {
        return ParsedProxyListDecision::Unavailable;
    }

    let mut parts = token.split_whitespace();
    let directive = parts.next();
    let hostport = parts.next();
    if let (Some(directive), Some(hostport), None) = (directive, hostport, parts.next()) {
        return match directive.to_ascii_lowercase().as_str() {
            "proxy" | "http" => proxy_url_from_hostport("http", hostport),
            "https" => proxy_url_from_hostport("https", hostport),
            "socks" | "socks4" | "socks5" => ParsedProxyListDecision::UnsupportedScheme,
            _ => ParsedProxyListDecision::Unavailable,
        };
    }

    proxy_url_from_hostport("http", token)
}

#[cfg(any(test, target_os = "windows"))]
fn parse_proxy_key_token(token: &str, target_scheme: &str) -> Option<ParsedProxyListDecision> {
    let (key, value) = token.split_once('=')?;
    if key.trim().eq_ignore_ascii_case(target_scheme) {
        Some(proxy_url_from_hostport("http", value.trim()))
    } else {
        Some(ParsedProxyListDecision::Unavailable)
    }
}

#[cfg(any(test, target_os = "windows"))]
fn proxy_url_from_hostport(proxy_scheme: &str, hostport: &str) -> ParsedProxyListDecision {
    if hostport.is_empty() {
        return ParsedProxyListDecision::Unavailable;
    }
    if hostport.contains("://") {
        return ParsedProxyListDecision::Proxy(hostport.to_string());
    }
    ParsedProxyListDecision::Proxy(format!("{proxy_scheme}://{hostport}"))
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
        .or_else(|| env.var(&lower))
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "outbound_proxy_tests.rs"]
mod tests;
