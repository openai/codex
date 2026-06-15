//! Sanitized, opt-in diagnostics for resolver-aware HTTP clients.
//!
//! Values emitted from this module must not contain request URLs, PAC URLs,
//! proxy hostnames, credentials, headers, tokens, or certificate paths.

use std::fmt;

use crate::outbound_proxy::ClientRouteClass;
use crate::outbound_proxy::RouteFailureClass;

/// Opt-in switch for sanitized network diagnostics.
///
/// Set to `1`, `true`, `on`, or `yes` to emit diagnostic events. The configured
/// value itself is never logged.
const CODEX_NETWORK_DIAGNOSTICS_ENV: &str = "CODEX_NETWORK_DIAGNOSTICS";

const CODEX_SYSTEM_PROXY_ENV: &str = "CODEX_SYSTEM_PROXY";

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).ok().as_deref().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on" | "yes"
        )
    })
}

/// Returns whether sanitized network diagnostics are enabled for this process.
fn network_diagnostics_enabled() -> bool {
    env_flag_enabled(CODEX_NETWORK_DIAGNOSTICS_ENV)
}

fn env_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn proxy_env_present(upper: &str, lower: &str) -> bool {
    env_present(upper) || env_present(lower)
}

fn system_proxy_override_state() -> &'static str {
    let disabled = std::env::var(CODEX_SYSTEM_PROXY_ENV)
        .ok()
        .as_deref()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "off" | "false" | "0" | "no" | "disabled"
            )
        });
    if disabled { "disabled" } else { "default" }
}

/// Emits environment presence bits for an auth operation.
///
/// Proxy values, CA paths, URLs, headers, and tokens are intentionally omitted.
pub fn emit_auth_network_environment_snapshot(operation: &'static str) {
    if !network_diagnostics_enabled() {
        return;
    }
    tracing::info!(
        target_class = "auth",
        operation = operation,
        http_proxy_present = proxy_env_present("HTTP_PROXY", "http_proxy"),
        https_proxy_present = proxy_env_present("HTTPS_PROXY", "https_proxy"),
        all_proxy_present = proxy_env_present("ALL_PROXY", "all_proxy"),
        no_proxy_present = proxy_env_present("NO_PROXY", "no_proxy"),
        codex_system_proxy = system_proxy_override_state(),
        custom_ca_present = env_present("CODEX_CA_CERTIFICATE") || env_present("SSL_CERT_FILE"),
        "opt-in auth network diagnostic snapshot"
    );
}

fn classify_reqwest_error(error: &reqwest::Error) -> Option<RouteFailureClass> {
    if error.is_timeout() {
        return Some(RouteFailureClass::ConnectTimeout);
    }
    if error.status().is_some_and(|status| status.as_u16() == 407) {
        return Some(RouteFailureClass::ProxyAuthenticationRequired);
    }
    let rendered = error.to_string().to_ascii_lowercase();
    if rendered.contains("tls") || rendered.contains("certificate") || rendered.contains("cert") {
        return Some(RouteFailureClass::TlsError);
    }
    if error.is_connect() {
        return Some(RouteFailureClass::ResolverError);
    }
    None
}

/// Emits a coarse auth transport failure without the error text or URL.
pub fn emit_auth_transport_failure(operation: &'static str, error: &reqwest::Error) {
    if !network_diagnostics_enabled() {
        return;
    }
    let failure = classify_reqwest_error(error)
        .map(|failure| failure.to_string())
        .unwrap_or_else(|| "other".to_string());
    tracing::info!(
        target_class = "auth",
        operation = operation,
        failure = %failure,
        is_timeout = error.is_timeout(),
        is_connect = error.is_connect(),
        status_present = error.status().is_some(),
        status = error
            .status()
            .map(|status| status.as_u16())
            .unwrap_or(/*default*/ 0),
        "opt-in auth network transport diagnostic"
    );
}

/// Emits an auth HTTP status without response content or endpoint details.
pub fn emit_auth_http_status(operation: &'static str, status: reqwest::StatusCode) {
    if !network_diagnostics_enabled() {
        return;
    }
    let failure = if status.as_u16() == 407 {
        RouteFailureClass::ProxyAuthenticationRequired.to_string()
    } else {
        "other".to_string()
    };
    tracing::info!(
        target_class = "auth",
        operation = operation,
        status = status.as_u16(),
        failure = %failure,
        "opt-in auth network HTTP status diagnostic"
    );
}

/// Source that produced a route decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteDecisionSource {
    Env,
    ConfigDisabled,
    UnsupportedPlatform,
    ResolutionError,
    #[cfg(target_os = "macos")]
    MacOsCfNetworkPac,
    #[cfg(target_os = "macos")]
    MacOsSystem,
    #[cfg(target_os = "windows")]
    WindowsWinHttpPac,
    #[cfg(target_os = "windows")]
    WindowsStatic,
    Direct,
}

