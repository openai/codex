//! Refreshes the cloud catalog each regular turn and projects Apps-only plugin packages.

use crate::PluginListQuery;
use crate::PluginsThreadState;
use crate::plugin_contributor::PluginContributor;
use crate::plugin_contributor_state::SelectedPluginMetadata;
use codex_core::config::Config;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginIdentity;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;
use codex_features::Feature;
use codex_mcp::McpResourceClient;
use std::collections::HashSet;
use std::sync::Arc;

const MAX_PROJECTED_CLOUD_PLUGINS: usize = 100;
const MAX_PLUGIN_DISPLAY_NAME_CHARS: usize = 64;
const MAX_CONNECTOR_IDS_PER_PLUGIN: usize = 64;

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
        state.plugins_enabled = plugins_enabled;
        if self.providers.cloud.is_none() || !plugins_enabled {
            state.cloud_generation = None;
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
            let Some(provider) = &self.providers.cloud else {
                return;
            };
            if !input
                .thread_store
                .get::<PluginsThreadState>()
                .is_some_and(|state| state.contributor_state().plugins_enabled)
            {
                return;
            }
            let resources = input
                .session_store
                .get::<McpResourceClient>()
                .map(|resources| resources.as_ref().clone());
            let state = input.thread_store.get_or_init(PluginsThreadState::default);
            // Change generations before the fallible lookup so only failures within the
            // same auth scope may retain a previously published catalog.
            let auth_cache_key = state
                .contributor_state()
                .prepare_cloud_generation(resources.clone());
            match provider
                .list(PluginListQuery {
                    thread_id: input.thread_store.level_id().to_string(),
                    turn_id: input.turn_id.to_string(),
                    mcp_resources: resources.map(Arc::new),
                })
                .await
            {
                Ok(catalog) => {
                    let mut state = state.contributor_state();
                    if !state.publish_cloud_catalog(auth_cache_key.as_ref(), catalog) {
                        tracing::warn!("cloud plugin discovery finished after auth changed");
                    }
                }
                Err(_) => tracing::warn!("cloud plugin discovery failed"),
            }
        })
    }
}

/// Cloud packages contribute Apps attribution, never executor MCP declarations.
pub(crate) struct CloudPluginMetadata {
    pub(crate) selection_order: usize,
    pub(crate) selected_root_id: String,
    pub(crate) metadata: SelectedPluginMetadata,
}

pub(crate) fn catalog_to_metadata(catalog: &PluginCatalog) -> Vec<CloudPluginMetadata> {
    catalog
        .entries
        .iter()
        .enumerate()
        .filter_map(|(selection_order, entry)| {
            let PluginIdentity::Remote { remote_plugin_id } = &entry.id else {
                return None;
            };
            let mut seen_connector_ids = HashSet::new();
            Some(CloudPluginMetadata {
                selection_order,
                selected_root_id: format!("cloud:{remote_plugin_id}"),
                metadata: SelectedPluginMetadata {
                    plugin_id: remote_plugin_id.clone(),
                    plugin_display_name: entry
                        .display_name
                        .chars()
                        .take(MAX_PLUGIN_DISPLAY_NAME_CHARS)
                        .collect(),
                    servers: Vec::new(),
                    connector_ids: entry
                        .connector_ids
                        .iter()
                        .filter(|connector_id| seen_connector_ids.insert((*connector_id).clone()))
                        .take(MAX_CONNECTOR_IDS_PER_PLUGIN)
                        .cloned()
                        .collect(),
                },
            })
        })
        .take(MAX_PROJECTED_CLOUD_PLUGINS)
        .collect()
}

#[cfg(test)]
#[path = "cloud_plugin_tests.rs"]
mod tests;
