mod execution_scope;

use crate::attribution::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use crate::config;
use crate::credential_broker::BROKERED_CREDENTIALS_ENV_KEY;
use crate::credential_broker::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use crate::http_proxy;
use crate::network_policy::NetworkPolicyDecider;
use crate::runtime::BlockedRequestObserver;
use crate::runtime::ConfigState;
use crate::runtime::unix_socket_permissions_supported;
use crate::socks5;
use crate::state::NetworkProxyState;
#[cfg(target_os = "windows")]
use crate::windows_proxy_ingress::WindowsProxyIngress;
#[cfg(target_os = "windows")]
use crate::windows_proxy_ingress::WindowsProxyRoute;
#[cfg(target_os = "windows")]
use crate::windows_proxy_ingress::WindowsRouteService;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::warn;

use self::execution_scope::ExecutionScope;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {}

#[derive(Debug)]
struct ReservedListeners {
    http: Mutex<Option<StdTcpListener>>,
    socks: Mutex<Option<StdTcpListener>>,
}

impl ReservedListeners {
    #[cfg(not(target_os = "windows"))]
    fn new(http: StdTcpListener, socks: Option<StdTcpListener>) -> Self {
        Self {
            http: Mutex::new(Some(http)),
            socks: Mutex::new(socks),
        }
    }

    fn take_http(&self) -> Option<StdTcpListener> {
        let mut guard = self
            .http
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take()
    }

    fn take_socks(&self) -> Option<StdTcpListener> {
        let mut guard = self
            .socks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take()
    }
}

pub(super) struct ReservedListenerSet {
    http_listener: StdTcpListener,
    socks_listener: Option<StdTcpListener>,
}

impl ReservedListenerSet {
    fn new(http_listener: StdTcpListener, socks_listener: Option<StdTcpListener>) -> Self {
        Self {
            http_listener,
            socks_listener,
        }
    }

    pub(super) fn http_addr(&self) -> Result<SocketAddr> {
        self.http_listener
            .local_addr()
            .context("failed to read reserved HTTP proxy address")
    }

    pub(super) fn socks_addr(&self, default_addr: SocketAddr) -> Result<SocketAddr> {
        self.socks_listener
            .as_ref()
            .map_or(Ok(default_addr), |listener| {
                listener
                    .local_addr()
                    .context("failed to read reserved SOCKS5 proxy address")
            })
    }

    #[cfg(not(target_os = "windows"))]
    fn into_reserved_listeners(self) -> Arc<ReservedListeners> {
        Arc::new(ReservedListeners::new(
            self.http_listener,
            self.socks_listener,
        ))
    }

    #[cfg(target_os = "windows")]
    pub(super) fn into_listeners(self) -> (StdTcpListener, Option<StdTcpListener>) {
        (self.http_listener, self.socks_listener)
    }
}

#[derive(Clone)]
pub struct NetworkProxyBuilder {
    state: Option<Arc<NetworkProxyState>>,
    http_addr: Option<SocketAddr>,
    socks_addr: Option<SocketAddr>,
    managed_by_codex: bool,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    blocked_request_observer: Option<Arc<dyn BlockedRequestObserver>>,
}

impl Default for NetworkProxyBuilder {
    fn default() -> Self {
        Self {
            state: None,
            http_addr: None,
            socks_addr: None,
            managed_by_codex: true,
            policy_decider: None,
            blocked_request_observer: None,
        }
    }
}

impl NetworkProxyBuilder {
    pub fn state(mut self, state: Arc<NetworkProxyState>) -> Self {
        self.state = Some(state);
        self
    }

    pub fn http_addr(mut self, addr: SocketAddr) -> Self {
        self.http_addr = Some(addr);
        self
    }

    pub fn socks_addr(mut self, addr: SocketAddr) -> Self {
        self.socks_addr = Some(addr);
        self
    }

    pub fn managed_by_codex(mut self, managed_by_codex: bool) -> Self {
        self.managed_by_codex = managed_by_codex;
        self
    }

    pub fn policy_decider<D>(mut self, decider: D) -> Self
    where
        D: NetworkPolicyDecider,
    {
        self.policy_decider = Some(Arc::new(decider));
        self
    }

    pub fn policy_decider_arc(mut self, decider: Arc<dyn NetworkPolicyDecider>) -> Self {
        self.policy_decider = Some(decider);
        self
    }

    pub fn blocked_request_observer<O>(mut self, observer: O) -> Self
    where
        O: BlockedRequestObserver,
    {
        self.blocked_request_observer = Some(Arc::new(observer));
        self
    }

    pub fn blocked_request_observer_arc(
        mut self,
        observer: Arc<dyn BlockedRequestObserver>,
    ) -> Self {
        self.blocked_request_observer = Some(observer);
        self
    }

    pub async fn build(self) -> Result<NetworkProxy> {
        let state = self.state.ok_or_else(|| {
            anyhow::anyhow!(
                "NetworkProxyBuilder requires a state; supply one via builder.state(...)"
            )
        })?;
        state
            .set_blocked_request_observer(self.blocked_request_observer.clone())
            .await;
        let current_cfg = state.current_cfg().await?;
        #[cfg(target_os = "windows")]
        let runtime_settings = NetworkProxyRuntimeSettings::from_config(&current_cfg)?;
        #[cfg(target_os = "windows")]
        let mut windows_ingress = None;
        let (requested_http_addr, requested_socks_addr, reserved_listeners) = if self
            .managed_by_codex
        {
            let runtime = config::resolve_runtime(&current_cfg)?;
            #[cfg(target_os = "windows")]
            {
                let (managed_http_addr, managed_socks_addr) =
                    config::clamp_bind_addrs(runtime.http_addr, runtime.socks_addr, &current_cfg);
                let ingress = WindowsProxyIngress::shared(
                    managed_http_addr,
                    managed_socks_addr,
                    current_cfg.enable_socks5,
                )?;
                let http_addr = ingress.http_addr();
                let socks_addr = ingress.socks_addr();
                windows_ingress = Some(ingress);
                (http_addr, socks_addr, None)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let reserved = reserve_loopback_ephemeral_listeners(current_cfg.enable_socks5)
                    .context("reserve managed loopback proxy listeners")?;
                let http_addr = reserved.http_addr()?;
                let socks_addr = reserved.socks_addr(runtime.socks_addr)?;
                (
                    http_addr,
                    socks_addr,
                    Some(reserved.into_reserved_listeners()),
                )
            }
        } else {
            let runtime = config::resolve_runtime(&current_cfg)?;
            (
                self.http_addr.unwrap_or(runtime.http_addr),
                self.socks_addr.unwrap_or(runtime.socks_addr),
                None,
            )
        };

        // Reapply bind clamping for caller overrides so unix-socket proxying stays loopback-only.
        let (http_addr, socks_addr) =
            config::clamp_bind_addrs(requested_http_addr, requested_socks_addr, &current_cfg);

        #[cfg(target_os = "windows")]
        let windows_runtime = windows_ingress.map(|ingress| {
            let http = http_proxy::http_proxy_service(
                Arc::clone(&state),
                self.policy_decider.clone(),
                /*environment_id*/ None,
            );
            let socks = current_cfg.enable_socks5.then(|| {
                socks5::socks5_proxy_service(
                    Arc::clone(&state),
                    self.policy_decider.clone(),
                    /*environment_id*/ None,
                    current_cfg.enable_socks5_udp,
                )
            });
            Arc::new(WindowsSharedProxyRuntime {
                ingress,
                http_service: http,
                socks_service: socks,
                active_route: Arc::new(Mutex::new(None)),
            })
        });

        #[cfg(not(target_os = "windows"))]
        let runtime_settings = NetworkProxyRuntimeSettings::from_config(&current_cfg)?;

        Ok(NetworkProxy {
            state,
            http_addr,
            socks_addr,
            socks_enabled: current_cfg.enable_socks5,
            socks5_udp_enabled: current_cfg.enable_socks5_udp,
            runtime_settings: Arc::new(RwLock::new(runtime_settings)),
            reserved_listeners,
            policy_decider: self.policy_decider,
            environment_proxies: Arc::new(Mutex::new(HashMap::new())),
            execution_scope: None,
            #[cfg(target_os = "windows")]
            windows_runtime,
        })
    }
}

fn reserve_loopback_ephemeral_listeners(
    reserve_socks_listener: bool,
) -> Result<ReservedListenerSet> {
    let http_listener =
        reserve_loopback_ephemeral_listener().context("reserve HTTP proxy listener")?;
    let socks_listener = if reserve_socks_listener {
        Some(reserve_loopback_ephemeral_listener().context("reserve SOCKS5 proxy listener")?)
    } else {
        None
    };
    Ok(ReservedListenerSet::new(http_listener, socks_listener))
}

