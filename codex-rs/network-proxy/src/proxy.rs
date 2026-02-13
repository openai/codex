use crate::admin;
use crate::attempt_metadata::proxy_username_for_attempt_id;
use crate::config;
use crate::http_proxy;
use crate::network_policy::NetworkPolicyDecider;
use crate::runtime::BlockedRequest;
use crate::runtime::unix_socket_permissions_supported;
use crate::socks5;
use crate::state::NetworkProxyState;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::warn;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {}

#[derive(Debug)]
struct ReservedListeners {
    http: Mutex<Option<StdTcpListener>>,
    socks: Mutex<Option<StdTcpListener>>,
    admin: Mutex<Option<StdTcpListener>>,
}

impl ReservedListeners {
    fn new(http: StdTcpListener, socks: Option<StdTcpListener>, admin: StdTcpListener) -> Self {
        Self {
            http: Mutex::new(Some(http)),
            socks: Mutex::new(socks),
            admin: Mutex::new(Some(admin)),
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

    fn take_admin(&self) -> Option<StdTcpListener> {
        let mut guard = self
            .admin
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take()
    }
}

#[derive(Clone)]
pub struct NetworkProxyBuilder {
    state: Option<Arc<NetworkProxyState>>,
    http_addr: Option<SocketAddr>,
    socks_addr: Option<SocketAddr>,
    admin_addr: Option<SocketAddr>,
    managed_by_codex: bool,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
}

impl Default for NetworkProxyBuilder {
    fn default() -> Self {
        Self {
            state: None,
            http_addr: None,
            socks_addr: None,
            admin_addr: None,
            managed_by_codex: true,
            policy_decider: None,
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

    pub fn admin_addr(mut self, addr: SocketAddr) -> Self {
        self.admin_addr = Some(addr);
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

    pub async fn build(self) -> Result<NetworkProxy> {
        let state = self.state.ok_or_else(|| {
            anyhow::anyhow!(
                "NetworkProxyBuilder requires a state; supply one via builder.state(...)"
            )
        })?;
        let current_cfg = state.current_cfg().await?;
        let (requested_http_addr, requested_socks_addr, requested_admin_addr, reserved_listeners) =
            if self.managed_by_codex {
                let runtime = config::resolve_runtime(&current_cfg)?;
                let (http_listener, socks_listener, admin_listener) =
                    reserve_loopback_ephemeral_listeners(current_cfg.network.enable_socks5)
                        .context("reserve managed loopback proxy listeners")?;
                let http_addr = http_listener
                    .local_addr()
                    .context("failed to read reserved HTTP proxy address")?;
                let socks_addr = if let Some(socks_listener) = socks_listener.as_ref() {
                    socks_listener
                        .local_addr()
                        .context("failed to read reserved SOCKS5 proxy address")?
                } else {
                    runtime.socks_addr
                };
                let admin_addr = admin_listener
                    .local_addr()
                    .context("failed to read reserved admin API address")?;
                (
                    http_addr,
                    socks_addr,
                    admin_addr,
                    Some(Arc::new(ReservedListeners::new(
                        http_listener,
                        socks_listener,
                        admin_listener,
                    ))),
                )
            } else {
                let runtime = config::resolve_runtime(&current_cfg)?;
                (
                    self.http_addr.unwrap_or(runtime.http_addr),
                    self.socks_addr.unwrap_or(runtime.socks_addr),
                    self.admin_addr.unwrap_or(runtime.admin_addr),
                    None,
                )
            };

        // Reapply bind clamping for caller overrides so unix-socket proxying stays loopback-only.
        let (http_addr, socks_addr, admin_addr) = config::clamp_bind_addrs(
            requested_http_addr,
            requested_socks_addr,
            requested_admin_addr,
            &current_cfg.network,
        );

        Ok(NetworkProxy {
            state,
            http_addr,
            socks_addr,
            socks_enabled: current_cfg.network.enable_socks5,
            allow_local_binding: current_cfg.network.allow_local_binding,
            admin_addr,
            reserved_listeners,
            policy_decider: self.policy_decider,
        })
    }
}

fn reserve_loopback_ephemeral_listeners(
    reserve_socks_listener: bool,
) -> Result<(StdTcpListener, Option<StdTcpListener>, StdTcpListener)> {
    let http_listener =
        reserve_loopback_ephemeral_listener().context("reserve HTTP proxy listener")?;
    let socks_listener = if reserve_socks_listener {
        Some(reserve_loopback_ephemeral_listener().context("reserve SOCKS5 proxy listener")?)
    } else {
        None
    };
    let admin_listener =
        reserve_loopback_ephemeral_listener().context("reserve admin API listener")?;
    Ok((http_listener, socks_listener, admin_listener))
}

fn reserve_loopback_ephemeral_listener() -> Result<StdTcpListener> {
    StdTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .context("bind loopback ephemeral port")
}

#[derive(Clone)]
pub struct NetworkProxy {
    state: Arc<NetworkProxyState>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    socks_enabled: bool,
    allow_local_binding: bool,
    admin_addr: SocketAddr,
    reserved_listeners: Option<Arc<ReservedListeners>>,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
}

impl std::fmt::Debug for NetworkProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Avoid logging internal state (config contents, derived globsets, etc.) which can be noisy
        // and may contain sensitive paths.
        f.debug_struct("NetworkProxy")
            .field("http_addr", &self.http_addr)
            .field("socks_addr", &self.socks_addr)
            .field("admin_addr", &self.admin_addr)
            .finish_non_exhaustive()
    }
}

impl PartialEq for NetworkProxy {
    fn eq(&self, other: &Self) -> bool {
        self.http_addr == other.http_addr
            && self.socks_addr == other.socks_addr
            && self.allow_local_binding == other.allow_local_binding
            && self.admin_addr == other.admin_addr
    }
}

impl Eq for NetworkProxy {}

pub const PROXY_URL_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
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
pub const ALLOW_LOCAL_BINDING_ENV_KEY: &str = "CODEX_NETWORK_ALLOW_LOCAL_BINDING";

const FTP_PROXY_ENV_KEYS: &[&str] = &["FTP_PROXY", "ftp_proxy"];

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
    "*.local,.local,",
    "169.254.0.0/16,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16"
);

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

fn apply_proxy_env_overrides(
    env: &mut HashMap<String, String>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    socks_enabled: bool,
    allow_local_binding: bool,
    network_attempt_id: Option<&str>,
) {
    let http_proxy_url = network_attempt_id
        .map(proxy_username_for_attempt_id)
        .map(|username| format!("http://{username}@{http_addr}"))
        .unwrap_or_else(|| format!("http://{http_addr}"));
    let socks_proxy_url = format!("socks5h://{socks_addr}");
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

    // Keep local/private targets direct so local IPC and metadata endpoints avoid the proxy.
    set_env_keys(env, NO_PROXY_ENV_KEYS, DEFAULT_NO_PROXY_VALUE);

    env.insert("ELECTRON_GET_USE_PROXY".to_string(), "true".to_string());

    // Keep HTTP_PROXY/HTTPS_PROXY as HTTP endpoints. A lot of clients break if
    // those vars contain SOCKS URLs. We only switch ALL_PROXY here.
    //
    // For attempt-scoped runs, point ALL_PROXY at the HTTP proxy URL so the
    // attempt metadata survives in proxy credentials for correlation.
    if socks_enabled && network_attempt_id.is_none() {
        set_env_keys(env, ALL_PROXY_ENV_KEYS, &socks_proxy_url);
        set_env_keys(env, FTP_PROXY_ENV_KEYS, &socks_proxy_url);
        #[cfg(target_os = "macos")]
        {
            // Preserve existing SSH wrappers (for example: Secretive/Teleport setups)
            // and only provide a SOCKS ProxyCommand fallback when one is not present.
            env.entry("GIT_SSH_COMMAND".to_string())
                .or_insert_with(|| format!("ssh -o ProxyCommand='nc -X 5 -x {socks_addr} %h %p'"));
        }
    } else {
        set_env_keys(env, ALL_PROXY_ENV_KEYS, &http_proxy_url);
        set_env_keys(env, FTP_PROXY_ENV_KEYS, &http_proxy_url);
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
        self.socks_addr
    }

    pub fn admin_addr(&self) -> SocketAddr {
        self.admin_addr
    }

    /// Temporarily allows a host for a single retried command execution.
    ///
    /// Returns the normalized host token that should be passed to
    /// `revoke_temporary_allowed_host` when the retry finishes.
    pub async fn grant_temporary_allowed_host(&self, host: &str) -> Option<String> {
        self.state.grant_temporary_allowed_host(host).await
    }

    /// Revokes a temporary host allowance created with
    /// `grant_temporary_allowed_host`.
    pub async fn revoke_temporary_allowed_host(&self, host: &str) {
        self.state.revoke_temporary_allowed_host(host).await;
    }

    /// Drains proxy-blocked request telemetry entries.
    pub async fn drain_blocked_requests(&self) -> Result<Vec<BlockedRequest>> {
        self.state.drain_blocked().await
    }

    /// Returns the current blocked-request telemetry cursor.
    pub async fn blocked_requests_cursor(&self) -> Result<u64> {
        self.state.blocked_cursor().await
    }

    /// Returns blocked-request telemetry entries recorded after `cursor`.
    pub async fn blocked_requests_since(&self, cursor: u64) -> Result<Vec<BlockedRequest>> {
        self.state.blocked_since(cursor).await
    }

    pub async fn latest_blocked_request_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<BlockedRequest>> {
        self.state.latest_blocked_for_attempt(attempt_id).await
    }

    pub fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        self.apply_to_env_for_attempt(env, None);
    }

    pub fn apply_to_env_for_attempt(
        &self,
        env: &mut HashMap<String, String>,
        network_attempt_id: Option<&str>,
    ) {
        // Enforce proxying for child processes. We intentionally override existing values so
        // command-level environment cannot bypass the managed proxy endpoint.
        apply_proxy_env_overrides(
            env,
            self.http_addr,
            self.socks_addr,
            self.socks_enabled,
            self.allow_local_binding,
            network_attempt_id,
        );
    }

    pub async fn run(&self) -> Result<NetworkProxyHandle> {
        let current_cfg = self.state.current_cfg().await?;
        if !current_cfg.network.enabled {
            warn!("network.enabled is false; skipping proxy listeners");
            return Ok(NetworkProxyHandle::noop());
        }

        ensure_rustls_crypto_provider();

        if !unix_socket_permissions_supported() {
            warn!("allowUnixSockets is macOS-only; requests will be rejected on this platform");
        }

        let reserved_listeners = self.reserved_listeners.as_ref();
        let http_listener = reserved_listeners.and_then(|listeners| listeners.take_http());
        let socks_listener = reserved_listeners.and_then(|listeners| listeners.take_socks());
        let admin_listener = reserved_listeners.and_then(|listeners| listeners.take_admin());

        let http_state = self.state.clone();
        let http_decider = self.policy_decider.clone();
        let http_addr = self.http_addr;
        let http_task = tokio::spawn(async move {
            match http_listener {
                Some(listener) => {
                    http_proxy::run_http_proxy_with_std_listener(http_state, listener, http_decider)
                        .await
                }
                None => http_proxy::run_http_proxy(http_state, http_addr, http_decider).await,
            }
        });

        let socks_task = if current_cfg.network.enable_socks5 {
            let socks_state = self.state.clone();
            let socks_decider = self.policy_decider.clone();
            let socks_addr = self.socks_addr;
            let enable_socks5_udp = current_cfg.network.enable_socks5_udp;
            Some(tokio::spawn(async move {
                match socks_listener {
                    Some(listener) => {
                        socks5::run_socks5_with_std_listener(
                            socks_state,
                            listener,
                            socks_decider,
                            enable_socks5_udp,
                        )
                        .await
                    }
                    None => {
                        socks5::run_socks5(
                            socks_state,
                            socks_addr,
                            socks_decider,
                            enable_socks5_udp,
                        )
                        .await
                    }
                }
            }))
        } else {
            None
        };
        let admin_state = self.state.clone();
        let admin_addr = self.admin_addr;
        let admin_task = tokio::spawn(async move {
            match admin_listener {
                Some(listener) => {
                    admin::run_admin_api_with_std_listener(admin_state, listener).await
                }
                None => admin::run_admin_api(admin_state, admin_addr).await,
            }
        });

        Ok(NetworkProxyHandle {
            http_task: Some(http_task),
            socks_task,
            admin_task: Some(admin_task),
            completed: false,
        })
    }
}

pub struct NetworkProxyHandle {
    http_task: Option<JoinHandle<Result<()>>>,
    socks_task: Option<JoinHandle<Result<()>>>,
    admin_task: Option<JoinHandle<Result<()>>>,
    completed: bool,
}

impl NetworkProxyHandle {
    fn noop() -> Self {
        Self {
            http_task: Some(tokio::spawn(async { Ok(()) })),
            socks_task: None,
            admin_task: Some(tokio::spawn(async { Ok(()) })),
            completed: true,
        }
    }

