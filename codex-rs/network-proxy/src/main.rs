mod standalone_otel;

use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use codex_network_proxy::Args;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use codex_otel::otel_provider::OtelProvider;
use codex_utils_home_dir::find_codex_home;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const CONFIG_TOML_FILE: &str = "config.toml";

struct StandaloneConfig {
    codex_home: PathBuf,
    config_path: PathBuf,
    config_missing: bool,
    network: NetworkProxyConfig,
    otel: standalone_otel::StandaloneOtelConfigToml,
}

#[derive(Debug, Deserialize, Default)]
struct StandaloneConfigToml {
    #[serde(flatten)]
    network: NetworkProxyConfig,
    #[serde(default)]
    otel: standalone_otel::StandaloneOtelConfigToml,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = Args::parse();
    let config = load_standalone_config().await?;
    let StandaloneConfig {
        codex_home,
        config_path,
        config_missing,
        network,
        otel: otel_config,
    } = config;

    let otel_provider = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        standalone_otel::build_provider(otel_config, codex_home, env!("CARGO_PKG_VERSION"))
    })) {
        Ok(Ok(otel)) => otel,
        Ok(Err(err)) => {
            eprintln!("Could not create otel exporter: {err}");
            None
        }
        Err(_) => {
            eprintln!("Could not create otel exporter: panicked during initialization");
            None
        }
    };
    init_tracing(otel_provider.as_ref());

    if config_missing {
        warn!(
            "config file not found at {}; using defaults",
            config_path.display()
        );
    }

    let state = build_config_state(network, NetworkProxyConstraints::default())?;
    let state = NetworkProxyState::with_reloader(state, Arc::new(StandaloneConfigReloader));
    let proxy = NetworkProxy::builder()
        .state(Arc::new(state))
        .managed_by_codex(false)
        .build()
        .await?;
    let handle = proxy.run().await?;

    info!(
        http = %proxy.http_addr(),
        socks = %proxy.socks_addr(),
        admin = %proxy.admin_addr(),
        "network proxy started"
    );

    tokio::select! {
        result = handle.wait() => result,
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}

fn init_tracing(otel: Option<&OtelProvider>) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr_fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);
    let otel_logger_layer = otel.and_then(|provider| provider.logger_layer());
    let otel_tracing_layer = otel.and_then(|provider| provider.tracing_layer());

    let _ = tracing_subscriber::registry()
        .with(stderr_fmt)
        .with(otel_logger_layer)
        .with(otel_tracing_layer)
        .try_init();
}

async fn load_standalone_config() -> Result<StandaloneConfig> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let raw = match tokio::fs::read_to_string(&config_path).await {
        Ok(raw) => Some(raw),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", config_path.display()));
        }
    };

    let (network, otel, config_missing) = match raw {
        Some(raw) => {
            let parsed: StandaloneConfigToml = toml::from_str(&raw)
                .with_context(|| format!("failed to parse {}", config_path.display()))?;
            (parsed.network, parsed.otel, false)
        }
        None => (
            NetworkProxyConfig::default(),
            standalone_otel::StandaloneOtelConfigToml::default(),
            true,
        ),
    };

    Ok(StandaloneConfig {
        codex_home,
        config_path,
        config_missing,
        network,
        otel,
    })
}

struct StandaloneConfigReloader;

#[async_trait]
impl ConfigReloader for StandaloneConfigReloader {
    fn source_label(&self) -> String {
        "standalone config state".to_string()
    }

    async fn maybe_reload(&self) -> Result<Option<ConfigState>> {
        Ok(None)
    }

    async fn reload_now(&self) -> Result<ConfigState> {
        Err(anyhow::anyhow!(
            "config reload is not supported in standalone mode"
        ))
    }
}
