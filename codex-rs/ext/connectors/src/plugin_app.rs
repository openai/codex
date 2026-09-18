use codex_connectors::parse_plugin_app_config;
use codex_core_plugins::ResolvedExecutorPlugin;
use codex_file_system::ReadFileOptions;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginResourceLocator;
use codex_utils_path_uri::PathUri;
use std::io;
use thiserror::Error;

/// Loads app declarations from a resolved plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct PluginAppProvider;

/// Failure to load app declarations from a plugin.
#[derive(Debug, Error)]
pub enum PluginAppProviderError {
    #[error("failed to read app config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ReadConfig {
        plugin_id: String,
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse app config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ParseConfig {
        plugin_id: String,
        path: PathUri,
        #[source]
        source: serde_json::Error,
    },
}

impl PluginAppProvider {
    /// Returns the app declarations contributed by `plugin`.
    #[tracing::instrument(name = "connectors.plugin.declarations.load", skip_all)]
    pub async fn load(
        &self,
        plugin: &ResolvedExecutorPlugin,
    ) -> Result<Vec<AppDeclaration>, PluginAppProviderError> {
        let resolved_plugin = plugin.plugin();
        let plugin_id = resolved_plugin.selected_root_id();
        let Some(PluginResourceLocator::Environment {
            path: config_path, ..
        }) = resolved_plugin.manifest().paths.apps.as_ref()
        else {
            return Ok(Vec::new());
        };
        let contents = plugin
            .file_system()
            .read_file_text(
                config_path,
                ReadFileOptions::default(),
                /*sandbox*/ None,
            )
            .await
            .map_err(|source| PluginAppProviderError::ReadConfig {
                plugin_id: plugin_id.to_string(),
                path: config_path.clone(),
                source,
            })?;

        parse_plugin_app_config(&contents).map_err(|source| PluginAppProviderError::ParseConfig {
            plugin_id: plugin_id.to_string(),
            path: config_path.clone(),
            source,
        })
    }
}
