#![deny(clippy::print_stdout, clippy::print_stderr)]

use anyhow::Context as _;
use anyhow::Result;
use anyhow::ensure;
use clap::Parser;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigReloaderFuture;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::Platform;
use codex_network_proxy::StaticNetworkProxyConfig;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

const MAX_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "codex-network-proxy", about = "Codex network policy proxy")]
struct Args {
    /// Standalone JSON configuration containing a `network` object.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Reject unknown config fields and require explicit managed constraints.
    #[arg(long)]
    strict: bool,
}

impl Args {
    fn parse_config(&self, bytes: &[u8]) -> Result<StaticNetworkProxyConfig> {
        if self.strict {
            StaticNetworkProxyConfig::from_slice_strict(bytes)
        } else {
            StaticNetworkProxyConfig::from_slice(bytes)
        }
    }
}

struct StaticConfigReloader {
    state: ConfigState,
}

impl ConfigReloader for StaticConfigReloader {
    fn source_label(&self) -> String {
        "standalone network proxy config".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async { Ok(self.state.clone()) })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    let mut bytes = Vec::new();
    std::fs::File::open(&args.config)
        .with_context(|| {
            format!(
                "failed to read network proxy config: {}",
                args.config.display()
            )
        })?
        .take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_CONFIG_BYTES,
        "network proxy config exceeds {MAX_CONFIG_BYTES} bytes"
    );
    let config = args
        .parse_config(&bytes)
        .with_context(|| format!("invalid network proxy config: {}", args.config.display()))?;
    let config_state = config
        .build_state(Platform::native())
        .context("failed to initialize network proxy policy")?;
    let reloader = Arc::new(StaticConfigReloader {
        state: config_state.clone(),
    });
    let state = Arc::new(NetworkProxyState::with_reloader(config_state, reloader));
    NetworkProxy::builder()
        .state(state)
        .managed_by_codex(/*managed_by_codex*/ false)
        .build()
        .await?
        .run()
        .await?
        .wait()
        .await
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
