//! Redacted route diagnostics shared by resolver-aware HTTP clients.
//!
//! This module keeps route values side-effect free; explicitly opt-in helpers emit logs. It gives the upcoming system
//! proxy resolver a common vocabulary for "what route did we choose?" without
//! changing any client routing in this phase. Values stored here must be safe to
//! emit in structured logs: proxy credentials, PAC URLs, request URLs, and token
//! material are never retained.

use std::fmt;

/// Environment kill switch reserved for system proxy discovery.
///
/// Values such as `off`, `false`, `0`, `no`, or `disabled` disable system/PAC
/// discovery while still allowing explicit environment proxies to be honored by
/// future resolver-aware clients.
pub const CODEX_SYSTEM_PROXY_ENV: &str = "CODEX_SYSTEM_PROXY";

/// Opt-in switch for sanitized network diagnostics during auth flows.
///
/// Set to `1`, `true`, `on`, or `yes` to emit one-shot diagnostic events from
/// call sites that explicitly opt in. Values are never logged.
pub const CODEX_NETWORK_DIAGNOSTICS_ENV: &str = "CODEX_NETWORK_DIAGNOSTICS";

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .as_deref()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Returns whether opt-in network diagnostics are enabled for this process.
pub fn network_diagnostics_enabled() -> bool {
    env_flag_enabled(/* name */ CODEX_NETWORK_DIAGNOSTICS_ENV)
}

fn env_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn proxy_env_present(upper: &str, lower: &str) -> bool {
    env_present(upper) || env_present(lower)
}

/// Emit a sanitized auth-network environment snapshot when diagnostics are opt-in.
///
/// This intentionally records only presence bits and coarse override state, never
/// proxy values, CA paths, URLs, headers, or tokens.
pub fn emit_auth_network_environment_snapshot(operation: &'static str) {
    if !network_diagnostics_enabled() {
        return;
    }
    let system_override = SystemProxyEnvOverride::from_env();
    let system_proxy_state = match system_override {
        SystemProxyEnvOverride::Default => "default",
        SystemProxyEnvOverride::Disabled => "disabled",
    };
    tracing::info!(
        target_class = "auth",
        operation = operation,
        http_proxy_present =
            proxy_env_present(/* upper */ "HTTP_PROXY", /* lower */ "http_proxy"),
        https_proxy_present = proxy_env_present(
            /* upper */ "HTTPS_PROXY",
            /* lower */ "https_proxy"
        ),
        all_proxy_present =
            proxy_env_present(/* upper */ "ALL_PROXY", /* lower */ "all_proxy"),
        no_proxy_present =
            proxy_env_present(/* upper */ "NO_PROXY", /* lower */ "no_proxy"),
        codex_system_proxy = system_proxy_state,
        custom_ca_present = env_present(/* name */ "CODEX_CA_CERTIFICATE")
            || env_present(/* name */ "SSL_CERT_FILE"),
        "opt-in auth network diagnostic snapshot"
    );
}

fn classify_reqwest_error(error: &reqwest::Error) -> RouteFailureClass {
    if error.is_timeout() {
        return RouteFailureClass::ConnectTimeout;
    }
    if let Some(status) = error.status()
        && status.as_u16() == 407
    {
        return RouteFailureClass::ProxyAuthenticationRequired;
    }
    let rendered = error.to_string().to_ascii_lowercase();
    if rendered.contains("tls") || rendered.contains("certificate") || rendered.contains("cert") {
        return RouteFailureClass::TlsError;
    }
    if error.is_connect() {
        return RouteFailureClass::ResolverError;
    }
    RouteFailureClass::Other
}

/// Emit a sanitized auth transport failure classification when diagnostics are opt-in.
pub fn emit_auth_transport_failure(operation: &'static str, error: &reqwest::Error) {
    if !network_diagnostics_enabled() {
        return;
    }
    let failure = classify_reqwest_error(error);
    tracing::info!(
        target_class = "auth",
        operation = operation,
        failure = %failure,
        is_timeout = error.is_timeout(),
        is_connect = error.is_connect(),
        status_present = error.status().is_some(),
        status = error.status().map(|status| status.as_u16()).unwrap_or(0),
        "opt-in auth network transport diagnostic"
    );
}

