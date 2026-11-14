//! Refreshes cloud plugins explicitly or when their accepted auth/resource generation is stale.
//! Partial catalogs and warnings remain usable and retry with exponential backoff.
//! Complete and partial results replace the catalog; failed attempts retain same-auth fallback.
//! Core serializes discovery and step snapshots with its per-thread catalog lock.

use crate::PluginListQuery;
use crate::PluginsThreadState;
use crate::plugin_contributor::PluginContributor;
use codex_core::config::Config;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginIdentity;
use codex_extension_api::CatalogRefreshBackoff;
use codex_extension_api::CatalogRefreshInput as StepCatalogRefreshInput;
use codex_extension_api::CloudCatalogRefreshInput;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use std::collections::HashSet;

const MAX_PROJECTED_CLOUD_PLUGINS: usize = 100;
pub(crate) const MAX_PLUGIN_DISPLAY_NAME_CHARS: usize = 64;
pub(crate) const MAX_CONNECTOR_IDS_PER_PLUGIN: usize = 64;

struct CloudPluginRefreshError {
    error: String,
    catalog_cleared: bool,
    /// Cooldown for the step caller to store after a current-generation failure.
    /// Superseded attempts return `None`, allowing the next step to retry immediately.
    retry_backoff: Option<CatalogRefreshBackoff>,
}

fn cloud_catalog_query(
    input: CloudCatalogRefreshInput<'_>,
    turn_id: Option<&str>,
) -> PluginListQuery {
    PluginListQuery {
        thread_id: input.thread_store.level_id().to_string(),
        turn_id: turn_id.map(str::to_owned),
        mcp_resources: input.session_store.get::<McpResourceClient>(),
        snapshots: Vec::new(),
    }
}

impl PluginContributor {
    /// Returns whether publishing or clearing a catalog requires MCP reprojection.
    async fn discover_cloud_catalog(
        &self,
        state: &PluginsThreadState,
        query: PluginListQuery,
    ) -> Result<bool, CloudPluginRefreshError> {
        let Some(provider) = &self.providers.cloud else {
            return Ok(false);
        };
        let (auth_cache_key, resource_cache_key, catalog_cleared) = {
            let mut state = state.contributor_state();
            if !state.plugins_enabled {
                return Ok(false);
            }
            let resource_cache_key = query
                .mcp_resources
                .as_ref()
                .and_then(|client| client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME));
            let previous_catalog_auth = state
                .cloud_generation
                .as_ref()
                .filter(|generation| generation.catalog.is_some())
                .map(|generation| generation.auth_cache_key.clone());
            let auth_cache_key =
                state.prepare_cloud_generation(query.mcp_resources.as_deref().cloned());
            let catalog_cleared =
                previous_catalog_auth.is_some_and(|previous| previous != auth_cache_key);
            (auth_cache_key, resource_cache_key, catalog_cleared)
        };
        let result = provider.list(query).await;
        let mut state = state.contributor_state();
        match result {
            Ok(catalog) => {
                let needs_retry = catalog.is_partial || !catalog.warnings.is_empty();
                let published = state.publish_cloud_catalog(
                    auth_cache_key.as_ref(),
                    resource_cache_key.clone(),
                    catalog,
                );
                if published {
                    state.cloud_retry = if needs_retry {
                        Some(CatalogRefreshBackoff::after_failure(
                            auth_cache_key,
                            resource_cache_key,
                            state.cloud_retry.as_ref(),
                        ))
                    } else {
                        None
                    };
                } else {
                    tracing::debug!(
                        "cloud plugin discovery finished after auth or resources changed"
                    );
                    return Err(CloudPluginRefreshError {
                        error: "cloud plugin discovery was superseded".to_string(),
                        catalog_cleared,
                        retry_backoff: None,
                    });
                }
            }
            Err(error) => {
                let retry_backoff = state
                    .cloud_generation
                    .as_ref()
                    .filter(|generation| {
                        generation.auth_cache_key == auth_cache_key
                            && generation.is_auth_current()
                            && generation.current_resource_cache_key() == resource_cache_key
                    })
                    .map(|_| {
                        CatalogRefreshBackoff::after_failure(
                            auth_cache_key,
                            resource_cache_key,
                            state.cloud_retry.as_ref(),
                        )
                    });
                tracing::warn!("cloud plugin discovery failed");
                return Err(CloudPluginRefreshError {
                    error: error.to_string(),
                    catalog_cleared,
                    retry_backoff,
                });
            }
        }
        Ok(true)
    }
}

