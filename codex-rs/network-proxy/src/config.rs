use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;
use std::net::IpAddr;
use std::net::SocketAddr;
use tracing::warn;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkProxyConfig {
    #[serde(default)]
    pub network: NetworkProxySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct NetworkProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_proxy_url")]
    pub proxy_url: String,
    #[serde(default = "default_admin_url")]
    pub admin_url: String,
    pub enable_socks5: bool,
    #[serde(default = "default_socks_url")]
    pub socks_url: String,
    pub enable_socks5_udp: bool,
    pub allow_upstream_proxy: bool,
    #[serde(default)]
    pub dangerously_allow_non_loopback_proxy: bool,
    #[serde(default)]
    pub dangerously_allow_non_loopback_admin: bool,
    #[serde(default)]
    pub mode: NetworkMode,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub denied_domains: Vec<String>,
    #[serde(default)]
    pub allow_unix_sockets: Vec<String>,
    pub allow_local_binding: bool,
}

impl Default for NetworkProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_url: default_proxy_url(),
            admin_url: default_admin_url(),
            enable_socks5: true,
            socks_url: default_socks_url(),
            enable_socks5_udp: true,
            allow_upstream_proxy: true,
            dangerously_allow_non_loopback_proxy: false,
            dangerously_allow_non_loopback_admin: false,
            mode: NetworkMode::default(),
            allowed_domains: Vec::new(),
            denied_domains: Vec::new(),
            allow_unix_sockets: Vec::new(),
            allow_local_binding: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    /// Limited (read-only) access: only GET/HEAD/OPTIONS are allowed for HTTP. HTTPS CONNECT is
    /// blocked unless MITM is enabled so the proxy can enforce method policy on inner requests.
    Limited,
    /// Full network access: all HTTP methods are allowed, and HTTPS CONNECTs are tunneled without
    /// MITM interception.
    #[default]
    Full,
}

impl NetworkMode {
    pub fn allows_method(self, method: &str) -> bool {
        match self {
            Self::Full => true,
            Self::Limited => matches!(method, "GET" | "HEAD" | "OPTIONS"),
        }
    }
}

fn default_proxy_url() -> String {
    "http://127.0.0.1:3128".to_string()
}

fn default_admin_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_socks_url() -> String {
    "http://127.0.0.1:8081".to_string()
}

/// Clamp non-loopback bind addresses to loopback unless explicitly allowed.
fn clamp_non_loopback(addr: SocketAddr, allow_non_loopback: bool, name: &str) -> SocketAddr {
    if addr.ip().is_loopback() {
        return addr;
    }

    if allow_non_loopback {
        warn!("DANGEROUS: {name} listening on non-loopback address {addr}");
        return addr;
    }

    warn!(
        "{name} requested non-loopback bind ({addr}); clamping to 127.0.0.1:{port} (set dangerously_allow_non_loopback_proxy or dangerously_allow_non_loopback_admin to override)",
        port = addr.port()
    );
    SocketAddr::from(([127, 0, 0, 1], addr.port()))
}

