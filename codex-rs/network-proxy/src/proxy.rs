use crate::admin;
use crate::config;
use crate::http_proxy;
use crate::network_policy::NetworkPolicyDecider;
use crate::runtime::unix_socket_permissions_supported;
use crate::socks5;
use crate::state::NetworkProxyState;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use tokio::task::JoinHandle;
use tracing::warn;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {}

const FALLBACK_EPHEMERAL_PORT_START: u16 = 40_000;
static FALLBACK_EPHEMERAL_PORT: AtomicU16 = AtomicU16::new(FALLBACK_EPHEMERAL_PORT_START);

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
        let (requested_http_addr, requested_socks_addr, requested_admin_addr) =
            if self.managed_by_codex {
                (
                    reserve_loopback_ephemeral_addr()?,
                    reserve_loopback_ephemeral_addr()?,
                    reserve_loopback_ephemeral_addr()?,
                )
            } else {
                let runtime = config::resolve_runtime(&current_cfg)?;
                (
                    self.http_addr.unwrap_or(runtime.http_addr),
                    self.socks_addr.unwrap_or(runtime.socks_addr),
                    self.admin_addr.unwrap_or(runtime.admin_addr),
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
            admin_addr,
            policy_decider: self.policy_decider,
        })
    }
}

fn reserve_loopback_ephemeral_addr() -> Result<SocketAddr> {
    match std::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) {
        Ok(listener) => listener
            .local_addr()
            .context("failed to get loopback ephemeral port"),
        Err(err) => {
            let port = next_fallback_ephemeral_port();
            warn!(
                "failed to reserve loopback ephemeral port ({err}); falling back to 127.0.0.1:{port}"
            );
            Ok(SocketAddr::from(([127, 0, 0, 1], port)))
        }
    }
}

fn next_fallback_ephemeral_port() -> u16 {
    // Keep ports nonzero and in a high range to reduce accidental collisions in test environments
    // where loopback binds are restricted and we cannot reserve an ephemeral port directly.
    let next = FALLBACK_EPHEMERAL_PORT.fetch_add(1, Ordering::Relaxed);
    if next == u16::MAX {
        FALLBACK_EPHEMERAL_PORT.store(FALLBACK_EPHEMERAL_PORT_START + 1, Ordering::Relaxed);
        FALLBACK_EPHEMERAL_PORT_START
    } else {
        next
    }
}

#[derive(Clone)]
pub struct NetworkProxy {
    state: Arc<NetworkProxyState>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    admin_addr: SocketAddr,
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
            && self.admin_addr == other.admin_addr
    }
}

impl Eq for NetworkProxy {}

impl NetworkProxy {
    pub fn builder() -> NetworkProxyBuilder {
        NetworkProxyBuilder::default()
    }

    pub fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        // Enforce proxying for all child processes when configured. We always override to ensure
        // the proxy is actually used even if the caller passed conflicting environment variables.
        let proxy_url = format!("http://{}", self.http_addr);
        for key in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
            env.insert(key.to_string(), proxy_url.clone());
        }
    }

    pub async fn run(&self) -> Result<NetworkProxyHandle> {
        let current_cfg = self.state.current_cfg().await?;
        if !current_cfg.network.enabled {
            warn!("network.enabled is false; skipping proxy listeners");
            return Ok(NetworkProxyHandle::noop());
        }

        if !unix_socket_permissions_supported() {
            warn!("allowUnixSockets is macOS-only; requests will be rejected on this platform");
        }

        let http_task = tokio::spawn(http_proxy::run_http_proxy(
            self.state.clone(),
            self.http_addr,
            self.policy_decider.clone(),
        ));
        let socks_task = if current_cfg.network.enable_socks5 {
            Some(tokio::spawn(socks5::run_socks5(
                self.state.clone(),
                self.socks_addr,
                self.policy_decider.clone(),
                current_cfg.network.enable_socks5_udp,
            )))
        } else {
            None
        };
        let admin_task = tokio::spawn(admin::run_admin_api(self.state.clone(), self.admin_addr));

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

    #[tokio::test]
    async fn managed_proxy_builder_uses_loopback_ephemeral_ports() {
        let state = Arc::new(network_proxy_state_for_policy(
            NetworkProxySettings::default(),
        ));
        let proxy = NetworkProxy::builder().state(state).build().await.unwrap();

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
}
