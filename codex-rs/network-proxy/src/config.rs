use anyhow::Context;
use anyhow::Result;
use codex_core::config::default_config_path;
use serde::Deserialize;
use serde::Serialize;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub network_proxy: NetworkProxyConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network_proxy: NetworkProxyConfig::default(),
        }
    }
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
            mode: NetworkMode::default(),
            policy: NetworkPolicy::default(),
            mitm: MitmConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    #[serde(default, rename = "allowedDomains")]
    pub allowed_domains: Vec<String>,
    #[serde(default, rename = "deniedDomains")]
    pub denied_domains: Vec<String>,
    #[serde(default, rename = "allowUnixSockets")]
    pub allow_unix_sockets: Vec<String>,
    #[serde(default, rename = "allowLocalBinding")]
    pub allow_local_binding: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allowed_domains: Vec::new(),
            denied_domains: Vec::new(),
            allow_unix_sockets: Vec::new(),
            allow_local_binding: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    Limited,
    Full,
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Full
    }
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

pub struct RuntimeConfig {
    pub http_addr: SocketAddr,
    pub socks_addr: SocketAddr,
    pub admin_addr: SocketAddr,
}

pub fn default_codex_config_path() -> Result<PathBuf> {
    default_config_path().context("failed to resolve Codex config path")
}

pub fn resolve_runtime(cfg: &Config) -> RuntimeConfig {
    let http_addr = resolve_addr(&cfg.network_proxy.proxy_url, 3128);
    let admin_addr = resolve_addr(&cfg.network_proxy.admin_url, 8080);
    let socks_addr = SocketAddr::from(([127, 0, 0, 1], 8081));

    RuntimeConfig {
        http_addr,
        socks_addr,
        admin_addr,
    }
}

fn resolve_addr(url: &str, default_port: u16) -> SocketAddr {
    let (host, port) = parse_host_port(url, default_port);
    let host = if host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        host
    };
    match host.parse::<IpAddr>() {
        Ok(ip) => SocketAddr::new(ip, port),
        Err(_) => SocketAddr::from(([127, 0, 0, 1], port)),
    }
}

fn parse_host_port(url: &str, default_port: u16) -> (&str, u16) {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return ("127.0.0.1", default_port);
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

    if host_port.starts_with('[') {
        if let Some(end) = host_port.find(']') {
            let host = &host_port[1..end];
            let port = host_port[end + 1..]
                .strip_prefix(':')
                .and_then(|port| port.parse::<u16>().ok())
                .unwrap_or(default_port);
            return (host, port);
        }
    }

    if let Some((host, port)) = host_port.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return (host, port);
        }
    }

    (host_port, default_port)
}
