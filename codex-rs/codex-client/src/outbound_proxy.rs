//! Conservative outbound proxy selection for resolver-aware clients.
//!
//! This module keeps the default path as close as possible to the existing
//! reqwest builder behavior. Explicit environment proxies are delegated back to
//! reqwest so its existing proxy and `NO_PROXY` behavior stays in control. The
//! Windows system resolver is only consulted when there is no explicit env
//! proxy and the caller allows system discovery.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use crate::custom_ca::BuildCustomCaTransportError;
use crate::custom_ca::build_reqwest_client_with_custom_ca;
use crate::custom_ca::custom_ca_env_configured;
use crate::route_diagnostics::RouteDiagnostic;
use crate::route_diagnostics::RouteFailureClass;
use crate::route_diagnostics::RouteSource;
use crate::route_diagnostics::RouteTarget;
use crate::route_diagnostics::SystemProxyEnvOverride;
use thiserror::Error;

const SYSTEM_PROXY_CACHE_TTL: Duration = Duration::from_secs(300);

#[cfg(target_os = "windows")]
mod windows;

/// How a resolver-aware client should choose an outbound proxy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OutboundProxyMode {
    /// Preserve explicit env proxy behavior, then use explicit system proxy/PAC settings.
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
#[derive(Default, Clone, PartialEq, Eq)]
pub struct OutboundProxyConfig {
    pub mode: OutboundProxyMode,
    pub proxy_url: Option<String>,
}

impl fmt::Debug for OutboundProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboundProxyConfig")
            .field("mode", &self.mode)
            .field(
                "proxy_url",
                &self.proxy_url.as_ref().map(|_| "<redacted-proxy-url>"),
            )
            .finish()
    }
}