#[cfg(target_os = "windows")]
pub(super) fn reserve_windows_managed_listeners(
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    reserve_socks_listener: bool,
) -> Result<ReservedListenerSet> {
    let http_addr = windows_managed_loopback_addr(http_addr);
    let socks_addr = windows_managed_loopback_addr(socks_addr);

    match try_reserve_windows_managed_listeners(http_addr, socks_addr, reserve_socks_listener) {
        Ok(listeners) => Ok(listeners),
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            warn!("managed Windows proxy ports are busy; falling back to ephemeral loopback ports");
            reserve_loopback_ephemeral_listeners(reserve_socks_listener)
                .context("reserve fallback loopback proxy listeners")
        }
        Err(err) => Err(err).context("reserve Windows managed proxy listeners"),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn reserve_windows_managed_socks_listener(
    socks_addr: SocketAddr,
) -> Result<StdTcpListener> {
    let socks_addr = windows_managed_loopback_addr(socks_addr);
    match StdTcpListener::bind(socks_addr) {
        Ok(listener) => Ok(listener),
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            warn!(
                "managed Windows SOCKS5 proxy port is busy; falling back to an ephemeral loopback port"
            );
            reserve_loopback_ephemeral_listener()
                .context("reserve fallback loopback SOCKS5 proxy listener")
        }
        Err(err) => Err(err).context("reserve Windows managed SOCKS5 proxy listener"),
    }
}

#[cfg(target_os = "windows")]
fn try_reserve_windows_managed_listeners(
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    reserve_socks_listener: bool,
) -> std::io::Result<ReservedListenerSet> {
    let http_listener = StdTcpListener::bind(http_addr)?;
    let socks_listener = if reserve_socks_listener {
        Some(StdTcpListener::bind(socks_addr)?)
    } else {
        None
    };
    Ok(ReservedListenerSet::new(http_listener, socks_listener))
}

#[cfg(target_os = "windows")]
pub(super) fn windows_managed_loopback_addr(addr: SocketAddr) -> SocketAddr {
    if !addr.ip().is_loopback() {
        warn!(
            "managed Windows proxies must bind to loopback; clamping {addr} to 127.0.0.1:{}",
            addr.port()
        );
    }
    SocketAddr::from(([127, 0, 0, 1], addr.port()))
}

