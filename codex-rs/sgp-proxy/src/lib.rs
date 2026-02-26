use std::fs;
use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use tokio::net::TcpListener;

pub mod agentex_client;
pub mod config;
pub mod error;
mod read_api_key;
pub mod server;
pub mod sse_writer;
pub mod state;
pub mod tool_routing;
pub mod translate;

pub use config::Args;
use read_api_key::read_auth_header_from_stdin;

#[derive(Serialize)]
struct ServerInfo {
    port: u16,
    pid: u32,
}

/// Main entry point for the proxy library.
pub async fn run_main(args: Args) -> Result<()> {
    let auth_header = read_auth_header_from_stdin()?;

    let proxy_state = state::ProxyState::new(&args, auth_header);
    let router = server::build_router(proxy_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port.unwrap_or(0)));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    let bound = listener
        .local_addr()
        .context("failed to read local_addr")?;

    if let Some(path) = args.server_info.as_ref() {
        write_server_info(path, bound.port())?;
    }

    eprintln!("codex-sgp-proxy listening on {bound}");

    axum::serve(listener, router.into_make_service())
        .await
        .context("axum server error")?;

    Ok(())
}

fn write_server_info(path: &Path, port: u16) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let info = ServerInfo {
        port,
        pid: std::process::id(),
    };
    let mut data = serde_json::to_string(&info)?;
    data.push('\n');
    let mut f = File::create(path)?;
    f.write_all(data.as_bytes())?;
    Ok(())
}