    pub async fn wait(mut self) -> Result<()> {
        let http_task = self.http_task.take().context("missing http proxy task")?;
        let admin_task = self.admin_task.take().context("missing admin proxy task")?;
        let socks_task = self.socks_task.take();
        let http_result = http_task.await;
        let admin_result = admin_task.await;
        let socks_result = match socks_task {
            Some(task) => Some(task.await),
            None => None,
        };
        self.completed = true;
        http_result??;
        admin_result??;
        if let Some(socks_result) = socks_result {
            socks_result??;
        }
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        abort_tasks(
            self.http_task.take(),
            self.socks_task.take(),
            self.admin_task.take(),
        )
        .await;
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
    admin_task: Option<JoinHandle<Result<()>>>,
) {
    abort_task(http_task).await;
    abort_task(socks_task).await;
    abort_task(admin_task).await;
}

impl Drop for NetworkProxyHandle {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let http_task = self.http_task.take();
        let socks_task = self.socks_task.take();
        let admin_task = self.admin_task.take();
        tokio::spawn(async move {
            abort_tasks(http_task, socks_task, admin_task).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkProxySettings;
    use crate::state::network_proxy_state_for_policy;
    use pretty_assertions::assert_eq;
    use std::net::IpAddr;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn managed_proxy_builder_uses_loopback_ephemeral_ports() {
        let state = Arc::new(network_proxy_state_for_policy(
            NetworkProxySettings::default(),
        ));
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
        assert!(proxy.admin_addr.ip().is_loopback());
        assert_ne!(proxy.http_addr.port(), 0);
        assert_ne!(proxy.socks_addr.port(), 0);
        assert_ne!(proxy.admin_addr.port(), 0);
    }

    #[tokio::test]
    async fn non_codex_managed_proxy_builder_uses_configured_ports() {
        let settings = NetworkProxySettings {
            proxy_url: "http://127.0.0.1:43128".to_string(),
            socks_url: "http://127.0.0.1:48081".to_string(),
            admin_url: "http://127.0.0.1:48080".to_string(),
            ..NetworkProxySettings::default()
        };
        let state = Arc::new(network_proxy_state_for_policy(settings));
        let proxy = NetworkProxy::builder()
            .state(state)
            .managed_by_codex(false)
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
        assert_eq!(
            proxy.admin_addr,
            "127.0.0.1:48080".parse::<SocketAddr>().unwrap()
        );
    }

    #[tokio::test]
    async fn managed_proxy_builder_does_not_reserve_socks_listener_when_disabled() {
        let settings = NetworkProxySettings {
            enable_socks5: false,
            socks_url: "http://127.0.0.1:43129".to_string(),
            ..NetworkProxySettings::default()
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
        assert!(proxy.admin_addr.ip().is_loopback());
        assert_eq!(
            proxy.socks_addr,
            "127.0.0.1:43129".parse::<SocketAddr>().unwrap()
        );
        assert!(
            proxy
                .reserved_listeners
                .as_ref()
                .expect("managed builder should reserve listeners")
                .take_socks()
                .is_none()
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
    fn apply_proxy_env_overrides_sets_common_tool_vars() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            true,
            false,
            None,
        );

        assert_eq!(
            env.get("HTTP_PROXY"),
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
        assert_eq!(
            env.get("NO_PROXY"),
            Some(&DEFAULT_NO_PROXY_VALUE.to_string())
        );
        assert_eq!(env.get(ALLOW_LOCAL_BINDING_ENV_KEY), Some(&"0".to_string()));
        assert_eq!(env.get("ELECTRON_GET_USE_PROXY"), Some(&"true".to_string()));
        #[cfg(target_os = "macos")]
        assert_eq!(
            env.get("GIT_SSH_COMMAND"),
            Some(&"ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'".to_string())
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(env.get("GIT_SSH_COMMAND"), None);
    }

    #[test]
    fn apply_proxy_env_overrides_uses_http_for_all_proxy_without_socks() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            false,
            true,
            None,
        );

        assert_eq!(
            env.get("ALL_PROXY"),
            Some(&"http://127.0.0.1:3128".to_string())
        );
        assert_eq!(env.get(ALLOW_LOCAL_BINDING_ENV_KEY), Some(&"1".to_string()));
    }

    #[test]
    fn apply_proxy_env_overrides_embeds_attempt_id_in_http_proxy_url() {
        let mut env = HashMap::new();
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            true,
            false,
            Some("attempt-123"),
        );

        assert_eq!(
            env.get("HTTP_PROXY"),
            Some(&"http://codex-net-attempt-attempt-123@127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("HTTPS_PROXY"),
            Some(&"http://codex-net-attempt-attempt-123@127.0.0.1:3128".to_string())
        );
        assert_eq!(
            env.get("ALL_PROXY"),
            Some(&"http://codex-net-attempt-attempt-123@127.0.0.1:3128".to_string())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apply_proxy_env_overrides_preserves_existing_git_ssh_command() {
        let mut env = HashMap::new();
        env.insert(
            "GIT_SSH_COMMAND".to_string(),
            "ssh -o ProxyCommand='tsh proxy ssh --cluster=dev %r@%h:%p'".to_string(),
        );
        apply_proxy_env_overrides(
            &mut env,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3128),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
            true,
            false,
            None,
        );

        assert_eq!(
            env.get("GIT_SSH_COMMAND"),
            Some(&"ssh -o ProxyCommand='tsh proxy ssh --cluster=dev %r@%h:%p'".to_string())
        );
    }
}
