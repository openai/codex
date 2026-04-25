use crate::config::find_codex_home;
use crate::config::resolve_permission_profile;
use crate::exec_policy::ExecPolicyError;
use crate::exec_policy::format_exec_policy_error_with_source;
use crate::exec_policy::load_exec_policy;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::CONFIG_TOML_FILE;
use codex_config::CloudRequirementsLoader;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::LoaderOverrides;
use codex_config::loader::load_config_layers_state;
use codex_config::permissions_toml::NetworkToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_config::permissions_toml::overlay_network_domain_permissions;
use codex_exec_server::LOCAL_FS;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraintError;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use codex_network_proxy::normalize_host;
use codex_network_proxy::validate_policy_against_constraints;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
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
        LOCAL_FS.as_ref(),
        &codex_home,
        /*cwd*/ None,
        &cli_overrides,
        overrides,
        CloudRequirementsLoader::default(),
        &codex_config::NoopThreadConfigLoader,
        /*host_name*/ None,
    )
    .await
    .context("failed to load Codex config")?;

    let (exec_policy, warning) = match load_exec_policy(&config_layer_stack).await {
        Ok(policy) => (policy, None),
        Err(err @ ExecPolicyError::ParsePolicy { .. }) => {
            (codex_execpolicy::Policy::empty(), Some(err))
        }
        Err(err) => return Err(err.into()),
    };
    if let Some(err) = warning.as_ref() {
        tracing::warn!(
            "failed to parse execpolicy while building network proxy state: {}",
            format_exec_policy_error_with_source(err)
        );
    }

    let config = config_from_layers(&config_layer_stack, &exec_policy)?;

    let constraints = enforce_trusted_constraints(&config_layer_stack, &config)?;
    let layer_mtimes = collect_layer_mtimes(&config_layer_stack);
    let state = build_config_state(config, constraints)?;
    Ok((state, layer_mtimes))
}

fn collect_layer_mtimes(stack: &ConfigLayerStack) -> Vec<LayerMtime> {
    stack
        .get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        )
        .iter()
        .filter_map(|layer| {
            let path = match &layer.name {
                ConfigLayerSource::System { file } => Some(file.clone()),
                ConfigLayerSource::User { file } => Some(file.clone()),
                ConfigLayerSource::Project { dot_codex_folder } => {
                    Some(dot_codex_folder.join(CONFIG_TOML_FILE))
                }
                ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => Some(file.clone()),
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
    for layer in layers.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        if is_user_controlled_layer(&layer.name) {
            continue;
        }

        let parsed = network_tables_from_toml(&layer.config)?;
        if let Some(network) = selected_network_from_tables(parsed)? {
            apply_network_constraints(network, &mut constraints);
        }
    }
    Ok(constraints)
}

fn apply_network_constraints(network: NetworkToml, constraints: &mut NetworkProxyConstraints) {
    if let Some(enabled) = network.enabled {
        constraints.enabled = Some(enabled);
    }
    if let Some(mode) = network.mode {
        constraints.mode = Some(mode);
    }
    if let Some(allow_upstream_proxy) = network.allow_upstream_proxy {
        constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
    }
    if let Some(dangerously_allow_non_loopback_proxy) = network.dangerously_allow_non_loopback_proxy
    {
        constraints.dangerously_allow_non_loopback_proxy =
            Some(dangerously_allow_non_loopback_proxy);
    }
    if let Some(dangerously_allow_all_unix_sockets) = network.dangerously_allow_all_unix_sockets {
        constraints.dangerously_allow_all_unix_sockets = Some(dangerously_allow_all_unix_sockets);
    }
    if let Some(domains) = network.domains.as_ref() {
        let mut config = NetworkProxyConfig::default();
        if let Some(allowed_domains) = constraints.allowed_domains.take() {
            config.network.set_allowed_domains(allowed_domains);
        }
        if let Some(denied_domains) = constraints.denied_domains.take() {
            config.network.set_denied_domains(denied_domains);
        }
        overlay_network_domain_permissions(&mut config, domains);
        constraints.allowed_domains = config.network.allowed_domains();
        constraints.denied_domains = config.network.denied_domains();
    }
    if let Some(unix_sockets) = network.unix_sockets.as_ref() {
        let allow_unix_sockets = unix_sockets.allow_unix_sockets();
        constraints.allow_unix_sockets = Some(allow_unix_sockets);
    }
    if let Some(allow_local_binding) = network.allow_local_binding {
        constraints.allow_local_binding = Some(allow_local_binding);
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NetworkTablesToml {
    default_permissions: Option<String>,
    permissions: Option<PermissionsToml>,
}

fn network_tables_from_toml(value: &toml::Value) -> Result<NetworkTablesToml> {
    value
        .clone()
        .try_into()
        .context("failed to deserialize network tables from config")
}

fn selected_network_from_tables(parsed: NetworkTablesToml) -> Result<Option<NetworkToml>> {
    let Some(default_permissions) = parsed.default_permissions else {
        return Ok(None);
    };

    let permissions = parsed
        .permissions
        .context("default_permissions requires a `[permissions]` table for network settings")?;
    let profile = resolve_permission_profile(&permissions, &default_permissions)
        .map_err(anyhow::Error::from)?;
    Ok(profile.network.clone())
}

fn apply_network_tables(config: &mut NetworkProxyConfig, parsed: NetworkTablesToml) -> Result<()> {
    if let Some(network) = selected_network_from_tables(parsed)? {
        network.apply_to_network_proxy_config(config);
    }
    Ok(())
}

fn config_from_layers(
    layers: &ConfigLayerStack,
    exec_policy: &codex_execpolicy::Policy,
) -> Result<NetworkProxyConfig> {
    let mut config = NetworkProxyConfig::default();
    for layer in layers.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        let parsed = network_tables_from_toml(&layer.config)?;
        apply_network_tables(&mut config, parsed)?;
    }
    apply_exec_policy_network_rules(&mut config, exec_policy);
    Ok(config)
}

fn apply_exec_policy_network_rules(
    config: &mut NetworkProxyConfig,
    exec_policy: &codex_execpolicy::Policy,
) {
    let (allowed_domains, denied_domains) = exec_policy.compiled_network_domains();
    for host in allowed_domains {
        upsert_network_domain(
            config,
            host,
            codex_network_proxy::NetworkDomainPermission::Allow,
        );
    }
    for host in denied_domains {
        upsert_network_domain(
            config,
            host,
            codex_network_proxy::NetworkDomainPermission::Deny,
        );
    }
}

fn upsert_network_domain(
    config: &mut NetworkProxyConfig,
    host: String,
    permission: codex_network_proxy::NetworkDomainPermission,
) {
    config
        .network
        .upsert_domain_permission(host, permission, normalize_host);
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
    path: AbsolutePathBuf,
    mtime: Option<std::time::SystemTime>,
}

impl LayerMtime {
    fn new(path: AbsolutePathBuf) -> Self {
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

#[cfg(test)]
#[path = "network_proxy_loader_tests.rs"]
mod tests;
