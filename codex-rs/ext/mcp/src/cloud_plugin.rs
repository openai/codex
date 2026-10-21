//! Refreshes cloud plugins each turn and after auth changes.

use crate::PluginListQuery;
use crate::PluginsThreadState;
use crate::plugin_contributor::PluginContributor;
use codex_core::config::Config;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginIdentity;
use codex_extension_api::CatalogRefreshBackoff;
use codex_extension_api::CatalogRefreshInput;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientAuthKey;
use std::collections::HashSet;

const MAX_PROJECTED_CLOUD_PLUGINS: usize = 100;
pub(crate) const MAX_PLUGIN_DISPLAY_NAME_CHARS: usize = 64;
pub(crate) const MAX_CONNECTOR_IDS_PER_PLUGIN: usize = 64;

/// Auth scope of the latest catalog accepted for this turn.
struct LatestCloudPluginAuth(Option<McpResourceClientAuthKey>);

impl PluginContributor {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "cloud discovery must remain serialized through fetching and catalog publication"
    )]
    async fn refresh_cloud_catalog(&self, input: CatalogRefreshInput<'_>) -> bool {
        let Some(provider) = &self.providers.cloud else {
            return false;
        };
        let Some(state) = input.thread_store.get::<PluginsThreadState>() else {
            return false;
        };
        // Wait without holding catalog state or the MCP runtime permit, then recheck the cache.
        let (mut refresh_succeeded, waited) = match state.cloud_refresh_lock.try_lock() {
            Ok(guard) => (guard, false),
            Err(_) => (state.cloud_refresh_lock.lock().await, true),
        };
        let query = PluginListQuery {
            thread_id: input.thread_store.level_id().to_string(),
            turn_id: input.turn_id.to_string(),
            mcp_resources: input.session_store.get::<McpResourceClient>(),
            snapshots: Vec::new(),
        };
        let (auth_cache_key, resource_cache_key, catalog_cleared) =
            {
                let mut state = state.contributor_state();
                if !state.plugins_enabled {
                    return false;
                }
                let auth = query
                    .mcp_resources
                    .as_ref()
                    .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME));
                let resource_cache_key = query
                    .mcp_resources
                    .as_ref()
                    .and_then(|client| client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME));
                if ((waited && *refresh_succeeded)
                    || input
                        .turn_store
                        .get::<LatestCloudPluginAuth>()
                        .is_some_and(|previous| previous.0 == auth))
                    && state
                        .cloud_generation
                        .as_ref()
                        .is_some_and(|generation| generation.auth_cache_key == auth)
                    && state.cloud_catalog().is_some()
                {
                    input.turn_store.insert(LatestCloudPluginAuth(auth));
                    return false;
                }
                if state.cloud_retry.as_ref().is_some_and(|retry| {
                    retry.is_pending(auth.as_ref(), resource_cache_key.as_ref())
                }) {
                    return false;
                }
                // Waiting callers must not reuse a failed or cancelled refresh.
                *refresh_succeeded = false;
                input.turn_store.remove::<LatestCloudPluginAuth>();
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
        let catalog_published = match result {
            Ok(catalog) => {
                let published = state.publish_cloud_catalog(auth_cache_key.as_ref(), catalog);
                if published {
                    state.cloud_retry = None;
                    input
                        .turn_store
                        .insert(LatestCloudPluginAuth(auth_cache_key));
                    *refresh_succeeded = true;
                } else {
                    tracing::debug!("cloud plugin discovery finished after auth changed");
                }
                published
            }
            Err(_) => {
                if state.cloud_generation.as_ref().is_some_and(|generation| {
                    generation.auth_cache_key == auth_cache_key && generation.is_auth_current()
                }) {
                    state.cloud_retry = Some(CatalogRefreshBackoff::after_failure(
                        auth_cache_key,
                        resource_cache_key,
                        state.cloud_retry.as_ref(),
                    ));
                }
                tracing::warn!("cloud plugin discovery failed");
                false
            }
        };
        catalog_cleared || catalog_published
    }
}

impl ThreadLifecycleContributor<Config> for PluginContributor {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let state = input.thread_store.get_or_init(PluginsThreadState::default);
            state.contributor_state().plugins_enabled =
                input.config.features.enabled(Feature::Plugins);
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
        let plugins_enabled = new_config.features.enabled(Feature::Plugins);
        let state = thread_store.get_or_init(PluginsThreadState::default);
        let mut state = state.contributor_state();
        if state.plugins_enabled != plugins_enabled {
            state.cloud_retry = None;
        }
        state.plugins_enabled = plugins_enabled;
        if self.providers.cloud.is_none() || !plugins_enabled {
            state.cloud_generation = None;
            state.cloud_retry = None;
        }
    }
}

impl TurnLifecycleContributor for PluginContributor {
    fn turn_start_phase(&self, thread_store: &ExtensionData) -> TurnStartPhase {
        if self.providers.cloud.is_some()
            && thread_store
                .get::<PluginsThreadState>()
                .is_some_and(|state| state.contributor_state().plugins_enabled)
        {
            TurnStartPhase::RegularTaskStart
        } else {
            TurnStartPhase::BeforeTaskRegistration
        }
    }

    fn requires_mcp_runtime(&self, thread_store: &ExtensionData) -> bool {
        self.turn_start_phase(thread_store) == TurnStartPhase::RegularTaskStart
    }

    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.turn_store.remove::<LatestCloudPluginAuth>();
            self.refresh_cloud_catalog(CatalogRefreshInput {
                turn_id: input.turn_id,
                session_store: input.session_store,
                thread_store: input.thread_store,
                turn_store: input.turn_store,
            })
            .await;
        })
    }

    fn refresh_catalog_for_step<'a>(
        &'a self,
        input: CatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, bool> {
        Box::pin(self.refresh_cloud_catalog(input))
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