/// Error while building a resolver-aware reqwest client.
#[derive(Debug, Error)]
pub enum BuildProxiedHttpClientError {
    #[error(transparent)]
    CustomCa(#[from] BuildCustomCaTransportError),

    #[error("Failed to configure outbound proxy selected for {target}")]
    InvalidProxyConfig { target: RouteTarget },

    #[error("System proxy resolution for {target} failed: {failure}")]
    SystemProxyUnavailable {
        target: RouteTarget,
        failure: RouteFailureClass,
    },
}

impl From<BuildProxiedHttpClientError> for io::Error {
    fn from(error: BuildProxiedHttpClientError) -> Self {
        match error {
            BuildProxiedHttpClientError::CustomCa(error) => error.into(),
            BuildProxiedHttpClientError::InvalidProxyConfig { .. }
            | BuildProxiedHttpClientError::SystemProxyUnavailable { .. } => io::Error::other(error),
        }
    }
}

/// Builds a reqwest client with conservative route selection and shared CA handling.
pub fn build_reqwest_client_for_route(
    builder: reqwest::ClientBuilder,
    request_url: &str,
    target: RouteTarget,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::Client, BuildProxiedHttpClientError> {
    let builder = configure_proxy_for_route(builder, request_url, target, config)?;
    build_reqwest_client_with_custom_ca(builder).map_err(Into::into)
}

fn configure_proxy_for_route(
    builder: reqwest::ClientBuilder,
    request_url: &str,
    target: RouteTarget,
    config: Option<&OutboundProxyConfig>,
) -> Result<reqwest::ClientBuilder, BuildProxiedHttpClientError> {
    let config = config.cloned().unwrap_or_default();
    let custom_ca_configured = custom_ca_env_configured();
    let origin = RequestOrigin::parse(request_url);

    if config.mode == OutboundProxyMode::Direct {
        RouteDiagnostic::direct(target, RouteSource::Disabled, custom_ca_configured).emit_opt_in();
        return Ok(builder.no_proxy());
    }

    if origin.as_ref().is_some_and(no_proxy_env_matches_origin) {
        RouteDiagnostic::direct(target, RouteSource::Env, custom_ca_configured).emit_opt_in();
        return Ok(builder);
    }

    if let Some(proxy_url) = config
        .proxy_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return configure_concrete_proxy(
            builder,
            target,
            RouteSource::ConfigOverride,
            proxy_url,
            custom_ca_configured,
        );
    }

    if let Some(origin) = origin.as_ref()
        && let Some(proxy_url) = env_proxy_for_origin(origin)
    {
        RouteDiagnostic::proxy(target, RouteSource::Env, &proxy_url, custom_ca_configured)
            .emit_opt_in();
        return Ok(builder);
    }

    if conventional_proxy_env_present() {
        RouteDiagnostic::direct(target, RouteSource::Env, custom_ca_configured).emit_opt_in();
        return Ok(builder);
    }

    if config.mode == OutboundProxyMode::Env
        || !SystemProxyEnvOverride::from_env().system_discovery_enabled()
    {
        RouteDiagnostic::direct(target, RouteSource::Disabled, custom_ca_configured).emit_opt_in();
        return Ok(builder);
    }

    if !system_proxy_supported() {
        if config.mode == OutboundProxyMode::System {
            RouteDiagnostic::unavailable(
                target,
                RouteSource::Unavailable,
                RouteFailureClass::PacUnavailable,
                custom_ca_configured,
            )
            .emit_opt_in();
            return Err(BuildProxiedHttpClientError::SystemProxyUnavailable {
                target,
                failure: RouteFailureClass::PacUnavailable,
            });
        }
        return Ok(builder);
    }

    let Some(origin) = origin else {
        RouteDiagnostic::unavailable(
            target,
            RouteSource::Error,
            RouteFailureClass::InvalidProxyConfig,
            custom_ca_configured,
        )
        .emit_opt_in();
        return if config.mode == OutboundProxyMode::System {
            Err(BuildProxiedHttpClientError::SystemProxyUnavailable {
                target,
                failure: RouteFailureClass::InvalidProxyConfig,
            })
        } else {
            Ok(builder)
        };
    };

    let include_auto_detect = config.mode == OutboundProxyMode::System;
    match resolve_system_proxy(request_url, &origin, include_auto_detect) {
        SystemProxyDecision::Direct { source } => {
            RouteDiagnostic::direct(target, source, custom_ca_configured).emit_opt_in();
            Ok(builder.no_proxy())
        }
        SystemProxyDecision::Proxy { source, url } => {
            configure_concrete_proxy(builder, target, source, &url, custom_ca_configured)
        }
        SystemProxyDecision::Unavailable { source, failure } => {
            RouteDiagnostic::unavailable(target, source, failure, custom_ca_configured)
                .emit_opt_in();
            if config.mode == OutboundProxyMode::System {
                Err(BuildProxiedHttpClientError::SystemProxyUnavailable { target, failure })
            } else {
                Ok(builder)
            }
        }
    }
}

const fn system_proxy_supported() -> bool {
    cfg!(target_os = "windows")
}

fn configure_concrete_proxy(
    builder: reqwest::ClientBuilder,
    target: RouteTarget,
    source: RouteSource,
    proxy_url: &str,
    custom_ca_configured: bool,
) -> Result<reqwest::ClientBuilder, BuildProxiedHttpClientError> {
    let proxy = match reqwest::Proxy::all(proxy_url) {
        Ok(proxy) => proxy,
        Err(_source) => {
            RouteDiagnostic::unavailable(
                target,
                source,
                RouteFailureClass::InvalidProxyConfig,
                custom_ca_configured,
            )
            .emit_opt_in();
            return Err(BuildProxiedHttpClientError::InvalidProxyConfig { target });
        }
    };
    RouteDiagnostic::proxy(target, source, proxy_url, custom_ca_configured).emit_opt_in();
    Ok(builder.proxy(proxy))
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

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum SystemProxyDecision {
    Direct {
        source: RouteSource,
    },
    Proxy {
        source: RouteSource,
        url: String,
    },
    Unavailable {
        source: RouteSource,
        failure: RouteFailureClass,
    },
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

#[cfg(target_os = "windows")]
fn resolve_platform_system_proxy(
    request_url: &str,
    origin: &RequestOrigin,
    include_auto_detect: bool,
) -> SystemProxyDecision {
    windows::resolve(request_url, origin, include_auto_detect)
}

#[cfg(not(target_os = "windows"))]
fn resolve_platform_system_proxy(
    _request_url: &str,
    _origin: &RequestOrigin,
    _include_auto_detect: bool,
) -> SystemProxyDecision {
    SystemProxyDecision::Unavailable {
        source: RouteSource::Unavailable,
        failure: RouteFailureClass::PacUnavailable,
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
    let cache = SYSTEM_PROXY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.insert(
            system_proxy_cache_key(request_url, include_auto_detect),
            CachedSystemProxyDecision {
                decision,
                expires_at: Instant::now() + SYSTEM_PROXY_CACHE_TTL,
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
    let mut http_fallback = None;
    for token in input
        .split(';')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if target_scheme == "https"
            && http_fallback.is_none()
            && let Some(ParsedProxyListDecision::Proxy(url)) = parse_proxy_key_token(token, "http")
        {
            http_fallback = Some(url);
        }
        match parse_proxy_token(token, target_scheme) {
            ParsedProxyListDecision::Direct => return ParsedProxyListDecision::Direct,
            ParsedProxyListDecision::Proxy(url) => return ParsedProxyListDecision::Proxy(url),
            ParsedProxyListDecision::UnsupportedScheme => saw_unsupported = true,
            ParsedProxyListDecision::Unavailable => {}
        }
    }

    if let Some(url) = http_fallback {
        ParsedProxyListDecision::Proxy(url)
    } else if saw_unsupported {
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

    if let Some((scheme, hostport)) = token.split_once(' ') {
        let scheme = scheme.trim().to_ascii_lowercase();
        let hostport = hostport.trim();
        return match scheme.as_str() {
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

#[cfg(any(test, target_os = "windows"))]
impl fmt::Display for ParsedProxyListDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::Proxy(_) => f.write_str("proxy"),
            Self::UnsupportedScheme => f.write_str("unsupported_scheme"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_pac_proxy_tokens() {
        assert_eq!(
            parse_proxy_list("PROXY proxy.internal:8080; DIRECT", "https"),
            ParsedProxyListDecision::Proxy("http://proxy.internal:8080".to_string())
        );
        assert_eq!(
            parse_proxy_list("HTTPS proxy.internal:8443", "https"),
            ParsedProxyListDecision::Proxy("https://proxy.internal:8443".to_string())
        );
    }

    #[test]
    fn parses_static_winhttp_proxy_entries_for_target_scheme() {
        assert_eq!(
            parse_proxy_list("http=web-proxy:8080;https=secure-proxy:8443", "https"),
            ParsedProxyListDecision::Proxy("http://secure-proxy:8443".to_string())
        );
        assert_eq!(
            parse_proxy_list("proxy.internal:8080", "https"),
            ParsedProxyListDecision::Proxy("http://proxy.internal:8080".to_string())
        );
    }

    #[test]
    fn reports_direct_and_unsupported_proxy_tokens() {
        assert_eq!(
            parse_proxy_list("PROXY proxy.internal:8080; DIRECT", "https"),
            ParsedProxyListDecision::Proxy("http://proxy.internal:8080".to_string())
        );
        assert_eq!(
            parse_proxy_list("DIRECT; PROXY proxy.internal:8080", "https"),
            ParsedProxyListDecision::Direct
        );
        assert_eq!(
            parse_proxy_list("DIRECT", "https"),
            ParsedProxyListDecision::Direct
        );
        assert_eq!(
            parse_proxy_list("SOCKS proxy.internal:1080", "https"),
            ParsedProxyListDecision::UnsupportedScheme
        );
    }

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

    #[test]
    fn system_proxy_cache_key_preserves_url_specific_pac_decisions() {
        assert_ne!(
            system_proxy_cache_key("https://auth.openai.com/oauth/token", false),
            system_proxy_cache_key("https://auth.openai.com/oauth/revoke", false)
        );
    }
}