impl ThreadLifecycleContributor<Config> for PluginContributor {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let state = input.thread_store.get_or_init(PluginsThreadState::default);
            {
                let mut state = state.contributor_state();
                state.plugins_enabled = self.providers.cloud_enabled(input.config);
                state.extension_metrics = input.extension_metrics;
            }
            if let Some(resources) = &input.mcp_resource_client {
                input.session_store.insert(resources.as_ref().clone());
            }
        })
    }
}

impl ConfigContributor<Config> for PluginContributor {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        let plugins_enabled = self.providers.cloud_enabled(new_config);
        let state = thread_store.get_or_init(PluginsThreadState::default);
        let mut state = state.contributor_state();
        if state.plugins_enabled != plugins_enabled {
            state.cloud_retry = None;
        }
        state.plugins_enabled = plugins_enabled;
        if !plugins_enabled {
            state.cloud_generation = None;
            state.cloud_retry = None;
        }
    }
}

impl TurnLifecycleContributor for PluginContributor {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        input: StepCatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, bool> {
        Box::pin(async move {
            let Some(state) = input.thread_store.get::<PluginsThreadState>() else {
                return false;
            };
            let query = cloud_catalog_query(input.into(), Some(input.turn_id));
            {
                let mut state = state.contributor_state();
                let auth = query
                    .mcp_resources
                    .as_ref()
                    .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME));
                let resources = query
                    .mcp_resources
                    .as_ref()
                    .and_then(|client| client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME));
                if let Some(generation) = &state.cloud_generation
                    && generation
                        .catalog
                        .as_ref()
                        .is_some_and(|catalog| !catalog.is_partial && catalog.warnings.is_empty())
                    && generation.auth_cache_key == auth
                    && generation.is_auth_current()
                    && generation.resource_cache_key == resources
                {
                    state.cloud_retry = None;
                    return false;
                }
                if state
                    .cloud_retry
                    .as_ref()
                    .is_some_and(|retry| retry.is_pending(auth.as_ref(), resources.as_ref()))
                {
                    return false;
                }
            }
            match self.discover_cloud_catalog(&state, query).await {
                Ok(changed) => changed,
                Err(failure) => {
                    if let Some(retry_backoff) = failure.retry_backoff {
                        state.contributor_state().cloud_retry = Some(retry_backoff);
                    }
                    failure.catalog_cleared
                }
            }
        })
    }

    fn refresh_cloud_catalog<'a>(
        &'a self,
        input: CloudCatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let Some(state) = input.thread_store.get::<PluginsThreadState>() else {
                return Ok(());
            };
            // A failed explicit attempt gets an immediate retry on the next step.
            state.contributor_state().cloud_retry = None;
            self.discover_cloud_catalog(&state, cloud_catalog_query(input, /*turn_id*/ None))
                .await
                .map(|_| ())
                .map_err(|failure| failure.error)
        })
    }
}

/// Cloud packages contribute Apps attribution, never executor MCP declarations.
pub(crate) fn hosted_plugin_connectors(catalog: &PluginCatalog) -> Vec<McpServerContribution> {
    catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled)
        .filter_map(|entry| {
            let PluginIdentity::Remote { remote_plugin_id } = &entry.id else {
                return None;
            };
            let mut seen_connector_ids = HashSet::new();
            Some(McpServerContribution::HostedPluginConnectors {
                plugin_id: remote_plugin_id.clone(),
                plugin_display_name: entry
                    .display_name
                    .chars()
                    .take(MAX_PLUGIN_DISPLAY_NAME_CHARS)
                    .collect(),
                connector_ids: entry
                    .connector_ids
                    .iter()
                    .filter(|connector_id| seen_connector_ids.insert((*connector_id).clone()))
                    .take(MAX_CONNECTOR_IDS_PER_PLUGIN)
                    .cloned()
                    .collect(),
            })
        })
        .take(MAX_PROJECTED_CLOUD_PLUGINS)
        .collect()
}

#[cfg(test)]
#[path = "cloud_plugin_tests.rs"]
mod tests;
