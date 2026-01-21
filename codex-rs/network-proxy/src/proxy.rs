use crate::admin;
use crate::config;
use crate::http_proxy;
use crate::init;
use crate::network_policy::NetworkPolicyDecider;
use crate::socks5;
use crate::state::AppState;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::warn;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Enable SOCKS5 UDP associate support (default: disabled).
    #[arg(long, default_value_t = false)]
    pub enable_socks5_udp: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Initialize the Codex network proxy directories (e.g. MITM cert paths).
    Init,
}

#[derive(Clone, Default)]
pub struct NetworkProxyBuilder {
    state: Option<Arc<AppState>>,
    http_addr: Option<SocketAddr>,
    socks_addr: Option<SocketAddr>,
    admin_addr: Option<SocketAddr>,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
}

impl NetworkProxyBuilder {
    #[must_use]
    pub fn state(mut self, state: Arc<AppState>) -> Self {
        self.state = Some(state);
        self
    }

    #[must_use]
    pub fn http_addr(mut self, addr: SocketAddr) -> Self {
        self.http_addr = Some(addr);
        self
    }

    #[must_use]
    pub fn socks_addr(mut self, addr: SocketAddr) -> Self {
        self.socks_addr = Some(addr);
        self
    }

    #[must_use]
    pub fn admin_addr(mut self, addr: SocketAddr) -> Self {
        self.admin_addr = Some(addr);
        self
    }

    #[must_use]
    pub fn policy_decider<D>(mut self, decider: D) -> Self
    where
        D: NetworkPolicyDecider,
    {
        self.policy_decider = Some(Arc::new(decider));
        self
    }

    #[must_use]
    pub fn policy_decider_arc(mut self, decider: Arc<dyn NetworkPolicyDecider>) -> Self {
        self.policy_decider = Some(decider);
        self
    }

    #[must_use]
    pub fn enable_socks5_udp(mut self, enabled: bool) -> Self {
        self.enable_socks5_udp = enabled;
        self
    }

    pub async fn build(self) -> Result<NetworkProxy> {
        let state = match self.state {
            Some(state) => state,
            None => Arc::new(AppState::new().await?),
        };
        let runtime = config::resolve_runtime(&state.current_cfg().await?);
        let current_cfg = state.current_cfg().await?;
        // Reapply bind clamping for caller overrides so unix-socket proxying stays loopback-only.
        let (http_addr, admin_addr) = config::clamp_bind_addrs(
            self.http_addr.unwrap_or(runtime.http_addr),
            self.admin_addr.unwrap_or(runtime.admin_addr),
            &current_cfg.network_proxy,
        );

        Ok(NetworkProxy {
            state,
            http_addr,
            socks_addr: self.socks_addr.unwrap_or(runtime.socks_addr),
            admin_addr,
            policy_decider: self.policy_decider,
            enable_socks5_udp: self.enable_socks5_udp,
        })
    }
}

#[derive(Clone)]
pub struct NetworkProxy {
    state: Arc<AppState>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    admin_addr: SocketAddr,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
}

impl NetworkProxy {
    #[must_use]
    pub fn builder() -> NetworkProxyBuilder {
        NetworkProxyBuilder::default()
    }

    pub async fn from_cli_args(args: Args) -> Result<Self> {
        let mut builder = Self::builder();
        builder = builder.enable_socks5_udp(args.enable_socks5_udp);
        builder.build().await
    }

    pub async fn run(&self) -> Result<NetworkProxyHandle> {
        let current_cfg = self.state.current_cfg().await?;
        if !current_cfg.network_proxy.enabled {
            warn!("network_proxy.enabled is false; skipping proxy listeners");
            return Ok(NetworkProxyHandle::noop());
        }

        if cfg!(not(target_os = "macos")) {
            warn!("allowUnixSockets is macOS-only; requests will be rejected on this platform");
        }

        let http_task = tokio::spawn(http_proxy::run_http_proxy(
            self.state.clone(),
            self.http_addr,
            self.policy_decider.clone(),
        ));
        let socks_task = tokio::spawn(socks5::run_socks5(
            self.state.clone(),
            self.socks_addr,
            self.policy_decider.clone(),
            self.enable_socks5_udp,
        ));
        let admin_task = tokio::spawn(admin::run_admin_api(self.state.clone(), self.admin_addr));

        Ok(NetworkProxyHandle {
            http_task,
            socks_task,
            admin_task,
        })
    }
}

pub struct NetworkProxyHandle {
    http_task: JoinHandle<Result<()>>,
    socks_task: JoinHandle<Result<()>>,
    admin_task: JoinHandle<Result<()>>,
}

impl NetworkProxyHandle {
    fn noop() -> Self {
        Self {
            http_task: tokio::spawn(async { Ok(()) }),
            socks_task: tokio::spawn(async { Ok(()) }),
            admin_task: tokio::spawn(async { Ok(()) }),
        }
    }

    pub async fn wait(self) -> Result<()> {
        self.http_task.await??;
        self.socks_task.await??;
        self.admin_task.await??;
        Ok(())
    }

    pub async fn shutdown(self) -> Result<()> {
        self.http_task.abort();
        self.socks_task.abort();
        self.admin_task.abort();
        let _ = self.http_task.await;
        let _ = self.socks_task.await;
        let _ = self.admin_task.await;
        Ok(())
    }
}

pub fn run_init() -> Result<()> {
    init::run_init()
}
