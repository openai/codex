use codex_connectors::parse_plugin_app_config;
use codex_core_plugins::ResolvedExecutorPlugin;
use codex_exec_server::ExecutorFileSystem;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginResourceLocator;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginLocation;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
use std::io;
use thiserror::Error;

const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";

/// Loads connector declarations from a resolved plugin through its owning executor.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecutorPluginConnectorProvider;

/// Failure to load connector declarations from an executor plugin.
#[derive(Debug, Error)]
pub enum ExecutorPluginConnectorProviderError {
    #[error(
        "failed to resolve app config path `{relative_path}` below selected plugin `{plugin_id}` at `{root}`: {source}"
    )]
    InvalidConfigPath {
        plugin_id: String,
        root: PathUri,
        relative_path: &'static str,
        #[source]
        source: PathUriParseError,
    },
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

impl ExecutorPluginConnectorProvider {
    /// Returns the connector declarations contributed by `plugin`.
    pub async fn load(
        &self,
        plugin: &ResolvedExecutorPlugin,
    ) -> Result<Vec<AppDeclaration>, ExecutorPluginConnectorProviderError> {
        let ResolvedPluginLocation::Environment { root, .. } = plugin.plugin().location();

        load_from_file_system(plugin.plugin(), root, plugin.file_system()).await
    }
}

async fn load_from_file_system(
    plugin: &ResolvedPlugin,
    plugin_root: &PathUri,
    file_system: &dyn ExecutorFileSystem,
) -> Result<Vec<AppDeclaration>, ExecutorPluginConnectorProviderError> {
    let plugin_id = plugin.selected_root_id();
    let config_path = match plugin.manifest().paths.apps.as_ref() {
        Some(PluginResourceLocator::Environment { path, .. }) => path.clone(),
        None => plugin_root
            .join(DEFAULT_APP_CONFIG_FILE)
            .map_err(
                |source| ExecutorPluginConnectorProviderError::InvalidConfigPath {
                    plugin_id: plugin_id.to_string(),
                    root: plugin_root.clone(),
                    relative_path: DEFAULT_APP_CONFIG_FILE,
                    source,
                },
            )?,
    };
    let contents = match file_system
        .read_file_text(&config_path, /*sandbox*/ None)
        .await
    {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(source) => {
            return Err(ExecutorPluginConnectorProviderError::ReadConfig {
                plugin_id: plugin_id.to_string(),
                path: config_path,
                source,
            });
        }
    };
    let mut declarations = parse_plugin_app_config(&contents).map_err(|source| {
        ExecutorPluginConnectorProviderError::ParseConfig {
            plugin_id: plugin_id.to_string(),
            path: config_path,
            source,
        }
    })?;
    let declaration_count = declarations.len();
    declarations.retain(|declaration| !declaration.connector_id.0.trim().is_empty());
    let ignored_declaration_count = declaration_count - declarations.len();
    if ignored_declaration_count > 0 {
        tracing::warn!(
            plugin = plugin_id,
            ignored_declaration_count,
            "ignoring executor plugin app declarations without connector IDs"
        );
    }

    Ok(declarations)
}

#[cfg(test)]
#[path = "executor_plugin_tests.rs"]
mod tests;
