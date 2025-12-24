mod admin;
mod config;
mod http_proxy;
mod init;
mod mitm;
mod policy;
mod responses;
mod socks5;
mod state;

use crate::state::AppState;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::warn;

#[derive(Debug, Clone, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network sandbox proxy")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Initialize the Codex network proxy directories (e.g. MITM cert paths).
    Init,
}

pub async fn run_main(args: Args) -> Result<()> {
    tracing_subscriber::fmt::init();

    if let Some(Command::Init) = args.command {
        init::run_init()?;
        return Ok(());
    }

    if cfg!(not(target_os = "macos")) {
        warn!("allowUnixSockets is macOS-only; requests will be rejected on this platform");
    }

    let state = Arc::new(AppState::new().await?);
    let runtime = config::resolve_runtime(&state.current_cfg().await?);

    let http_addr: SocketAddr = runtime.http_addr;
    let socks_addr: SocketAddr = runtime.socks_addr;
    let admin_addr: SocketAddr = runtime.admin_addr;

    let http_task = http_proxy::run_http_proxy(state.clone(), http_addr);
    let socks_task = socks5::run_socks5(state.clone(), socks_addr);
    let admin_task = admin::run_admin_api(state.clone(), admin_addr);

    tokio::try_join!(http_task, socks_task, admin_task)?;
    Ok(())
}