fn reserve_loopback_ephemeral_listener() -> Result<StdTcpListener> {
    StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .context("bind loopback ephemeral port")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkProxyRuntimeSettings {
    allow_local_binding: bool,
    allow_unix_sockets: Arc<[String]>,
    dangerously_allow_all_unix_sockets: bool,
    mitm_ca_trust_bundle: Option<crate::certs::ManagedMitmCaTrustBundle>,
}

impl NetworkProxyRuntimeSettings {
    fn from_config(config: &config::NetworkProxyConfig) -> Result<Self> {
        let mitm_ca_trust_bundle = if config.mitm {
            let env = crate::certs::ca_env_from_process();
            Some(crate::certs::managed_ca_trust_bundle(&env)?)
        } else {
            None
        };
        Ok(Self {
            allow_local_binding: config.allow_local_binding,
            allow_unix_sockets: config.allow_unix_sockets().into(),
            dangerously_allow_all_unix_sockets: config.dangerously_allow_all_unix_sockets,
            mitm_ca_trust_bundle,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EnvironmentProxyAddrs {
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
}

/// Portable managed-network facts needed by an operating-system sandbox.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedNetworkSandboxContext {
    /// Loopback proxy ports that sandboxed commands may connect to.
    #[serde(default)]
    pub loopback_ports: Vec<u16>,
    /// Whether the command may bind local sockets and exchange loopback traffic.
    #[serde(default)]
    pub allow_local_binding: bool,
}

/// Environment-specific managed-network settings prepared for one command launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedManagedNetwork {
    /// Complete command environment with managed proxy variables applied.
    pub env: HashMap<String, String>,
    /// Matching portable sandbox inputs for the command environment.
    pub sandbox_context: ManagedNetworkSandboxContext,
}

struct EnvironmentProxy {
    addrs: EnvironmentProxyAddrs,
    runtime: EnvironmentProxyRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvironmentProxyClient {
    SandboxedProcess,
    TrustedBridge,
}

enum EnvironmentProxyRuntime {
    ListenerTasks {
        http_task: JoinHandle<Result<()>>,
        socks_task: Option<JoinHandle<Result<()>>>,
    },
    #[cfg(target_os = "windows")]
    SharedIngress { _route: Arc<WindowsProxyRoute> },
}

impl EnvironmentProxyRuntime {
    #[cfg(target_os = "windows")]
    fn network_proxy_restricting_sid(&self) -> Option<String> {
        match self {
            Self::ListenerTasks { .. } => None,
            Self::SharedIngress { _route: route } => Some(route.sid().to_string()),
        }
    }
}

#[cfg(target_os = "windows")]
struct WindowsSharedProxyRuntime {
    ingress: Arc<WindowsProxyIngress>,
    http_service: WindowsRouteService,
    socks_service: Option<WindowsRouteService>,
    active_route: Arc<Mutex<Option<Arc<WindowsProxyRoute>>>>,
}

#[derive(Clone)]
pub struct NetworkProxy {
    state: Arc<NetworkProxyState>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    socks_enabled: bool,
    socks5_udp_enabled: bool,
    runtime_settings: Arc<RwLock<NetworkProxyRuntimeSettings>>,
    reserved_listeners: Option<Arc<ReservedListeners>>,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    environment_proxies: Arc<Mutex<HashMap<String, EnvironmentProxy>>>,
    execution_scope: Option<Arc<ExecutionScope>>,
    #[cfg(target_os = "windows")]
    windows_runtime: Option<Arc<WindowsSharedProxyRuntime>>,
}

impl std::fmt::Debug for NetworkProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Avoid logging internal state (config contents, derived globsets, etc.) which can be noisy
        // and may contain sensitive paths.
        f.debug_struct("NetworkProxy")
            .field("http_addr", &self.http_addr)
            .field("socks_addr", &self.socks_addr())
            .finish_non_exhaustive()
    }
}

impl PartialEq for NetworkProxy {
    fn eq(&self, other: &Self) -> bool {
        self.http_addr == other.http_addr
            && self.socks_addr() == other.socks_addr()
            && self.runtime_settings() == other.runtime_settings()
    }
}

impl Eq for NetworkProxy {}

pub const PROXY_URL_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "WS_PROXY",
    "WSS_PROXY",
    "ALL_PROXY",
    "FTP_PROXY",
    "YARN_HTTP_PROXY",
    "YARN_HTTPS_PROXY",
    "NPM_CONFIG_HTTP_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_PROXY",
    "BUNDLE_HTTP_PROXY",
    "BUNDLE_HTTPS_PROXY",
    "PIP_PROXY",
    "DOCKER_HTTP_PROXY",
    "DOCKER_HTTPS_PROXY",
];

pub const ALL_PROXY_ENV_KEYS: &[&str] = &["ALL_PROXY", "all_proxy"];
pub const PROXY_ACTIVE_ENV_KEY: &str = "CODEX_NETWORK_PROXY_ACTIVE";
pub const ALLOW_LOCAL_BINDING_ENV_KEY: &str = "CODEX_NETWORK_ALLOW_LOCAL_BINDING";
// Internal wire format shared with windows-sandbox-rs/src/setup.rs. The value is a
// comma-separated, sorted list of non-zero loopback proxy ports used only when computing the
// Windows offline sandbox setup marker.
#[cfg(target_os = "windows")]
const WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY: &str = "CODEX_WINDOWS_SANDBOX_PROXY_PORTS";
const ELECTRON_GET_USE_PROXY_ENV_KEY: &str = "ELECTRON_GET_USE_PROXY";
const NODE_USE_ENV_PROXY_ENV_KEY: &str = "NODE_USE_ENV_PROXY";
#[cfg(any(target_os = "macos", test))]
const GIT_SSH_COMMAND_ENV_KEY: &str = "GIT_SSH_COMMAND";
pub const PROXY_ENV_KEYS: &[&str] = &[
    PROXY_ACTIVE_ENV_KEY,
    CREDENTIAL_BROKER_ACTIVE_ENV_KEY,
    BROKERED_CREDENTIALS_ENV_KEY,
    ALLOW_LOCAL_BINDING_ENV_KEY,
    #[cfg(target_os = "windows")]
    WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY,
    PROXY_ATTRIBUTION_TOKEN_ENV_KEY,
    ELECTRON_GET_USE_PROXY_ENV_KEY,
    NODE_USE_ENV_PROXY_ENV_KEY,
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "http_proxy",
    "https_proxy",
    "YARN_HTTP_PROXY",
    "YARN_HTTPS_PROXY",
    "npm_config_http_proxy",
    "npm_config_https_proxy",
    "npm_config_proxy",
    "NPM_CONFIG_HTTP_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_PROXY",
    "BUNDLE_HTTP_PROXY",
    "BUNDLE_HTTPS_PROXY",
    "PIP_PROXY",
    "DOCKER_HTTP_PROXY",
    "DOCKER_HTTPS_PROXY",
    "WS_PROXY",
    "WSS_PROXY",
    "ws_proxy",
    "wss_proxy",
    "NO_PROXY",
    "no_proxy",
    "npm_config_noproxy",
    "NPM_CONFIG_NOPROXY",
    "YARN_NO_PROXY",
    "BUNDLE_NO_PROXY",
    "ALL_PROXY",
    "all_proxy",
    "FTP_PROXY",
    "ftp_proxy",
];

pub fn is_managed_proxy_env_var(key: &str, value: &str) -> bool {
    if PROXY_ENV_KEYS.contains(&key) {
        return true;
    }
    if crate::certs::CUSTOM_CA_ENV_KEYS.contains(&key) {
        return crate::certs::is_managed_mitm_ca_trust_bundle_path(value);
    }
    #[cfg(target_os = "macos")]
    {
        key == PROXY_GIT_SSH_COMMAND_ENV_KEY
            && value.starts_with(CODEX_PROXY_GIT_SSH_COMMAND_MARKER)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn strip_managed_proxy_env(env: &mut HashMap<String, String>) {
    env.retain(|key, value| !is_managed_proxy_env_var(key, value));
}

#[cfg(target_os = "macos")]
pub const PROXY_GIT_SSH_COMMAND_ENV_KEY: &str = GIT_SSH_COMMAND_ENV_KEY;

const FTP_PROXY_ENV_KEYS: &[&str] = &["FTP_PROXY", "ftp_proxy"];
const WEBSOCKET_PROXY_ENV_KEYS: &[&str] = &["WS_PROXY", "WSS_PROXY", "ws_proxy", "wss_proxy"];

pub const NO_PROXY_ENV_KEYS: &[&str] = &[
    "NO_PROXY",
    "no_proxy",
    "npm_config_noproxy",
    "NPM_CONFIG_NOPROXY",
    "YARN_NO_PROXY",
    "BUNDLE_NO_PROXY",
];

pub const DEFAULT_NO_PROXY_VALUE: &str = concat!(
    "localhost,127.0.0.1,::1,",
    "10.0.0.0/8,",
    "172.16.0.0/12,",
    "192.168.0.0/16"
);

#[cfg(target_os = "macos")]
pub const CODEX_PROXY_GIT_SSH_COMMAND_MARKER: &str = "CODEX_PROXY_GIT_SSH_COMMAND=1 ";
#[cfg(target_os = "macos")]
const CODEX_PROXY_GIT_SSH_COMMAND_PREFIX: &str =
    "CODEX_PROXY_GIT_SSH_COMMAND=1 ssh -o ProxyCommand='nc -X 5 -x ";
#[cfg(target_os = "macos")]
const CODEX_PROXY_GIT_SSH_COMMAND_SUFFIX: &str = " %h %p'";

pub fn proxy_url_env_value<'a>(
    env: &'a HashMap<String, String>,
    canonical_key: &str,
) -> Option<&'a str> {
    if let Some(value) = env.get(canonical_key) {
        return Some(value.as_str());
    }
    let lower_key = canonical_key.to_ascii_lowercase();
    env.get(lower_key.as_str()).map(String::as_str)
}

pub fn has_proxy_url_env_vars(env: &HashMap<String, String>) -> bool {
    PROXY_URL_ENV_KEYS
        .iter()
        .any(|key| proxy_url_env_value(env, key).is_some_and(|value| !value.trim().is_empty()))
}

fn set_env_keys(env: &mut HashMap<String, String>, keys: &[&str], value: &str) {
    for key in keys {
        env.insert((*key).to_string(), value.to_string());
    }
}

#[cfg(target_os = "macos")]
fn codex_proxy_git_ssh_command(socks_addr: SocketAddr) -> String {
    format!("{CODEX_PROXY_GIT_SSH_COMMAND_PREFIX}{socks_addr}{CODEX_PROXY_GIT_SSH_COMMAND_SUFFIX}")
}

#[cfg(target_os = "macos")]
fn is_codex_proxy_git_ssh_command(command: &str) -> bool {
    command.starts_with(CODEX_PROXY_GIT_SSH_COMMAND_PREFIX)
        && command.ends_with(CODEX_PROXY_GIT_SSH_COMMAND_SUFFIX)
}

fn apply_proxy_env_overrides(
    env: &mut HashMap<String, String>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    socks_enabled: bool,
    allow_local_binding: bool,
    mitm_ca_trust_bundle: Option<&crate::certs::ManagedMitmCaTrustBundle>,
) {
    let http_proxy_url = format!("http://{http_addr}");
    let socks_proxy_url = format!("socks5h://{socks_addr}");
    env.insert(PROXY_ACTIVE_ENV_KEY.to_string(), "1".to_string());
    env.insert(
        ALLOW_LOCAL_BINDING_ENV_KEY.to_string(),
        if allow_local_binding {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );

    // HTTP-based clients are best served by explicit HTTP proxy URLs.
    set_env_keys(
        env,
        &[
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "http_proxy",
            "https_proxy",
            "YARN_HTTP_PROXY",
            "YARN_HTTPS_PROXY",
            "npm_config_http_proxy",
            "npm_config_https_proxy",
            "npm_config_proxy",
            "NPM_CONFIG_HTTP_PROXY",
            "NPM_CONFIG_HTTPS_PROXY",
            "NPM_CONFIG_PROXY",
            "BUNDLE_HTTP_PROXY",
            "BUNDLE_HTTPS_PROXY",
            "PIP_PROXY",
            "DOCKER_HTTP_PROXY",
            "DOCKER_HTTPS_PROXY",
        ],
        &http_proxy_url,
    );
    // Some websocket clients look for dedicated WS/WSS proxy environment variables instead of
    // HTTP(S)_PROXY. Keep them aligned with the managed HTTP proxy endpoint.
    set_env_keys(env, WEBSOCKET_PROXY_ENV_KEYS, &http_proxy_url);

    // Keep local targets direct only when local binding is enabled. Otherwise route them through
    // the proxy so explicit literal allowlists and local-network restrictions can be enforced.
    let no_proxy = if allow_local_binding {
        DEFAULT_NO_PROXY_VALUE
    } else {
        ""
    };
    set_env_keys(env, NO_PROXY_ENV_KEYS, no_proxy);

    env.insert(
        ELECTRON_GET_USE_PROXY_ENV_KEY.to_string(),
        "true".to_string(),
    );
    // Node.js built-in HTTP clients only honor proxy environment variables when this is enabled.
    env.insert(NODE_USE_ENV_PROXY_ENV_KEY.to_string(), "1".to_string());

    // Keep HTTP_PROXY/HTTPS_PROXY as HTTP endpoints. A lot of clients break if
    // those vars contain SOCKS URLs. We only switch ALL_PROXY here.
    //
    if socks_enabled {
        set_env_keys(env, ALL_PROXY_ENV_KEYS, &socks_proxy_url);
        set_env_keys(env, FTP_PROXY_ENV_KEYS, &socks_proxy_url);
    } else {
        set_env_keys(env, ALL_PROXY_ENV_KEYS, &http_proxy_url);
        set_env_keys(env, FTP_PROXY_ENV_KEYS, &http_proxy_url);
    }

    #[cfg(target_os = "macos")]
    if socks_enabled {
        // Preserve existing SSH wrappers (for example: Secretive/Teleport setups)
        // but refresh a previously injected Codex fallback so it cannot point
        // at a stale proxy port after the proxy is restarted.
        match env.get(GIT_SSH_COMMAND_ENV_KEY) {
            Some(command) if !is_codex_proxy_git_ssh_command(command) => {}
            _ => {
                env.insert(
                    GIT_SSH_COMMAND_ENV_KEY.to_string(),
                    codex_proxy_git_ssh_command(socks_addr),
                );
            }
        }
    }

    if let Some(mitm_ca_trust_bundle) = mitm_ca_trust_bundle {
        let managed_path = mitm_ca_trust_bundle.path.to_string_lossy().into_owned();
        for key in crate::certs::CUSTOM_CA_ENV_KEYS {
            if env
                .get(key)
                .filter(|value| !value.is_empty())
                .is_some_and(|value| {
                    value != &managed_path
                        && mitm_ca_trust_bundle.startup_env_values.get(key) != Some(value)
                })
            {
                // TODO(winston): Materialize policy-checked per-child bundles for readable
                // startup and command-scoped CA overrides. For now startup overrides are
                // replaced with the default bundle and later command-scoped overrides are
                // preserved, either of which can make intercepted TLS fail.
                continue;
            }
            env.insert(key.to_string(), managed_path.clone());
        }
    }
}

impl NetworkProxy {
    pub fn builder() -> NetworkProxyBuilder {
        NetworkProxyBuilder::default()
    }

    pub fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    pub fn socks_addr(&self) -> SocketAddr {
        #[cfg(target_os = "windows")]
        if let Some(runtime) = self.windows_runtime.as_ref() {
            return runtime.ingress.socks_addr();
        }
        self.socks_addr
    }

    /// Returns the restricting SID that identifies this logical proxy route to the shared
    /// Windows ingress. Environment routes are available after their proxy settings are prepared.
    #[cfg(target_os = "windows")]
    pub fn network_proxy_restricting_sid(&self, environment_id: Option<&str>) -> Option<String> {
        match environment_id {
            Some(environment_id) => self
                .environment_proxies
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(environment_id)
                .and_then(|proxy| proxy.runtime.network_proxy_restricting_sid()),
            None => self.windows_runtime.as_ref().and_then(|runtime| {
                runtime
                    .active_route
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                    .map(|route| route.sid().to_string())
            }),
        }
    }

    pub async fn current_cfg(&self) -> Result<config::NetworkProxyConfig> {
        self.state.current_cfg().await
    }

    /// Captures the static inputs needed to launch a matching executor-local proxy.
    pub async fn remote_launch_config(&self) -> Result<crate::RemoteNetworkProxyLaunchConfig> {
        let proxy = crate::RemoteNetworkProxyConfig::from_effective_config(
            &self.state.current_cfg().await?,
        )?;
        let (environment_id, execution_id) = self
            .execution_scope
            .as_ref()
            .map(|scope| {
                (
                    Some(scope.environment_id.clone()),
                    Some(scope.execution_id.clone()),
                )
            })
            .unwrap_or_default();
        Ok(crate::RemoteNetworkProxyLaunchConfig {
            proxy,
            audit_metadata: self.state.audit_metadata().clone(),
            environment_id,
            execution_id,
        })
    }

    pub async fn add_allowed_domain(&self, host: &str) -> Result<()> {
        self.state.add_allowed_domain(host).await
    }

    pub async fn add_denied_domain(&self, host: &str) -> Result<()> {
        self.state.add_denied_domain(host).await
    }

    pub fn allow_local_binding(&self) -> bool {
        self.runtime_settings().allow_local_binding
    }

    pub fn allow_unix_sockets(&self) -> Arc<[String]> {
        self.runtime_settings().allow_unix_sockets
    }

    pub fn dangerously_allow_all_unix_sockets(&self) -> bool {
        self.runtime_settings().dangerously_allow_all_unix_sockets
    }

    /// Returns the generated MITM CA bundle path child sandboxes should expose to TLS clients.
    pub fn managed_mitm_ca_trust_bundle_path(&self) -> Option<AbsolutePathBuf> {
        self.runtime_settings()
            .mitm_ca_trust_bundle
            .and_then(|bundle| {
                AbsolutePathBuf::from_absolute_path(bundle.path)
                    .map_err(|err| warn!("managed MITM CA trust bundle path is invalid: {err}"))
                    .ok()
            })
    }

    fn prepare_for_addrs(
        &self,
        mut env: HashMap<String, String>,
        addrs: EnvironmentProxyAddrs,
        #[cfg_attr(not(target_os = "windows"), allow(unused_variables))]
        client: EnvironmentProxyClient,
    ) -> PreparedManagedNetwork {
        #[cfg(target_os = "windows")]
        let shared_socks_addr = (client == EnvironmentProxyClient::SandboxedProcess)
            .then(|| {
                self.windows_runtime
                    .as_ref()
                    .and_then(|runtime| runtime.ingress.active_socks_addr())
            })
            .flatten();
        #[cfg(target_os = "windows")]
        let addrs = EnvironmentProxyAddrs {
            socks_addr: shared_socks_addr.unwrap_or(addrs.socks_addr),
            ..addrs
        };
        let runtime_settings = self.runtime_settings();
        // Enforce proxying for child processes. Proxy endpoint values are always rewritten;
        // managed MITM CA vars preserve child-scoped overrides after proxy startup.
        apply_proxy_env_overrides(
            &mut env,
            addrs.http_addr,
            addrs.socks_addr,
            self.socks_enabled,
            runtime_settings.allow_local_binding,
            runtime_settings.mitm_ca_trust_bundle.as_ref(),
        );
        self.state.virtualize_child_credentials(&mut env);
        if let Some(execution_scope) = self.execution_scope.as_ref() {
            env.insert(
                PROXY_ATTRIBUTION_TOKEN_ENV_KEY.to_string(),
                execution_scope.attribution_token.clone(),
            );
        } else {
            env.remove(PROXY_ATTRIBUTION_TOKEN_ENV_KEY);
        }
        let expose_socks_port = self.socks_enabled;
        #[cfg(target_os = "windows")]
        let expose_socks_port = expose_socks_port || shared_socks_addr.is_some();
        let mut loopback_ports = [
            Some(addrs.http_addr),
            expose_socks_port.then_some(addrs.socks_addr),
        ]
        .into_iter()
        .flatten()
        .filter(|addr| addr.ip().is_loopback())
        .map(|addr| addr.port())
        .collect::<Vec<_>>();
        loopback_ports.sort_unstable();
        loopback_ports.dedup();
        #[cfg(target_os = "windows")]
        if client == EnvironmentProxyClient::SandboxedProcess && self.windows_runtime.is_some() {
            env.insert(
                WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY.to_string(),
                loopback_ports
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        } else {
            env.remove(WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY);
        }
        PreparedManagedNetwork {
            env,
            sandbox_context: ManagedNetworkSandboxContext {
                loopback_ports,
                allow_local_binding: runtime_settings.allow_local_binding,
            },
        }
    }

    fn apply_to_env_for_addrs(
        &self,
        env: &mut HashMap<String, String>,
        addrs: EnvironmentProxyAddrs,
    ) {
        let prepared = self.prepare_for_addrs(
            std::mem::take(env),
            addrs,
            EnvironmentProxyClient::SandboxedProcess,
        );
        *env = prepared.env;
    }

    pub fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        self.apply_to_env_for_addrs(
            env,
            EnvironmentProxyAddrs {
                http_addr: self.http_addr,
                socks_addr: self.socks_addr,
            },
        );
    }

    pub fn apply_to_env_for_environment(
        &self,
        env: &mut HashMap<String, String>,
        environment_id: &str,
    ) -> Result<()> {
        let addrs =
            self.environment_proxy_addrs(environment_id, EnvironmentProxyClient::SandboxedProcess)?;
        self.apply_to_env_for_addrs(env, addrs);
        Ok(())
    }

    pub fn apply_to_env_for_optional_environment(
        &self,
        env: &mut HashMap<String, String>,
        environment_id: Option<&str>,
    ) -> Result<()> {
        match environment_id {
            Some(environment_id) => self.apply_to_env_for_environment(env, environment_id),
            None => {
                self.apply_to_env(env);
                Ok(())
            }
        }
    }

    /// Applies the environment-specific proxy settings and returns the matching portable sandbox
    /// projection from the same runtime configuration snapshot.
    pub fn prepare_for_optional_environment(
        &self,
        env: HashMap<String, String>,
        environment_id: Option<&str>,
    ) -> Result<PreparedManagedNetwork> {
        let addrs = match environment_id {
            Some(environment_id) => self.environment_proxy_addrs(
                environment_id,
                EnvironmentProxyClient::SandboxedProcess,
            )?,
            None => EnvironmentProxyAddrs {
                http_addr: self.http_addr,
                socks_addr: self.socks_addr,
            },
        };
        Ok(self.prepare_for_addrs(env, addrs, EnvironmentProxyClient::SandboxedProcess))
    }

    /// Prepares proxy settings for a remote executor whose connection reaches this process through
    /// the trusted proxy bridge rather than directly from a locally spawned sandbox process.
    pub fn prepare_for_remote_environment(
        &self,
        env: HashMap<String, String>,
        environment_id: &str,
    ) -> Result<PreparedManagedNetwork> {
        let addrs =
            self.environment_proxy_addrs(environment_id, EnvironmentProxyClient::TrustedBridge)?;
        Ok(self.prepare_for_addrs(env, addrs, EnvironmentProxyClient::TrustedBridge))
    }

    fn environment_proxy_addrs(
        &self,
        environment_id: &str,
        #[cfg_attr(not(target_os = "windows"), allow(unused_variables))]
        client: EnvironmentProxyClient,
    ) -> Result<EnvironmentProxyAddrs> {
        if let Some(execution_scope) = self.execution_scope.as_ref() {
            anyhow::ensure!(
                execution_scope.environment_id == environment_id,
                "execution-scoped network proxy belongs to environment `{}`, not `{environment_id}`",
                execution_scope.environment_id
            );
        }

        let mut proxies = self
            .environment_proxies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(proxy) = proxies.get(environment_id) {
            #[cfg(target_os = "windows")]
            let uses_shared_ingress = client == EnvironmentProxyClient::SandboxedProcess
                && self.windows_runtime.is_some();
            #[cfg(target_os = "windows")]
            anyhow::ensure!(
                matches!(
                    (&proxy.runtime, uses_shared_ingress),
                    (EnvironmentProxyRuntime::SharedIngress { .. }, true)
                        | (EnvironmentProxyRuntime::ListenerTasks { .. }, false)
                ),
                "network proxy for environment `{environment_id}` was prepared for a different client type"
            );
            return Ok(proxy.addrs);
        }

        #[cfg(target_os = "windows")]
        if client == EnvironmentProxyClient::SandboxedProcess
            && let Some(windows_runtime) = self.windows_runtime.as_ref()
        {
            let active_route = windows_runtime
                .active_route
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            anyhow::ensure!(
                active_route.is_some(),
                "shared managed Windows proxy route is not running"
            );
            let environment_id = environment_id.to_string();
            let http = http_proxy::http_proxy_service(
                Arc::clone(&self.state),
                self.policy_decider.clone(),
                Some(environment_id.clone()),
            );
            let socks = self.socks_enabled.then(|| {
                socks5::socks5_proxy_service(
                    Arc::clone(&self.state),
                    self.policy_decider.clone(),
                    Some(environment_id.clone()),
                    self.socks5_udp_enabled,
                )
            });
            let route = Arc::new(windows_runtime.ingress.register_route(http, socks));
            let addrs = EnvironmentProxyAddrs {
                http_addr: self.http_addr,
                socks_addr: self.socks_addr,
            };
            proxies.insert(
                environment_id,
                EnvironmentProxy {
                    addrs,
                    runtime: EnvironmentProxyRuntime::SharedIngress { _route: route },
                },
            );
            return Ok(addrs);
        }

        let runtime = tokio::runtime::Handle::try_current().with_context(|| {
            format!("failed to create network proxy for environment `{environment_id}`")
        })?;
        let listeners =
            reserve_loopback_ephemeral_listeners(self.socks_enabled).with_context(|| {
                format!("failed to reserve network proxy for environment `{environment_id}`")
            })?;
        let http_addr = listeners.http_addr().with_context(|| {
            format!("failed to read HTTP proxy address for environment `{environment_id}`")
        })?;
        let socks_addr = listeners.socks_addr(self.socks_addr).with_context(|| {
            format!("failed to read SOCKS proxy address for environment `{environment_id}`")
        })?;
        let addrs = EnvironmentProxyAddrs {
            http_addr,
            socks_addr,
        };
        let ReservedListenerSet {
            http_listener,
            socks_listener,
        } = listeners;

        let environment_id = environment_id.to_string();
        let http_state = self.state.clone();
        let http_decider = self.policy_decider.clone();
        let http_environment_id = Some(environment_id.clone());
        let http_task = runtime.spawn(async move {
            http_proxy::run_http_proxy_with_std_listener(
                http_state,
                http_listener,
                http_decider,
                http_environment_id,
            )
            .await
        });

        let socks_task = if self.socks_enabled {
            let socks_state = self.state.clone();
            let socks_decider = self.policy_decider.clone();
            let socks_environment_id = Some(environment_id.clone());
            let socks5_udp_enabled = self.socks5_udp_enabled;
            socks_listener.map(|listener| {
                runtime.spawn(async move {
                    socks5::run_socks5_with_std_listener(
                        socks_state,
                        listener,
                        socks_decider,
                        socks_environment_id,
                        socks5_udp_enabled,
                    )
                    .await
                })
            })
        } else {
            None
        };

        proxies.insert(
            environment_id,
            EnvironmentProxy {
                addrs,
                runtime: EnvironmentProxyRuntime::ListenerTasks {
                    http_task,
                    socks_task,
                },
            },
        );
        Ok(addrs)
    }

    pub async fn replace_config_state(&self, new_state: ConfigState) -> Result<()> {
        let current_cfg = self.state.current_cfg().await?;
        anyhow::ensure!(
            new_state.config.enabled == current_cfg.enabled,
            "cannot update network.enabled on a running proxy"
        );
        anyhow::ensure!(
            new_state.config.proxy_url == current_cfg.proxy_url,
            "cannot update network.proxy_url on a running proxy"
        );
        anyhow::ensure!(
            new_state.config.socks_url == current_cfg.socks_url,
            "cannot update network.socks_url on a running proxy"
        );
        anyhow::ensure!(
            new_state.config.enable_socks5 == current_cfg.enable_socks5,
            "cannot update network.enable_socks5 on a running proxy"
        );
        anyhow::ensure!(
            new_state.config.enable_socks5_udp == current_cfg.enable_socks5_udp,
            "cannot update network.enable_socks5_udp on a running proxy"
        );
        let settings = NetworkProxyRuntimeSettings::from_config(&new_state.config)?;
        self.state.replace_config_state(new_state).await?;
        let mut guard = self
            .runtime_settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = settings;
        Ok(())
    }

    fn runtime_settings(&self) -> NetworkProxyRuntimeSettings {
        self.runtime_settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub async fn run(&self) -> Result<NetworkProxyHandle> {
        anyhow::ensure!(
            self.execution_scope.is_none(),
            "execution-scoped network proxy is already running"
        );
        let current_cfg = self.state.current_cfg().await?;
        if !current_cfg.enabled {
            warn!("network.enabled is false; skipping proxy listeners");
            return Ok(NetworkProxyHandle::noop());
        }

        if !unix_socket_permissions_supported() {
            warn!(
                "allowUnixSockets and dangerouslyAllowAllUnixSockets are macOS-only; requests will be rejected on this platform"
            );
        }

        #[cfg(target_os = "windows")]
        if let Some(windows_runtime) = self.windows_runtime.as_ref() {
            let mut active_route = windows_runtime
                .active_route
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            anyhow::ensure!(
                active_route.is_none(),
                "shared managed Windows proxy route is already running"
            );
            *active_route = Some(Arc::new(windows_runtime.ingress.register_route(
                windows_runtime.http_service.clone(),
                windows_runtime.socks_service.clone(),
            )));
            drop(active_route);
            return Ok(NetworkProxyHandle::windows_shared(
                Arc::clone(&windows_runtime.active_route),
                Arc::clone(&self.environment_proxies),
            ));
        }

        let reserved_listeners = self.reserved_listeners.as_ref();
        let http_listener = reserved_listeners.and_then(|listeners| listeners.take_http());
        let socks_listener = reserved_listeners.and_then(|listeners| listeners.take_socks());

        let http_state = self.state.clone();
        let http_decider = self.policy_decider.clone();
        let http_addr = self.http_addr;
        let http_task = tokio::spawn(async move {
            match http_listener {
                Some(listener) => {
                    http_proxy::run_http_proxy_with_std_listener(
                        http_state,
                        listener,
                        http_decider,
                        /*environment_id*/ None,
                    )
                    .await
                }
                None => {
                    http_proxy::run_http_proxy(
                        http_state,
                        http_addr,
                        http_decider,
                        /*environment_id*/ None,
                    )
                    .await
                }
            }
        });

        let socks_task = if current_cfg.enable_socks5 {
            let socks_state = self.state.clone();
            let socks_decider = self.policy_decider.clone();
            let socks_addr = self.socks_addr;
            let enable_socks5_udp = current_cfg.enable_socks5_udp;
            Some(tokio::spawn(async move {
                match socks_listener {
                    Some(listener) => {
                        socks5::run_socks5_with_std_listener(
                            socks_state,
                            listener,
                            socks_decider,
                            /*environment_id*/ None,
                            enable_socks5_udp,
                        )
                        .await
                    }
                    None => {
                        socks5::run_socks5(
                            socks_state,
                            socks_addr,
                            socks_decider,
                            /*environment_id*/ None,
                            enable_socks5_udp,
                        )
                        .await
                    }
                }
            }))
        } else {
            None
        };

        Ok(NetworkProxyHandle {
            http_task: Some(http_task),
            socks_task,
            environment_proxies: self.environment_proxies.clone(),
            completed: false,
            #[cfg(target_os = "windows")]
            windows_active_route: None,
        })
    }
}

pub struct NetworkProxyHandle {
    http_task: Option<JoinHandle<Result<()>>>,
    socks_task: Option<JoinHandle<Result<()>>>,
    environment_proxies: Arc<Mutex<HashMap<String, EnvironmentProxy>>>,
    completed: bool,
    #[cfg(target_os = "windows")]
    windows_active_route: Option<Arc<Mutex<Option<Arc<WindowsProxyRoute>>>>>,
}

impl NetworkProxyHandle {
    fn noop() -> Self {
        Self {
            http_task: Some(tokio::spawn(async { Ok(()) })),
            socks_task: None,
            environment_proxies: Arc::new(Mutex::new(HashMap::new())),
            completed: true,
            #[cfg(target_os = "windows")]
            windows_active_route: None,
        }
    }

    #[cfg(target_os = "windows")]
    fn windows_shared(
        active_route: Arc<Mutex<Option<Arc<WindowsProxyRoute>>>>,
        environment_proxies: Arc<Mutex<HashMap<String, EnvironmentProxy>>>,
    ) -> Self {
        Self {
            http_task: Some(tokio::spawn(async {
                std::future::pending::<()>().await;
                Ok(())
            })),
            socks_task: None,
            environment_proxies,
            completed: false,
            windows_active_route: Some(active_route),
        }
    }

    #[cfg(target_os = "windows")]
    fn deactivate_windows_route(&mut self) {
        if let Some(active_route) = self.windows_active_route.take() {
            active_route
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
    }

    pub async fn wait(mut self) -> Result<()> {
        let http_task = self.http_task.take().context("missing http proxy task")?;
        let socks_task = self.socks_task.take();
        let http_result = http_task.await;
        let socks_result = match socks_task {
            Some(task) => Some(task.await),
            None => None,
        };
        #[cfg(target_os = "windows")]
        self.deactivate_windows_route();
        self.completed = true;
        abort_environment_proxies(self.environment_proxies.clone()).await;
        http_result??;
        if let Some(socks_result) = socks_result {
            socks_result??;
        }
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        self.deactivate_windows_route();
        abort_tasks(self.http_task.take(), self.socks_task.take()).await;
        abort_environment_proxies(self.environment_proxies.clone()).await;
        self.completed = true;
        Ok(())
    }
}

async fn abort_task(task: Option<JoinHandle<Result<()>>>) {
    if let Some(task) = task {
        task.abort();
        let _ = task.await;
    }
}

async fn abort_tasks(
    http_task: Option<JoinHandle<Result<()>>>,
    socks_task: Option<JoinHandle<Result<()>>>,
) {
    abort_task(http_task).await;
    abort_task(socks_task).await;
}

async fn abort_environment_proxies(
    environment_proxies: Arc<Mutex<HashMap<String, EnvironmentProxy>>>,
) {
    let proxies = {
        let mut guard = environment_proxies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.drain().map(|(_, proxy)| proxy).collect::<Vec<_>>()
    };
    for proxy in proxies {
        match proxy.runtime {
            EnvironmentProxyRuntime::ListenerTasks {
                http_task,
                socks_task,
            } => {
                abort_task(Some(http_task)).await;
                abort_task(socks_task).await;
            }
            #[cfg(target_os = "windows")]
            EnvironmentProxyRuntime::SharedIngress { .. } => {}
        }
    }
}

#[cfg(target_os = "windows")]
fn unregister_windows_ingress_environment_routes(
    environment_proxies: &Arc<Mutex<HashMap<String, EnvironmentProxy>>>,
) {
    environment_proxies
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, proxy| {
            matches!(
                &proxy.runtime,
                EnvironmentProxyRuntime::ListenerTasks { .. }
            )
        });
}

impl Drop for NetworkProxyHandle {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let http_task = self.http_task.take();
        let socks_task = self.socks_task.take();
        let environment_proxies = self.environment_proxies.clone();
        #[cfg(target_os = "windows")]
        {
            self.deactivate_windows_route();
            unregister_windows_ingress_environment_routes(&environment_proxies);
        }
        tokio::spawn(async move {
            abort_tasks(http_task, socks_task).await;
            abort_environment_proxies(environment_proxies).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkProxyConfig;
    use crate::state::network_proxy_state_for_policy;
    use pretty_assertions::assert_eq;
    use std::net::IpAddr;
    use std::net::Ipv4Addr;
    use std::path::Path;

    #[cfg(target_os = "windows")]
    static WINDOWS_INGRESS_TEST_LOCK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

    #[tokio::test]
    async fn managed_proxy_builder_uses_loopback_ports() {
        #[cfg(target_os = "windows")]
        let _permit = WINDOWS_INGRESS_TEST_LOCK.acquire().await.unwrap();
        let http_listener = StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let http_addr = http_listener.local_addr().unwrap();
        let socks_listener = StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let socks_addr = socks_listener.local_addr().unwrap();
        drop(http_listener);
        drop(socks_listener);

        let state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig {
            enabled: true,
            proxy_url: format!("http://{http_addr}"),
            socks_url: format!("http://{socks_addr}"),
            ..NetworkProxyConfig::default()
        }));
        let proxy = match NetworkProxy::builder().state(state).build().await {
            Ok(proxy) => proxy,
            Err(err) => {
                if err
                    .chain()
                    .any(|cause| cause.to_string().contains("Operation not permitted"))
                {
                    return;
                }
                panic!("failed to build managed proxy: {err:#}");
            }
        };

        assert!(proxy.http_addr.ip().is_loopback());
        assert!(proxy.socks_addr.ip().is_loopback());
        #[cfg(target_os = "windows")]
        {
            assert_eq!(proxy.http_addr, http_addr);
            assert_eq!(proxy.socks_addr, socks_addr);
            assert_eq!(proxy.network_proxy_restricting_sid(None), None);
            let handle = proxy.run().await.expect("start stable ingress route");
            let second_state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig {
                enabled: true,
                proxy_url: format!("http://{http_addr}"),
                socks_url: format!("http://{socks_addr}"),
                ..NetworkProxyConfig::default()
            }));
            let second = NetworkProxy::builder()
                .state(second_state)
                .build()
                .await
                .expect("second proxy should share the stable ingress");
            let second_handle = second
                .run()
                .await
                .expect("start second stable ingress route");
            assert_eq!(second.http_addr, proxy.http_addr);
            assert_eq!(second.socks_addr, proxy.socks_addr);
            assert_ne!(
                second.network_proxy_restricting_sid(None),
                proxy.network_proxy_restricting_sid(None)
            );
            let differently_configured =
                Arc::new(network_proxy_state_for_policy(NetworkProxyConfig {
                    enabled: true,
                    proxy_url: "http://127.0.0.1:1".to_string(),
                    socks_url: "http://127.0.0.1:2".to_string(),
                    allow_local_binding: true,
                    ..NetworkProxyConfig::default()
                }));
            let third = NetworkProxy::builder()
                .state(differently_configured)
                .build()
                .await
                .expect("different route config should share the stable ingress");
            let third_handle = third
                .run()
                .await
                .expect("start differently configured stable route");
            assert_eq!(third.http_addr, proxy.http_addr);
            assert_eq!(third.socks_addr, proxy.socks_addr);
            assert!(third.allow_local_binding());
            let replacement = crate::state::build_config_state(
                NetworkProxyConfig {
                    enabled: true,
                    proxy_url: format!("http://{http_addr}"),
                    socks_url: format!("http://{socks_addr}"),
                    allow_local_binding: true,
                    ..NetworkProxyConfig::default()
                },
                Default::default(),
            )
            .expect("replacement config state");
            proxy
                .replace_config_state(replacement)
                .await
                .expect("live route should accept a local-binding policy change");
            assert!(proxy.allow_local_binding());
            third_handle
                .shutdown()
                .await
                .expect("stop differently configured stable route");
            second_handle
                .shutdown()
                .await
                .expect("stop second stable ingress route");
            handle.shutdown().await.expect("stop stable ingress route");
            assert_eq!(proxy.network_proxy_restricting_sid(None), None);
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_ne!(proxy.http_addr.port(), 0);
            assert_ne!(proxy.socks_addr.port(), 0);
        }
    }

    #[tokio::test]
    async fn non_codex_managed_proxy_builder_uses_configured_ports() {
        let settings = NetworkProxyConfig {
            proxy_url: "http://127.0.0.1:43128".to_string(),
            socks_url: "http://127.0.0.1:48081".to_string(),
            ..NetworkProxyConfig::default()
        };
        let state = Arc::new(network_proxy_state_for_policy(settings));
        let proxy = NetworkProxy::builder()
            .state(state)
            .managed_by_codex(/*managed_by_codex*/ false)
            .build()
            .await
            .unwrap();

        assert_eq!(
            proxy.http_addr,
            "127.0.0.1:43128".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            proxy.socks_addr,
            "127.0.0.1:48081".parse::<SocketAddr>().unwrap()
        );
    }

    #[tokio::test]
    async fn prepare_for_environment_keeps_env_and_sandbox_ports_in_sync() -> Result<()> {
        #[cfg(target_os = "windows")]
        let _permit = WINDOWS_INGRESS_TEST_LOCK.acquire().await.unwrap();
        let state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig {
            enabled: true,
            ..NetworkProxyConfig::default()
        }));
        let proxy = NetworkProxy::builder().state(state).build().await?;
        let handle = proxy.run().await?;

        let base_env = HashMap::from([("PRESERVED".to_string(), "value".to_string())]);
        let local = proxy.prepare_for_optional_environment(base_env.clone(), Some("local"))?;
        let remote = proxy.prepare_for_remote_environment(HashMap::new(), "remote")?;

        assert_eq!(
            local.env.get("PRESERVED").map(String::as_str),
            Some("value")
        );
        #[cfg(target_os = "windows")]
        {
            assert_eq!(
                local.env.get("HTTP_PROXY"),
                Some(&format!("http://{}", proxy.http_addr()))
            );
            assert_ne!(local.env.get("HTTP_PROXY"), remote.env.get("HTTP_PROXY"));
            assert!(proxy.network_proxy_restricting_sid(Some("local")).is_some());
            assert_eq!(proxy.network_proxy_restricting_sid(Some("remote")), None);
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_ne!(local.env.get("HTTP_PROXY"), remote.env.get("HTTP_PROXY"));
            assert_ne!(
                local.env.get("HTTP_PROXY"),
                Some(&format!("http://{}", proxy.http_addr()))
            );
            assert_ne!(
                remote.env.get("HTTP_PROXY"),
                Some(&format!("http://{}", proxy.http_addr()))
            );
        }
        for prepared in [&local, &remote] {
            let http_port = prepared
                .env
                .get("HTTP_PROXY")
                .and_then(|value| value.strip_prefix("http://"))
                .and_then(|value| value.parse::<SocketAddr>().ok())
                .map(|addr| addr.port())
                .expect("managed HTTP proxy address");
            let socks_port = prepared
                .env
                .get("ALL_PROXY")
                .and_then(|value| value.strip_prefix("socks5h://"))
                .and_then(|value| value.parse::<SocketAddr>().ok())
                .map(|addr| addr.port())
                .expect("managed SOCKS proxy address");
            let mut expected_ports = vec![http_port, socks_port];
            expected_ports.sort_unstable();
            expected_ports.dedup();
            assert_eq!(
                prepared.sandbox_context,
                ManagedNetworkSandboxContext {
                    loopback_ports: expected_ports,
                    allow_local_binding: false,
                }
            );
        }
        let mut legacy_env = base_env;
        proxy.apply_to_env_for_environment(&mut legacy_env, "local")?;
        assert_eq!(legacy_env, local.env);

        handle.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn remote_launch_config_carries_execution_scope() -> Result<()> {
        #[cfg(target_os = "windows")]
        let _permit = WINDOWS_INGRESS_TEST_LOCK.acquire().await.unwrap();
        let state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig::default()));
        let proxy = match NetworkProxy::builder().state(state).build().await {
            Ok(proxy) => proxy,
            Err(err) => {
                if err
                    .chain()
                    .any(|cause| cause.to_string().contains("Operation not permitted"))
                {
                    return Ok(());
                }
                return Err(err);
            }
        };

        let scoped = proxy.for_execution("remote-env", "execution-1", "token-1".to_string())?;
        let launch = scoped.remote_launch_config().await?;
        let prepared = scoped.prepare_for_optional_environment(
            HashMap::from([(
                PROXY_ATTRIBUTION_TOKEN_ENV_KEY.to_string(),
                "foreign-token".to_string(),
            )]),
            /*environment_id*/ None,
        )?;

        assert_eq!(launch.environment_id.as_deref(), Some("remote-env"));
        assert_eq!(launch.execution_id.as_deref(), Some("execution-1"));
        assert_eq!(
            prepared
                .env
                .get(PROXY_ATTRIBUTION_TOKEN_ENV_KEY)
                .map(String::as_str),
            Some("token-1")
        );
        Ok(())
    }

    #[tokio::test]
    async fn managed_proxy_builder_lazily_upgrades_disabled_socks() {
        #[cfg(target_os = "windows")]
        let _permit = WINDOWS_INGRESS_TEST_LOCK.acquire().await.unwrap();
        let http_listener = StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let http_addr = http_listener.local_addr().unwrap();
        drop(http_listener);
        let occupied_socks = StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let socks_addr = occupied_socks.local_addr().unwrap();
        let settings = NetworkProxyConfig {
            enabled: true,
            enable_socks5: false,
            proxy_url: format!("http://{http_addr}"),
            socks_url: format!("http://{socks_addr}"),
            ..NetworkProxyConfig::default()
        };
        let state = Arc::new(network_proxy_state_for_policy(settings));
        let proxy = match NetworkProxy::builder().state(state).build().await {
            Ok(proxy) => proxy,
            Err(err) => {
                if err
                    .chain()
                    .any(|cause| cause.to_string().contains("Operation not permitted"))
                {
                    return;
                }
                panic!("failed to build managed proxy: {err:#}");
            }
        };

        assert!(proxy.http_addr.ip().is_loopback());
        assert_ne!(proxy.http_addr.port(), 0);
        assert_eq!(proxy.socks_addr, socks_addr);
        #[cfg(target_os = "windows")]
        {
            assert_eq!(proxy.http_addr, http_addr);
            assert!(proxy.reserved_listeners.is_none());
            assert!(proxy.windows_runtime.is_some());
            assert_eq!(proxy.network_proxy_restricting_sid(None), None);
            let handle = proxy.run().await.expect("start HTTP-only stable route");
            assert!(proxy.network_proxy_restricting_sid(None).is_some());
            let prepared_before_upgrade = proxy
                .prepare_for_optional_environment(
                    HashMap::from([(
                        WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY.to_string(),
                        "1,2".to_string(),
                    )]),
                    None,
                )
                .expect("prepare stable Windows proxy");
            assert_eq!(
                prepared_before_upgrade.sandbox_context.loopback_ports,
                vec![proxy.http_addr.port()]
            );
            assert_eq!(
                prepared_before_upgrade
                    .env
                    .get(WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY),
                Some(&proxy.http_addr.port().to_string())
            );
            assert_eq!(
                prepared_before_upgrade.env.get("ALL_PROXY"),
                Some(&format!("http://{}", proxy.http_addr))
            );
            let environment_id = "cached-before-socks-upgrade";
            let environment_before_upgrade = proxy
                .prepare_for_optional_environment(HashMap::new(), Some(environment_id))
                .expect("prepare cached HTTP-only environment");
            assert_eq!(
                environment_before_upgrade.sandbox_context.loopback_ports,
                vec![proxy.http_addr.port()]
            );

            let requested_socks =
                StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
            let requested_socks_addr = requested_socks.local_addr().unwrap();
            assert_ne!(requested_socks_addr, socks_addr);
            let socks_state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig {
                enabled: true,
                enable_socks5: true,
                proxy_url: format!("http://{http_addr}"),
                socks_url: format!("socks5://{requested_socks_addr}"),
                ..NetworkProxyConfig::default()
            }));
            let socks_proxy = NetworkProxy::builder()
                .state(socks_state)
                .build()
                .await
                .expect("upgrade stable ingress to SOCKS5");
            let actual_socks_addr = socks_proxy.socks_addr();
            assert_eq!(socks_proxy.http_addr(), proxy.http_addr());
            assert!(actual_socks_addr.ip().is_loopback());
            assert_ne!(actual_socks_addr, requested_socks_addr);
            assert_eq!(proxy.socks_addr(), actual_socks_addr);
            let socks_handle = socks_proxy
                .run()
                .await
                .expect("start SOCKS-enabled stable route");

            let mut expected_ports = vec![proxy.http_addr.port(), actual_socks_addr.port()];
            expected_ports.sort_unstable();
            let prepared_after_upgrade = proxy
                .prepare_for_optional_environment(HashMap::new(), None)
                .expect("re-prepare HTTP-only route after SOCKS5 upgrade");
            let environment_after_upgrade = proxy
                .prepare_for_optional_environment(HashMap::new(), Some(environment_id))
                .expect("re-prepare cached environment after SOCKS5 upgrade");
            for prepared in [&prepared_after_upgrade, &environment_after_upgrade] {
                assert_eq!(prepared.sandbox_context.loopback_ports, expected_ports);
                assert_eq!(
                    prepared.env.get(WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY),
                    Some(
                        &expected_ports
                            .iter()
                            .map(u16::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                );
                assert_eq!(
                    prepared.env.get("ALL_PROXY"),
                    Some(&format!("http://{}", proxy.http_addr))
                );
            }
            let socks_prepared = socks_proxy
                .prepare_for_optional_environment(HashMap::new(), None)
                .expect("prepare SOCKS-enabled route");
            assert_eq!(
                socks_prepared.sandbox_context.loopback_ports,
                expected_ports
            );
            assert_eq!(
                socks_prepared.env.get("ALL_PROXY"),
                Some(&format!("socks5h://{actual_socks_addr}"))
            );
            let remote = proxy
                .prepare_for_remote_environment(
                    HashMap::from([(
                        WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY.to_string(),
                        "1,2".to_string(),
                    )]),
                    "remote",
                )
                .expect("prepare HTTP-only trusted bridge proxy");
            assert_eq!(remote.sandbox_context.loopback_ports.len(), 1);
            assert_eq!(remote.env.get(WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY), None);
            assert_eq!(proxy.network_proxy_restricting_sid(Some("remote")), None);
            socks_handle
                .shutdown()
                .await
                .expect("stop SOCKS-enabled stable route");
            handle
                .shutdown()
                .await
                .expect("stop HTTP-only stable route");
            assert_eq!(proxy.network_proxy_restricting_sid(None), None);
        }
        #[cfg(not(target_os = "windows"))]
        assert!(
            proxy
                .reserved_listeners
                .as_ref()
                .expect("managed builder should reserve listeners")
                .take_socks()
                .is_none()
        );
        drop(proxy);
        drop(occupied_socks);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_managed_loopback_addr_clamps_non_loopback_inputs() {
        assert_eq!(
            windows_managed_loopback_addr("0.0.0.0:3128".parse::<SocketAddr>().unwrap()),
            "127.0.0.1:3128".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            windows_managed_loopback_addr("[::]:8081".parse::<SocketAddr>().unwrap()),
            "127.0.0.1:8081".parse::<SocketAddr>().unwrap()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reserve_windows_managed_listeners_falls_back_when_http_port_is_busy() {
        let occupied = StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let busy_port = occupied.local_addr().unwrap().port();

        let reserved = reserve_windows_managed_listeners(
            SocketAddr::from(([127, 0, 0, 1], busy_port)),
            SocketAddr::from(([127, 0, 0, 1], 48081)),
            /*reserve_socks_listener*/ false,
        )
        .unwrap();

        assert!(reserved.socks_listener.is_none());
        assert!(
            reserved
                .http_listener
                .local_addr()
                .unwrap()
                .ip()
                .is_loopback()
        );
        assert_ne!(
            reserved.http_listener.local_addr().unwrap().port(),
            busy_port
        );
    }

    #[test]
    fn proxy_url_env_value_resolves_lowercase_aliases() {
        let mut env = HashMap::new();
        env.insert(
            "http_proxy".to_string(),
            "http://127.0.0.1:3128".to_string(),
        );

        assert_eq!(
            proxy_url_env_value(&env, "HTTP_PROXY"),
            Some("http://127.0.0.1:3128")
        );
    }

    #[test]
    fn has_proxy_url_env_vars_detects_lowercase_aliases() {
        let mut env = HashMap::new();
        env.insert(
            "all_proxy".to_string(),
            "socks5h://127.0.0.1:8081".to_string(),
        );

        assert_eq!(has_proxy_url_env_vars(&env), true);
    }

    #[test]
    fn has_proxy_url_env_vars_detects_websocket_proxy_keys() {
        let mut env = HashMap::new();
        env.insert("wss_proxy".to_string(), "http://127.0.0.1:3128".to_string());

        assert_eq!(has_proxy_url_env_vars(&env), true);
    }

    #[test]
    fn apply_proxy_env_overrides_sets_common_tool_vars() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get("HTTP_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("WS_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("WSS_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("npm_config_proxy"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("ALL_PROXY"),
            Some(&"socks5h://127.0.0.1:8081".to_string())
        );
        assert_eq!(
            env.get("FTP_PROXY"),
            Some(&"socks5h://127.0.0.1:8081".to_string())
        );
        assert_eq!(env.get("NO_PROXY"), Some(&String::new()));
        assert_eq!(env.get(PROXY_ACTIVE_ENV_KEY), Some(&"1".to_string()));
        assert_eq!(env.get(ALLOW_LOCAL_BINDING_ENV_KEY), Some(&"0".to_string()));
        assert_eq!(
            env.get(ELECTRON_GET_USE_PROXY_ENV_KEY),
            Some(&"true".to_string())
        );
        assert_eq!(env.get(NODE_USE_ENV_PROXY_ENV_KEY), Some(&"1".to_string()));
        #[cfg(target_os = "macos")]
        assert_eq!(
            env.get(GIT_SSH_COMMAND_ENV_KEY),
            Some(
                &"CODEX_PROXY_GIT_SSH_COMMAND=1 ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'"
                    .to_string()
            )
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(env.get(GIT_SSH_COMMAND_ENV_KEY), None);
    }

    #[test]
    fn apply_proxy_env_overrides_keeps_local_targets_direct_when_local_binding_enabled() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ true,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get("NO_PROXY"),
            Some(&DEFAULT_NO_PROXY_VALUE.to_string())
        );
    }

    #[test]
    fn apply_proxy_env_overrides_sets_only_expected_env_keys() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        for key in env.keys() {
            let is_managed_git_ssh_key =
                cfg!(target_os = "macos") && key == GIT_SSH_COMMAND_ENV_KEY;
            assert!(
                PROXY_ENV_KEYS.contains(&key.as_str()) || is_managed_git_ssh_key,
                "proxy env writer set unexpected key: {key}"
            );
        }
    }

    #[test]
    fn apply_proxy_env_overrides_sets_mitm_ca_trust_bundle_vars() {
        let mut env = HashMap::new();
        let mitm_ca_trust_bundle_path = Path::new("/tmp/codex-proxy/ca-bundle.pem");
        let mitm_ca_trust_bundle = crate::certs::ManagedMitmCaTrustBundle {
            path: mitm_ca_trust_bundle_path.to_path_buf(),
            startup_env_values: HashMap::new(),
        };
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            Some(&mitm_ca_trust_bundle),
        );

        for key in crate::certs::CUSTOM_CA_ENV_KEYS {
            assert_eq!(
                env.get(key),
                Some(&mitm_ca_trust_bundle_path.display().to_string())
            );
        }
    }

    #[test]
    fn apply_proxy_env_overrides_preserves_command_scoped_mitm_ca_override() {
        let command_ca_bundle_path = "/tmp/command-ca.pem".to_string();
        let mut env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            command_ca_bundle_path.clone(),
        )]);
        let mitm_ca_trust_bundle_path = Path::new("/tmp/codex-proxy/ca-bundle.pem");
        let mitm_ca_trust_bundle = crate::certs::ManagedMitmCaTrustBundle {
            path: mitm_ca_trust_bundle_path.to_path_buf(),
            startup_env_values: HashMap::new(),
        };

        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            Some(&mitm_ca_trust_bundle),
        );

        assert_eq!(env.get("REQUESTS_CA_BUNDLE"), Some(&command_ca_bundle_path));
        assert_eq!(
            env.get("SSL_CERT_FILE"),
            Some(&mitm_ca_trust_bundle_path.display().to_string())
        );
    }

    #[test]
    fn apply_proxy_env_overrides_uses_http_for_all_proxy_without_socks() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ false,
            /*allow_local_binding*/ true,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get("ALL_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(env.get(ALLOW_LOCAL_BINDING_ENV_KEY), Some(&"1".to_string()));
    }

    #[test]
    fn apply_proxy_env_overrides_uses_plain_http_proxy_url() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get("HTTP_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("HTTPS_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("WS_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("WSS_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("ALL_PROXY"),
            Some(&"socks5h://127.0.0.1:8081".to_string())
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            env.get(GIT_SSH_COMMAND_ENV_KEY),
            Some(
                &"CODEX_PROXY_GIT_SSH_COMMAND=1 ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'"
                    .to_string()
            )
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(env.get(GIT_SSH_COMMAND_ENV_KEY), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_proxy_env_overrides_preserves_existing_git_ssh_command() {
        let mut env = HashMap::new();
        env.insert(
            GIT_SSH_COMMAND_ENV_KEY.to_string(),
            "ssh -o ProxyCommand='tsh proxy ssh --cluster=dev %r@%h:%p'".to_string(),
        );
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get(GIT_SSH_COMMAND_ENV_KEY),
            Some(&"ssh -o ProxyCommand='tsh proxy ssh --cluster=dev %r@%h:%p'".to_string())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_proxy_env_overrides_preserves_unmarked_git_ssh_command_with_proxy_shape() {
        let mut env = HashMap::new();
        env.insert(
            GIT_SSH_COMMAND_ENV_KEY.to_string(),
            "ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'".to_string(),
        );
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 48081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get(GIT_SSH_COMMAND_ENV_KEY),
            Some(&"ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'".to_string())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_proxy_env_overrides_refreshes_previous_codex_proxy_git_ssh_command() {
        let mut env = HashMap::new();
        env.insert(
            GIT_SSH_COMMAND_ENV_KEY.to_string(),
            codex_proxy_git_ssh_command(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081)),
        );

        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 43128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 48081),
            /*socks_enabled*/ true,
            /*allow_local_binding*/ false,
            /*mitm_ca_trust_bundle*/ None,
        );

        assert_eq!(
            env.get(GIT_SSH_COMMAND_ENV_KEY),
            Some(&codex_proxy_git_ssh_command(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                48081,
            )))
        );
    }
}
