use serde::Deserialize;
use serde::Serialize;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub network_proxy: NetworkProxyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_proxy_url")]
    pub proxy_url: String,
    #[serde(default = "default_admin_url")]
    pub admin_url: String,
    #[serde(default)]
    pub dangerously_allow_non_loopback_proxy: bool,
    #[serde(default)]
    pub dangerously_allow_non_loopback_admin: bool,
    #[serde(default)]
    pub mode: NetworkMode,
    #[serde(default)]
    pub policy: NetworkPolicy,
    #[serde(default)]
    pub mitm: MitmConfig,
}

impl Default for NetworkProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_url: default_proxy_url(),
            admin_url: default_admin_url(),
            dangerously_allow_non_loopback_proxy: false,
            dangerously_allow_non_loopback_admin: false,
            mode: NetworkMode::default(),
            policy: NetworkPolicy::default(),
            mitm: MitmConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkPolicy {
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub denied_domains: Vec<String>,
    #[serde(default)]
    pub allow_unix_sockets: Vec<String>,
    #[serde(default)]
    pub allow_local_binding: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    Limited,
    #[default]
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub inspect: bool,
    #[serde(default = "default_mitm_max_body_bytes")]
    pub max_body_bytes: usize,
    #[serde(default = "default_ca_cert_path")]
    pub ca_cert_path: PathBuf,
    #[serde(default = "default_ca_key_path")]
    pub ca_key_path: PathBuf,
}

impl Default for MitmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            inspect: false,
            max_body_bytes: default_mitm_max_body_bytes(),
            ca_cert_path: default_ca_cert_path(),
            ca_key_path: default_ca_key_path(),
        }
    }
}

fn default_proxy_url() -> String {
    "http://127.0.0.1:3128".to_string()
}

fn default_admin_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_ca_cert_path() -> PathBuf {
    PathBuf::from("network_proxy/mitm/ca.pem")
}

fn default_ca_key_path() -> PathBuf {
    PathBuf::from("network_proxy/mitm/ca.key")
}

fn default_mitm_max_body_bytes() -> usize {
    4096
}

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

pub struct RuntimeConfig {
    pub http_addr: SocketAddr,
    pub socks_addr: SocketAddr,
    pub admin_addr: SocketAddr,
}

pub fn resolve_runtime(cfg: &Config) -> RuntimeConfig {
    let http_addr = resolve_addr(&cfg.network_proxy.proxy_url, 3128);
    let admin_addr = resolve_addr(&cfg.network_proxy.admin_url, 8080);
    let http_addr = clamp_non_loopback(
        http_addr,
        cfg.network_proxy.dangerously_allow_non_loopback_proxy,
        "HTTP proxy",
    );
    let admin_addr = clamp_non_loopback(
        admin_addr,
        cfg.network_proxy.dangerously_allow_non_loopback_admin,
        "admin API",
    );
    let (http_addr, admin_addr) = if cfg.network_proxy.policy.allow_unix_sockets.is_empty() {
        (http_addr, admin_addr)
    } else {
        // `x-unix-socket` is intentionally a local escape hatch. If the proxy (or admin API) is
        // reachable from outside the machine, it can become a remote bridge into local daemons
        // (e.g. docker.sock). To avoid footguns, enforce loopback binding whenever unix sockets
        // are enabled.
        if cfg.network_proxy.dangerously_allow_non_loopback_proxy && !http_addr.ip().is_loopback() {
            warn!(
                "unix socket proxying is enabled; ignoring dangerously_allow_non_loopback_proxy and clamping HTTP proxy to loopback"
            );
        }
        if cfg.network_proxy.dangerously_allow_non_loopback_admin && !admin_addr.ip().is_loopback()
        {
            warn!(
                "unix socket proxying is enabled; ignoring dangerously_allow_non_loopback_admin and clamping admin API to loopback"
            );
        }
        (
            SocketAddr::from(([127, 0, 0, 1], http_addr.port())),
            SocketAddr::from(([127, 0, 0, 1], admin_addr.port())),
        )
    };
    let socks_addr = SocketAddr::from(([127, 0, 0, 1], 8081));

    RuntimeConfig {
        http_addr,
        socks_addr,
        admin_addr,
    }
}