pub(crate) fn clamp_bind_addrs(
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    admin_addr: SocketAddr,
    cfg: &NetworkProxySettings,
) -> (SocketAddr, SocketAddr, SocketAddr) {
    let http_addr = clamp_non_loopback(
        http_addr,
        cfg.dangerously_allow_non_loopback_proxy,
        "HTTP proxy",
    );
    let socks_addr = clamp_non_loopback(
        socks_addr,
        cfg.dangerously_allow_non_loopback_proxy,
        "SOCKS5 proxy",
    );
    let admin_addr = clamp_non_loopback(
        admin_addr,
        cfg.dangerously_allow_non_loopback_admin,
        "admin API",
    );
    if cfg.allow_unix_sockets.is_empty() {
        return (http_addr, socks_addr, admin_addr);
    }

    // `x-unix-socket` is intentionally a local escape hatch. If the proxy (or admin API) is
    // reachable from outside the machine, it can become a remote bridge into local daemons
    // (e.g. docker.sock). To avoid footguns, enforce loopback binding whenever unix sockets
    // are enabled.
    if cfg.dangerously_allow_non_loopback_proxy && !http_addr.ip().is_loopback() {
        warn!(
            "unix socket proxying is enabled; ignoring dangerously_allow_non_loopback_proxy and clamping HTTP proxy to loopback"
        );
    }
    if cfg.dangerously_allow_non_loopback_proxy && !socks_addr.ip().is_loopback() {
        warn!(
            "unix socket proxying is enabled; ignoring dangerously_allow_non_loopback_proxy and clamping SOCKS5 proxy to loopback"
        );
    }
    if cfg.dangerously_allow_non_loopback_admin && !admin_addr.ip().is_loopback() {
        warn!(
            "unix socket proxying is enabled; ignoring dangerously_allow_non_loopback_admin and clamping admin API to loopback"
        );
    }
    (
        SocketAddr::from(([127, 0, 0, 1], http_addr.port())),
        SocketAddr::from(([127, 0, 0, 1], socks_addr.port())),
        SocketAddr::from(([127, 0, 0, 1], admin_addr.port())),
    )
}

pub struct RuntimeConfig {
    pub http_addr: SocketAddr,
    pub socks_addr: SocketAddr,
    pub admin_addr: SocketAddr,
}

pub fn resolve_runtime(cfg: &NetworkProxyConfig) -> Result<RuntimeConfig> {
    let http_addr = resolve_addr(&cfg.network.proxy_url, 3128)
        .with_context(|| format!("invalid network.proxy_url: {}", cfg.network.proxy_url))?;
    let socks_addr = resolve_addr(&cfg.network.socks_url, 8081)
        .with_context(|| format!("invalid network.socks_url: {}", cfg.network.socks_url))?;
    let admin_addr = resolve_addr(&cfg.network.admin_url, 8080)
        .with_context(|| format!("invalid network.admin_url: {}", cfg.network.admin_url))?;
    let (http_addr, socks_addr, admin_addr) =
        clamp_bind_addrs(http_addr, socks_addr, admin_addr, &cfg.network);

    Ok(RuntimeConfig {
        http_addr,
        socks_addr,
        admin_addr,
    })
}

fn resolve_addr(url: &str, default_port: u16) -> Result<SocketAddr> {
    let addr_parts = parse_host_port(url, default_port)?;
    let host = if addr_parts.host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1".to_string()
    } else {
        addr_parts.host
    };
    match host.parse::<IpAddr>() {
        Ok(ip) => Ok(SocketAddr::new(ip, addr_parts.port)),
        Err(_) => Ok(SocketAddr::from(([127, 0, 0, 1], addr_parts.port))),
    }
}

pub fn host_and_port_from_network_addr(value: &str, default_port: u16) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<missing>".to_string();
    }

    let parts = match parse_host_port(trimmed, default_port) {
        Ok(parts) => parts,
        Err(_) => {
            return format_host_and_port(trimmed, default_port);
        }
    };

    format_host_and_port(&parts.host, parts.port)
}

fn format_host_and_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SocketAddressParts {
    host: String,
    port: u16,
}

fn parse_host_port(url: &str, default_port: u16) -> Result<SocketAddressParts> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        bail!("missing host in network proxy address: {url}");
    }

    // Avoid treating unbracketed IPv6 literals like "2001:db8::1" as scheme-prefixed URLs.
    if matches!(trimmed.parse::<IpAddr>(), Ok(IpAddr::V6(_))) && !trimmed.starts_with('[') {
        return Ok(SocketAddressParts {
            host: trimmed.to_string(),
            port: default_port,
        });
    }

    // Prefer the standard URL parser when the input is URL-like. Prefix a scheme when absent so
    // we still accept loose host:port inputs.
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    if let Ok(parsed) = Url::parse(&candidate)
        && let Some(host) = parsed.host_str()
    {
        let host = host.trim_matches(|c| c == '[' || c == ']');
        if host.is_empty() {
            bail!("missing host in network proxy address: {url}");
        }
        return Ok(SocketAddressParts {
            host: host.to_string(),
            port: parsed.port().unwrap_or(default_port),
        });
    }

    parse_host_port_fallback(trimmed, default_port)
}

