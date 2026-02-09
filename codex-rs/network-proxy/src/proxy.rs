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
use tokio::task::JoinHandle;
use tracing::warn;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {}

#[derive(Clone, Default)]
pub struct NetworkProxyBuilder {
    state: Option<Arc<NetworkProxyState>>,
    http_addr: Option<SocketAddr>,
    admin_addr: Option<SocketAddr>,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
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

    pub fn admin_addr(mut self, addr: SocketAddr) -> Self {
        self.admin_addr = Some(addr);
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
        let runtime = config::resolve_runtime(&current_cfg)?;
        // Reapply bind clamping for caller overrides so unix-socket proxying stays loopback-only.
        let (http_addr, socks_addr, admin_addr) = config::clamp_bind_addrs(
            self.http_addr.unwrap_or(runtime.http_addr),
            runtime.socks_addr,
            self.admin_addr.unwrap_or(runtime.admin_addr),
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