fn resolve_addr(url: &str, default_port: u16) -> SocketAddr {
    let addr_parts = parse_host_port(url, default_port);
    let host = if addr_parts.host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        addr_parts.host
    };
    match host.parse::<IpAddr>() {
        Ok(ip) => SocketAddr::new(ip, addr_parts.port),
        Err(_) => SocketAddr::from(([127, 0, 0, 1], addr_parts.port)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SocketAddressParts<'a> {
    host: &'a str,
    port: u16,
}

fn parse_host_port(url: &str, default_port: u16) -> SocketAddressParts<'_> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return SocketAddressParts {
            host: "127.0.0.1",
            port: default_port,
        };
    }
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
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
        return SocketAddressParts { host, port };
    }

    // Only treat `host:port` as such when there's a single `:`. This avoids
    // accidentally interpreting unbracketed IPv6 addresses as `host:port`.
    if host_port.bytes().filter(|b| *b == b':').count() == 1
        && let Some((host, port)) = host_port.rsplit_once(':')
        && let Ok(port) = port.parse::<u16>()
    {
        return SocketAddressParts { host, port };
    }

    SocketAddressParts {
        host: host_port,
        port: default_port,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn parse_host_port_defaults_for_empty_string() {
        assert_eq!(
            parse_host_port("", 1234),
            SocketAddressParts {
                host: "127.0.0.1",
                port: 1234,
            }
        );
    }

    #[test]
    fn parse_host_port_defaults_for_whitespace() {
        assert_eq!(
            parse_host_port("   ", 5555),
            SocketAddressParts {
                host: "127.0.0.1",
                port: 5555,
            }
        );
    }

    #[test]
    fn parse_host_port_parses_host_port_without_scheme() {
        assert_eq!(
            parse_host_port("127.0.0.1:8080", 3128),
            SocketAddressParts {
                host: "127.0.0.1",
                port: 8080,
            }
        );
    }

    #[test]
    fn parse_host_port_parses_host_port_with_scheme_and_path() {
        assert_eq!(
            parse_host_port("http://example.com:8080/some/path", 3128),
            SocketAddressParts {
                host: "example.com",
                port: 8080,
            }
        );
    }

    #[test]
    fn parse_host_port_strips_userinfo() {
        assert_eq!(
            parse_host_port("http://user:pass@host.example:5555", 3128),
            SocketAddressParts {
                host: "host.example",
                port: 5555,
            }
        );
    }

    #[test]
    fn parse_host_port_parses_ipv6_with_brackets() {
        assert_eq!(
            parse_host_port("http://[::1]:9999", 3128),
            SocketAddressParts {
                host: "::1",
                port: 9999,
            }
        );
    }

    #[test]
    fn parse_host_port_does_not_treat_unbracketed_ipv6_as_host_port() {
        assert_eq!(
            parse_host_port("2001:db8::1", 3128),
            SocketAddressParts {
                host: "2001:db8::1",
                port: 3128,
            }
        );
    }

    #[test]
    fn parse_host_port_falls_back_to_default_port_when_port_is_invalid() {
        assert_eq!(
            parse_host_port("example.com:notaport", 3128),
            SocketAddressParts {
                host: "example.com:notaport",
                port: 3128,
            }
        );
    }

    #[test]
    fn resolve_addr_maps_localhost_to_loopback() {
        assert_eq!(
            resolve_addr("localhost", 3128),
            "127.0.0.1:3128".parse().unwrap()
        );
    }

    #[test]
    fn resolve_addr_parses_ip_literals() {
        assert_eq!(resolve_addr("1.2.3.4", 80), "1.2.3.4:80".parse().unwrap());
    }

    #[test]
    fn resolve_addr_parses_ipv6_literals() {
        assert_eq!(
            resolve_addr("http://[::1]:8080", 3128),
            "[::1]:8080".parse().unwrap()
        );
    }

    #[test]
    fn resolve_addr_falls_back_to_loopback_for_hostnames() {
        assert_eq!(
            resolve_addr("http://example.com:5555", 3128),
            "127.0.0.1:5555".parse().unwrap()
        );
    }
}