fn parse_host_port_fallback(input: &str, default_port: u16) -> Result<SocketAddressParts> {
    let without_scheme = input
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(input);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host_port = host_port
        .rsplit_once('@')
        .map(|(_, rest)| rest)
        .unwrap_or(host_port);

    if host_port.starts_with('[')
        && let Some(end) = host_port.find(']')
    {
        let host = &host_port[1..end];
        let port = host_port[end + 1..]
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
            .unwrap_or(default_port);
        if host.is_empty() {
            bail!("missing host in network proxy address: {input}");
        }
        return Ok(SocketAddressParts {
            host: host.to_string(),
            port,
        });
    }

    // Only treat `host:port` as such when there's a single `:`. This avoids
    // accidentally interpreting unbracketed IPv6 addresses as `host:port`.
    if host_port.bytes().filter(|b| *b == b':').count() == 1
        && let Some((host, port)) = host_port.rsplit_once(':')
    {
        if host.is_empty() {
            bail!("missing host in network proxy address: {input}");
        }
        return Ok(SocketAddressParts {
            host: host.to_string(),
            port: port.parse::<u16>().ok().unwrap_or(default_port),
        });
    }

    if host_port.is_empty() {
        bail!("missing host in network proxy address: {input}");
    }
    Ok(SocketAddressParts {
        host: host_port.to_string(),
        port: default_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn network_proxy_settings_default_matches_local_use_baseline() {
        assert_eq!(
            NetworkProxySettings::default(),
            NetworkProxySettings {
                enabled: false,
                proxy_url: "http://127.0.0.1:3128".to_string(),
                admin_url: "http://127.0.0.1:8080".to_string(),
                enable_socks5: true,
                socks_url: "http://127.0.0.1:8081".to_string(),
                enable_socks5_udp: true,
                allow_upstream_proxy: true,
                dangerously_allow_non_loopback_proxy: false,
                dangerously_allow_non_loopback_admin: false,
                mode: NetworkMode::Full,
                allowed_domains: Vec::new(),
                denied_domains: Vec::new(),
                allow_unix_sockets: Vec::new(),
                allow_local_binding: true,
            }
        );
    }

    #[test]
    fn partial_network_config_uses_struct_defaults_for_missing_fields() {
        let config: NetworkProxyConfig = serde_json::from_str(
            r#"{
                "network": {
                    "enabled": true
                }
            }"#,
        )
        .unwrap();
        let expected = NetworkProxySettings {
            enabled: true,
            ..NetworkProxySettings::default()
        };

        assert_eq!(config.network, expected);
    }

    #[test]
    fn parse_host_port_defaults_for_empty_string() {
        assert!(parse_host_port("", 1234).is_err());
    }

    #[test]
    fn parse_host_port_defaults_for_whitespace() {
        assert!(parse_host_port("   ", 5555).is_err());
    }

    #[test]
    fn parse_host_port_parses_host_port_without_scheme() {
        assert_eq!(
            parse_host_port("127.0.0.1:8080", 3128).unwrap(),
            SocketAddressParts {
                host: "127.0.0.1".to_string(),
                port: 8080,
            }
        );
    }

    #[test]
    fn parse_host_port_parses_host_port_with_scheme_and_path() {
        assert_eq!(
            parse_host_port("http://example.com:8080/some/path", 3128).unwrap(),
            SocketAddressParts {
                host: "example.com".to_string(),
                port: 8080,
            }
        );
    }

    #[test]
    fn parse_host_port_strips_userinfo() {
        assert_eq!(
            parse_host_port("http://user:pass@host.example:5555", 3128).unwrap(),
            SocketAddressParts {
                host: "host.example".to_string(),
                port: 5555,
            }
        );
    }

    #[test]
    fn parse_host_port_parses_ipv6_with_brackets() {
        assert_eq!(
            parse_host_port("http://[::1]:9999", 3128).unwrap(),
            SocketAddressParts {
                host: "::1".to_string(),
                port: 9999,
            }
        );
    }

    #[test]
    fn parse_host_port_does_not_treat_unbracketed_ipv6_as_host_port() {
        assert_eq!(
            parse_host_port("2001:db8::1", 3128).unwrap(),
            SocketAddressParts {
                host: "2001:db8::1".to_string(),
                port: 3128,
            }
        );
    }

    #[test]
    fn parse_host_port_falls_back_to_default_port_when_port_is_invalid() {
        assert_eq!(
            parse_host_port("example.com:notaport", 3128).unwrap(),
            SocketAddressParts {
                host: "example.com".to_string(),
                port: 3128,
            }
        );
    }

    #[test]
    fn host_and_port_from_network_addr_defaults_for_empty_string() {
        assert_eq!(host_and_port_from_network_addr("", 1234), "<missing>");
    }

    #[test]
    fn host_and_port_from_network_addr_formats_ipv6() {
        assert_eq!(
            host_and_port_from_network_addr("http://[::1]:8080", 3128),
            "[::1]:8080"
        );
    }

    #[test]
    fn resolve_addr_maps_localhost_to_loopback() {
        assert_eq!(
            resolve_addr("localhost", 3128).unwrap(),
            "127.0.0.1:3128".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn resolve_addr_parses_ip_literals() {
        assert_eq!(
            resolve_addr("1.2.3.4", 80).unwrap(),
            "1.2.3.4:80".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn resolve_addr_parses_ipv6_literals() {
        assert_eq!(
            resolve_addr("http://[::1]:8080", 3128).unwrap(),
            "[::1]:8080".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn resolve_addr_falls_back_to_loopback_for_hostnames() {
        assert_eq!(
            resolve_addr("http://example.com:5555", 3128).unwrap(),
            "127.0.0.1:5555".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn clamp_bind_addrs_allows_non_loopback_when_enabled() {
        let cfg = NetworkProxySettings {
            dangerously_allow_non_loopback_proxy: true,
            dangerously_allow_non_loopback_admin: true,
            ..Default::default()
        };
        let http_addr = "0.0.0.0:3128".parse::<SocketAddr>().unwrap();
        let socks_addr = "0.0.0.0:8081".parse::<SocketAddr>().unwrap();
        let admin_addr = "0.0.0.0:8080".parse::<SocketAddr>().unwrap();

        let (http_addr, socks_addr, admin_addr) =
            clamp_bind_addrs(http_addr, socks_addr, admin_addr, &cfg);

        assert_eq!(http_addr, "0.0.0.0:3128".parse::<SocketAddr>().unwrap());
        assert_eq!(socks_addr, "0.0.0.0:8081".parse::<SocketAddr>().unwrap());
        assert_eq!(admin_addr, "0.0.0.0:8080".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn clamp_bind_addrs_forces_loopback_when_unix_sockets_enabled() {
        let cfg = NetworkProxySettings {
            dangerously_allow_non_loopback_proxy: true,
            dangerously_allow_non_loopback_admin: true,
            allow_unix_sockets: vec!["/tmp/docker.sock".to_string()],
            ..Default::default()
        };
        let http_addr = "0.0.0.0:3128".parse::<SocketAddr>().unwrap();
        let socks_addr = "0.0.0.0:8081".parse::<SocketAddr>().unwrap();
        let admin_addr = "0.0.0.0:8080".parse::<SocketAddr>().unwrap();

        let (http_addr, socks_addr, admin_addr) =
            clamp_bind_addrs(http_addr, socks_addr, admin_addr, &cfg);

        assert_eq!(http_addr, "127.0.0.1:3128".parse::<SocketAddr>().unwrap());
        assert_eq!(socks_addr, "127.0.0.1:8081".parse::<SocketAddr>().unwrap());
        assert_eq!(admin_addr, "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
    }
}