impl fmt::Display for RouteDecisionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Env => "env",
            Self::ConfigDisabled => "config_disabled",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::ResolutionError => "resolution_error",
            #[cfg(target_os = "macos")]
            Self::MacOsCfNetworkPac => "macos_cfnetwork_pac",
            #[cfg(target_os = "macos")]
            Self::MacOsSystem => "macos_system",
            #[cfg(target_os = "windows")]
            Self::WindowsWinHttpPac => "windows_winhttp_pac",
            #[cfg(target_os = "windows")]
            Self::WindowsStatic => "windows_static",
            Self::Direct => "direct",
        })
    }
}

/// A proxy endpoint rendered without credentials, hostname, path, or query.
#[derive(Clone, PartialEq, Eq)]
struct RedactedProxyEndpoint(String);

impl RedactedProxyEndpoint {
    fn parse(input: &str) -> Self {
        let Some((scheme, rest)) = input.split_once("://") else {
            return Self("<invalid-proxy-url>".to_string());
        };
        if scheme.is_empty()
            || !scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        {
            return Self("<invalid-proxy-url>".to_string());
        }

        let Some(authority) = rest
            .split(['/', '?', '#'])
            .next()
            .filter(|authority| !authority.is_empty())
        else {
            return Self("<invalid-proxy-url>".to_string());
        };
        let host_port = authority
            .rsplit_once('@')
            .map_or(authority, |(_, tail)| tail);
        let port = redacted_port_suffix(host_port).unwrap_or_default();
        Self(format!(
            "{}://<redacted-host>{port}",
            scheme.to_ascii_lowercase()
        ))
    }
}

fn redacted_port_suffix(host_port: &str) -> Option<String> {
    if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        let port = host_port[end + 1..].strip_prefix(':')?;
        return (!port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| format!(":{port}"));
    }

    let (host, port) = host_port.rsplit_once(':')?;
    if host.is_empty() || host.contains(':') || port.is_empty() {
        return None;
    }
    port.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| format!(":{port}"))
}

impl fmt::Debug for RedactedProxyEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for RedactedProxyEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteDecision {
    Direct,
    Proxy(RedactedProxyEndpoint),
    Unavailable(RouteFailureClass),
}

impl fmt::Display for RouteDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::Proxy(endpoint) => write!(f, "proxy({endpoint})"),
            Self::Unavailable(failure) => write!(f, "unavailable({failure})"),
        }
    }
}

/// One safe diagnostic event for a resolver/client decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteDiagnostic {
    route_class: ClientRouteClass,
    source: RouteDecisionSource,
    decision: RouteDecision,
}

impl RouteDiagnostic {
    pub(crate) fn direct(route_class: ClientRouteClass, source: RouteDecisionSource) -> Self {
        Self::new(route_class, source, RouteDecision::Direct)
    }

    pub(crate) fn proxy(
        route_class: ClientRouteClass,
        source: RouteDecisionSource,
        proxy_url: &str,
    ) -> Self {
        Self::new(
            route_class,
            source,
            RouteDecision::Proxy(RedactedProxyEndpoint::parse(proxy_url)),
        )
    }

    pub(crate) fn unavailable(
        route_class: ClientRouteClass,
        source: RouteDecisionSource,
        failure: RouteFailureClass,
    ) -> Self {
        Self::new(route_class, source, RouteDecision::Unavailable(failure))
    }

    fn new(
        route_class: ClientRouteClass,
        source: RouteDecisionSource,
        decision: RouteDecision,
    ) -> Self {
        Self {
            route_class,
            source,
            decision,
        }
    }

    /// Emits a sanitized structured event when diagnostics are explicitly enabled.
    pub(crate) fn emit_opt_in(&self) {
        if !network_diagnostics_enabled() {
            return;
        }
        let failure = match &self.decision {
            RouteDecision::Unavailable(failure) => failure.to_string(),
            RouteDecision::Direct | RouteDecision::Proxy(_) => "none".to_string(),
        };
        let custom_ca_configured =
            env_present("CODEX_CA_CERTIFICATE") || env_present("SSL_CERT_FILE");
        tracing::info!(
            route_class = %self.route_class,
            source = %self.source,
            decision = %self.decision,
            failure = %failure,
            custom_ca_configured = custom_ca_configured,
            "opt-in outbound route diagnostic"
        );
    }
}

#[cfg(test)]
#[path = "route_diagnostics_tests.rs"]
mod tests;
