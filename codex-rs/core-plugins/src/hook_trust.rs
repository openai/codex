use crate::loader::load_plugin_hooks;
use crate::manifest::load_plugin_manifest;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use codex_config::ConfigLayerStack;
use codex_hooks::PluginHookTrustEntry;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PluginHookTrustError {
    #[error("installed plugin `{plugin_id}` has a missing or invalid plugin.json")]
    InvalidManifest { plugin_id: String },

    #[error("failed to load hooks for installed plugin `{plugin_id}`: {warnings}")]
    HookLoad { plugin_id: String, warnings: String },

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("failed to persist automatic plugin hook trust: {0}")]
    Persist(#[from] std::io::Error),
}

/// Resolve canonical trust identities from a fully materialized plugin root.
pub fn installed_plugin_hook_trust_entries(
    codex_home: &Path,
    plugin_id: &PluginId,
    installed_path: &AbsolutePathBuf,
) -> Result<Vec<PluginHookTrustEntry>, PluginHookTrustError> {
    let manifest = load_plugin_manifest(installed_path.as_path()).ok_or_else(|| {
        PluginHookTrustError::InvalidManifest {
            plugin_id: plugin_id.as_key(),
        }
    })?;
    let store = PluginStore::try_new(codex_home.to_path_buf())?;
    let (hook_sources, warnings) = load_plugin_hooks(
        installed_path,
        plugin_id,
        &store.plugin_data_root(plugin_id),
        &manifest.paths,
    );
    if !warnings.is_empty() {
        return Err(PluginHookTrustError::HookLoad {
            plugin_id: plugin_id.as_key(),
            warnings: warnings.join("; "),
        });
    }
    Ok(codex_hooks::plugin_hook_trust_entries(&hook_sources))
}

/// True when every supported command hook already has durable user trust.
pub fn installed_plugin_hook_trust_is_current(
    config_layer_stack: &ConfigLayerStack,
    entries: &[PluginHookTrustEntry],
) -> bool {
    let states = codex_hooks::persisted_user_hook_states_from_stack(Some(config_layer_stack));
    entries.iter().all(|entry| {
        states
            .get(&entry.key)
            .and_then(|state| state.trusted_hash.as_deref())
            == Some(entry.current_hash.as_str())
    })
}

/// Perform the ordinary user-level trust write for installed plugin hooks.
pub async fn trust_installed_plugin_hooks(
    codex_home: &Path,
    config_layer_stack: &ConfigLayerStack,
    plugin_id: &PluginId,
    installed_path: &AbsolutePathBuf,
) -> Result<Vec<PluginHookTrustEntry>, PluginHookTrustError> {
    let entries = installed_plugin_hook_trust_entries(codex_home, plugin_id, installed_path)?;
    codex_config::upsert_hook_trusted_hashes(
        selected_user_config_path(codex_home, config_layer_stack).as_path(),
        entries
            .iter()
            .map(|entry| (entry.key.clone(), entry.current_hash.clone()))
            .collect(),
    )
    .await?;
    Ok(entries)
}

pub(crate) fn selected_user_config_path(
    codex_home: &Path,
    config_layer_stack: &ConfigLayerStack,
) -> std::path::PathBuf {
    config_layer_stack
        .get_user_config_file()
        .map(AbsolutePathBuf::to_path_buf)
        .unwrap_or_else(|| codex_home.join(codex_config::CONFIG_TOML_FILE))
}