/// Emit a sanitized auth HTTP status diagnostic when diagnostics are opt-in.
pub fn emit_auth_http_status(operation: &'static str, status: reqwest::StatusCode) {
    if !network_diagnostics_enabled() {
        return;
    }
    let failure = if status.as_u16() == 407 {
        RouteFailureClass::ProxyAuthenticationRequired
    } else {
        RouteFailureClass::Other
    };
    tracing::info!(
        target_class = "auth",
        operation = operation,
        status = status.as_u16(),
        failure = %failure,
        "opt-in auth network HTTP status diagnostic"
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemProxyEnvOverride {
    Default,
    Disabled,
}

impl SystemProxyEnvOverride {
    pub fn from_value(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::Default;
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" | "no" | "disabled" => Self::Disabled,
            _ => Self::Default,
        }
    }

    pub fn from_env() -> Self {
        Self::from_value(std::env::var(CODEX_SYSTEM_PROXY_ENV).ok().as_deref())
    }

    pub const fn system_discovery_enabled(self) -> bool {
        matches!(self, Self::Default)
    }
}

/// High-level client path being routed. Keep this coarse to avoid leaking URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    Auth,
    Api,
    WebSocket,
    Other,
}

impl fmt::Display for RouteTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auth => "auth",
            Self::Api => "api",
            Self::WebSocket => "wss",
            Self::Other => "other",
        })
    }
}

/// Source that produced a route decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource {
    ConfigOverride,
    Env,
    System,
    Direct,
    Disabled,
    Unavailable,
}

impl fmt::Display for RouteSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ConfigOverride => "config_override",
            Self::Env => "env",
            Self::System => "system",
            Self::Direct => "direct",
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
        })
    }
}

/// Coarse failure class suitable for logs and support bundles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteFailureClass {
    PacUnavailable,
    ConnectTimeout,
    ProxyAuthenticationRequired,
    TlsError,
    InvalidProxyConfig,
    UnsupportedProxyScheme,
    ResolverError,
    Other,
}

impl fmt::Display for RouteFailureClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PacUnavailable => "pac_unavailable",
            Self::ConnectTimeout => "connect_timeout",
            Self::ProxyAuthenticationRequired => "proxy_407",
            Self::TlsError => "tls_error",
            Self::InvalidProxyConfig => "invalid_proxy_config",
            Self::UnsupportedProxyScheme => "unsupported_proxy_scheme",
            Self::ResolverError => "resolver_error",
            Self::Other => "other",
        })
    }
}

/// A proxy endpoint rendered without credentials, hostnames, paths, or query strings.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedProxyEndpoint(String);

impl RedactedProxyEndpoint {
    pub fn parse(input: &str) -> Self {
        // Avoid a URL parser dependency here: diagnostics must never echo input,
        // so a conservative scheme/authority splitter is sufficient. Anything
        // outside the common absolute-URL shape is rendered as invalid.
        let Some((scheme, rest)) = input.split_once("://") else {
            return Self("<invalid-proxy-url>".to_string());
        };
        if scheme.is_empty()
            || !scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        {
            return Self("<invalid-proxy-url>".to_string());
        }

        let authority = rest
            .split(['/', '?', '#'])
            .next()
            .filter(|authority| !authority.is_empty());
        let Some(authority) = authority else {
            return Self("<invalid-proxy-url>".to_string());
        };
        // Drop credentials if present. We only inspect the post-@ authority for
        // a numeric port; the host itself is never copied into the output.
        let hostport = authority
            .rsplit_once('@')
            .map_or(authority, |(_, tail)| tail);
        let port = redacted_port_suffix(hostport).unwrap_or_default();
        let scheme = scheme.to_ascii_lowercase();
        Self(format!("{scheme}://<redacted-host>{port}"))
    }

    pub fn redacted(&self) -> &str {
        &self.0
    }
}

