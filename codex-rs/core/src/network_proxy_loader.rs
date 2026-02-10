use crate::config::CONFIG_TOML_FILE;
use crate::config::find_codex_home;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use codex_app_server_protocol::ConfigLayerSource;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraintError;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::PartialNetworkProxyConfig;
use codex_network_proxy::build_config_state;
use codex_network_proxy::validate_policy_against_constraints;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn build_network_proxy_state() -> Result<NetworkProxyState> {
    let (state, reloader) = build_network_proxy_state_and_reloader().await?;
    Ok(NetworkProxyState::with_reloader(state, Arc::new(reloader)))
}

pub async fn build_network_proxy_state_and_reloader() -> Result<(ConfigState, MtimeConfigReloader)>
{
    let (state, layer_mtimes) = build_config_state_with_mtimes().await?;
    Ok((state, MtimeConfigReloader::new(layer_mtimes)))
}

async fn build_config_state_with_mtimes() -> Result<(ConfigState, Vec<LayerMtime>)> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let cli_overrides = Vec::new();
    let overrides = LoaderOverrides::default();
    let config_layer_stack = load_config_layers_state(
        &codex_home,
        None,
        &cli_overrides,
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await
    .context("failed to load Codex config")?;

    let merged_toml = config_layer_stack.effective_config();
    let config: NetworkProxyConfig = merged_toml
        .try_into()
        .context("failed to deserialize network proxy config")?;

    let constraints = enforce_trusted_constraints(&config_layer_stack, &config)?;
    let layer_mtimes = collect_layer_mtimes(&config_layer_stack);
    let state = build_config_state(config, constraints)?;
    Ok((state, layer_mtimes))
}

fn collect_layer_mtimes(stack: &ConfigLayerStack) -> Vec<LayerMtime> {
    stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, false)
        .iter()
        .filter_map(|layer| {
            let path = match &layer.name {
                ConfigLayerSource::System { file } => Some(file.as_path().to_path_buf()),
                ConfigLayerSource::User { file } => Some(file.as_path().to_path_buf()),
                ConfigLayerSource::Project { dot_codex_folder } => dot_codex_folder
                    .join(CONFIG_TOML_FILE)
                    .ok()
                    .map(|p| p.as_path().to_path_buf()),
                ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
                    Some(file.as_path().to_path_buf())
                }
                _ => None,
            };
            path.map(LayerMtime::new)
        })
        .collect()
}

fn enforce_trusted_constraints(
    layers: &ConfigLayerStack,
    config: &NetworkProxyConfig,
) -> Result<NetworkProxyConstraints> {
    let constraints = network_constraints_from_trusted_layers(layers)?;
    validate_policy_against_constraints(config, &constraints)
        .map_err(NetworkProxyConstraintError::into_anyhow)
        .context("network proxy constraints")?;
    Ok(constraints)
}

fn network_constraints_from_trusted_layers(
    layers: &ConfigLayerStack,
) -> Result<NetworkProxyConstraints> {
    let mut constraints = NetworkProxyConstraints::default();
    for layer in layers.get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, false) {
        if is_user_controlled_layer(&layer.name) {
            continue;
        }

        let partial: PartialNetworkProxyConfig = layer
            .config
            .clone()
            .try_into()
            .context("failed to deserialize trusted config layer")?;

        if let Some(enabled) = partial.network.enabled {
            constraints.enabled = Some(enabled);
        }
        if let Some(mode) = partial.network.mode {
            constraints.mode = Some(mode);
        }
        if let Some(allow_upstream_proxy) = partial.network.allow_upstream_proxy {
            constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            partial.network.dangerously_allow_non_loopback_proxy
        {
            constraints.dangerously_allow_non_loopback_proxy =
                Some(dangerously_allow_non_loopback_proxy);
        }
        if let Some(dangerously_allow_non_loopback_admin) =
            partial.network.dangerously_allow_non_loopback_admin
        {
            constraints.dangerously_allow_non_loopback_admin =
                Some(dangerously_allow_non_loopback_admin);
        }

        if let Some(allowed_domains) = partial.network.allowed_domains {
            constraints.allowed_domains = Some(allowed_domains);
        }
        if let Some(denied_domains) = partial.network.denied_domains {
            constraints.denied_domains = Some(denied_domains);
        }
        if let Some(allow_unix_sockets) = partial.network.allow_unix_sockets {
            constraints.allow_unix_sockets = Some(allow_unix_sockets);
        }
        if let Some(allow_local_binding) = partial.network.allow_local_binding {
            constraints.allow_local_binding = Some(allow_local_binding);
        }
    }
    Ok(constraints)
}

fn is_user_controlled_layer(layer: &ConfigLayerSource) -> bool {
    matches!(
        layer,
        ConfigLayerSource::User { .. }
            | ConfigLayerSource::Project { .. }
            | ConfigLayerSource::SessionFlags
    )
}

#[derive(Clone)]
struct LayerMtime {
    path: PathBuf,
    mtime: Option<std::time::SystemTime>,
}

impl LayerMtime {
    fn new(path: PathBuf) -> Self {
        let mtime = path.metadata().and_then(|m| m.modified()).ok();
        Self { path, mtime }
    }
}

pub struct MtimeConfigReloader {
    layer_mtimes: RwLock<Vec<LayerMtime>>,
}

impl MtimeConfigReloader {
    fn new(layer_mtimes: Vec<LayerMtime>) -> Self {
        Self {
            layer_mtimes: RwLock::new(layer_mtimes),
        }
    }

    async fn needs_reload(&self) -> bool {
        let guard = self.layer_mtimes.read().await;
        guard.iter().any(|layer| {
            let metadata = std::fs::metadata(&layer.path).ok();
            match (metadata.and_then(|m| m.modified().ok()), layer.mtime) {
                (Some(new_mtime), Some(old_mtime)) => new_mtime > old_mtime,
                (Some(_), None) => true,
                (None, Some(_)) => true,
                (None, None) => false,
            }
        })
    }
}

#[async_trait]
impl ConfigReloader for MtimeConfigReloader {
    fn source_label(&self) -> String {
        "config layers".to_string()
    }

    async fn maybe_reload(&self) -> Result<Option<ConfigState>> {
        if !self.needs_reload().await {
            return Ok(None);
        }

        let (state, layer_mtimes) = build_config_state_with_mtimes().await?;
        let mut guard = self.layer_mtimes.write().await;
        *guard = layer_mtimes;
        Ok(Some(state))
    }

    async fn reload_now(&self) -> Result<ConfigState> {
        let (state, layer_mtimes) = build_config_state_with_mtimes().await?;
        let mut guard = self.layer_mtimes.write().await;
        *guard = layer_mtimes;
        Ok(state)
    }
}