fn redacted_port_suffix(hostport: &str) -> Option<String> {
    if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        let suffix = &hostport[end + 1..];
        if let Some(port) = suffix.strip_prefix(':')
            && !port.is_empty()
            && port.bytes().all(|b| b.is_ascii_digit())
        {
            return Some(format!(":{port}"));
        }
        return None;
    }

    let (host, port) = hostport.rsplit_once(':')?;
    // Treat unbracketed IPv6 or empty host/port as no parseable port.
    if host.is_empty() || host.contains(':') || port.is_empty() {
        return None;
    }
    if port.bytes().all(|b| b.is_ascii_digit()) {
        Some(format!(":{port}"))
    } else {
        None
    }
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

/// Redacted route decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    Direct,
    Proxy(RedactedProxyEndpoint),
    Unavailable(RouteFailureClass),
}

impl fmt::Display for RouteDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::Proxy(endpoint) => write!(f, "proxy({endpoint})"),
            Self::Unavailable(reason) => write!(f, "unavailable({reason})"),
        }
    }
}

/// One safe diagnostic event for a resolver/client decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDiagnostic {
    pub target: RouteTarget,
    pub source: RouteSource,
    pub decision: RouteDecision,
    pub failure: Option<RouteFailureClass>,
    pub custom_ca_configured: bool,
}

impl RouteDiagnostic {
    pub const fn direct(
        target: RouteTarget,
        source: RouteSource,
        custom_ca_configured: bool,
    ) -> Self {
        Self {
            target,
            source,
            decision: RouteDecision::Direct,
            failure: None,
            custom_ca_configured,
        }
    }

    pub fn proxy(
        target: RouteTarget,
        source: RouteSource,
        proxy_url: &str,
        custom_ca_configured: bool,
    ) -> Self {
        Self {
            target,
            source,
            decision: RouteDecision::Proxy(RedactedProxyEndpoint::parse(proxy_url)),
            failure: None,
            custom_ca_configured,
        }
    }

    pub const fn unavailable(
        target: RouteTarget,
        source: RouteSource,
        failure: RouteFailureClass,
        custom_ca_configured: bool,
    ) -> Self {
        Self {
            target,
            source,
            decision: RouteDecision::Unavailable(failure),
            failure: Some(failure),
            custom_ca_configured,
        }
    }

    /// Emit a redacted structured debug event. Callers should add request IDs in
    /// their own span rather than passing URLs or tokens here.
    pub fn emit_debug(&self) {
        let failure = self
            .failure
            .map(|failure| failure.to_string())
            .unwrap_or_else(|| "none".to_string());
        tracing::debug!(
            route_target = %self.target,
            source = %self.source,
            decision = %self.decision,
            failure = %failure,
            custom_ca_configured = self.custom_ca_configured,
            "outbound route diagnostic"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_proxy_credentials_host_path_and_query() {
        let endpoint = RedactedProxyEndpoint::parse(
            "http://user:secret@proxy.internal.example:8080/pac?token=secret",
        );
        assert_eq!(endpoint.redacted(), "http://<redacted-host>:8080");
        assert!(!format!("{endpoint:?}").contains("secret"));
        assert!(!format!("{endpoint}").contains("proxy.internal"));
    }

    #[test]
    fn invalid_proxy_url_is_not_echoed() {
        let endpoint = RedactedProxyEndpoint::parse("not a url with password=secret");
        assert_eq!(endpoint.redacted(), "<invalid-proxy-url>");
    }

    #[test]
    fn system_proxy_env_override_accepts_disable_spellings() {
        for value in ["off", " OFF ", "false", "0", "no", "disabled"] {
            assert_eq!(
                SystemProxyEnvOverride::from_value(/* value */ Some(value)),
                SystemProxyEnvOverride::Disabled
            );
        }
        assert_eq!(
            SystemProxyEnvOverride::from_value(/* value */ None),
            SystemProxyEnvOverride::Default
        );
        assert_eq!(
            SystemProxyEnvOverride::from_value(/* value */ Some("auto")),
            SystemProxyEnvOverride::Default
        );
    }
}
