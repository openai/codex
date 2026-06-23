use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use codex_app_server_protocol::AuthMode;
use codex_core_skills::config_rules::SkillConfigRules;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_plugin::app_connector_ids_from_declarations;
use codex_plugin::prompt_safe_plugin_description;
use codex_protocol::protocol::Product;
use tokio::sync::Semaphore;

use crate::app_mcp_routing::apply_app_mcp_routing_policy;
use crate::loader::PluginSkillInventory;
use crate::loader::load_plugin_apps;
use crate::loader::load_plugin_mcp_servers;
use crate::loader::load_plugin_skill_inventory;
use crate::manager::ConfiguredMarketplacePlugin;
use crate::manager::remote_plugin_install_required_description;
use crate::manifest::load_plugin_manifest;
use crate::marketplace::MarketplaceError;
use crate::marketplace::MarketplacePluginSource;

const MAX_TOOL_SUGGEST_METADATA_CACHE_ENTRIES: usize = 1024;

type ToolSuggestMetadataEntry = Result<Arc<ToolSuggestMetadataFragment>, String>;

/// Source-derived plugin metadata cached for tool suggestions.
///
/// `PluginsManager` clears these entries alongside its loaded-plugin cache. Current skill config
/// and auth routing are projected after each lookup and are not part of this cache.
pub(crate) struct ToolSuggestMetadataCache {
    state: RwLock<ToolSuggestMetadataCacheState>,
    load_semaphore: Semaphore,
}

#[derive(Default)]
struct ToolSuggestMetadataCacheState {
    generation: u64,
    entries: HashMap<PluginArtifactIdentity, ToolSuggestMetadataEntry>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct PluginArtifactIdentity {
    plugin_id: String,
    source: MarketplacePluginSource,
}

pub(crate) struct ToolSuggestMetadataFragment {
    config_name: String,
    display_name: String,
    description: Option<String>,
    mcp_server_names: Vec<String>,
    app_declarations: Vec<AppDeclaration>,
    skill_inventory: Option<PluginSkillInventory>,
}

impl ToolSuggestMetadataFragment {
    pub(crate) fn project(
        &self,
        skill_config_rules: &SkillConfigRules,
        auth_mode: Option<AuthMode>,
    ) -> PluginCapabilitySummary {
        let mut app_declarations = self.app_declarations.clone();
        let mut mcp_servers = self
            .mcp_server_names
            .iter()
            .cloned()
            .map(|name| (name, ()))
            .collect::<HashMap<_, _>>();
        if auth_mode.is_some() {
            apply_app_mcp_routing_policy(
                &mut app_declarations,
                &mut mcp_servers,
                auth_mode,
                /*plugin_active*/ true,
            );
        }
        let mut mcp_server_names = mcp_servers.into_keys().collect::<Vec<_>>();
        mcp_server_names.sort_unstable();

        PluginCapabilitySummary {
            config_name: self.config_name.clone(),
            display_name: self.display_name.clone(),
            description: self.description.clone(),
            has_skills: self
                .skill_inventory
                .as_ref()
                .is_some_and(|inventory| inventory.has_enabled_skills(skill_config_rules)),
            mcp_server_names,
            app_connector_ids: app_connector_ids_from_declarations(&app_declarations),
        }
    }
}

impl ToolSuggestMetadataCache {
    pub(crate) fn new() -> Self {
        Self {
            state: RwLock::new(ToolSuggestMetadataCacheState::default()),
            load_semaphore: Semaphore::new(/*permits*/ 1),
        }
    }

    pub(crate) fn clear(&self) {
        let mut state = match self.state.write() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };
        state.generation = state.generation.wrapping_add(1);
        state.entries.clear();
    }

    pub(crate) async fn metadata_for_plugin(
        &self,
        marketplace_name: &str,
        plugin: &ConfiguredMarketplacePlugin,
        restriction_product: Option<Product>,
    ) -> Result<Arc<ToolSuggestMetadataFragment>, MarketplaceError> {
        let artifact = PluginArtifactIdentity {
            plugin_id: plugin.id.clone(),
            source: plugin.source.clone(),
        };
        loop {
            if let Some(entry) = self.cached_entry(&artifact) {
                return entry.map_err(MarketplaceError::InvalidPlugin);
            }

            let _load_permit = self.load_semaphore.acquire().await.map_err(|_| {
                MarketplaceError::InvalidPlugin(
                    "tool-suggest metadata cache loader closed".to_string(),
                )
            })?;
            if let Some(entry) = self.cached_entry(&artifact) {
                return entry.map_err(MarketplaceError::InvalidPlugin);
            }

            let generation = self.generation();
            let entry = load_plugin_metadata(marketplace_name, plugin, restriction_product).await;
            if self.cache_entry_if_current(generation, artifact.clone(), entry.clone()) {
                return entry.map_err(MarketplaceError::InvalidPlugin);
            }
        }
    }

    fn cached_entry(&self, artifact: &PluginArtifactIdentity) -> Option<ToolSuggestMetadataEntry> {
        match self.state.read() {
            Ok(state) => state.entries.get(artifact).cloned(),
            Err(err) => err.into_inner().entries.get(artifact).cloned(),
        }
    }

    fn generation(&self) -> u64 {
        match self.state.read() {
            Ok(state) => state.generation,
            Err(err) => err.into_inner().generation,
        }
    }

    fn cache_entry_if_current(
        &self,
        generation: u64,
        artifact: PluginArtifactIdentity,
        entry: ToolSuggestMetadataEntry,
    ) -> bool {
        let mut state = match self.state.write() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };
        if state.generation != generation {
            return false;
        }
        if state.entries.len() >= MAX_TOOL_SUGGEST_METADATA_CACHE_ENTRIES
            && !state.entries.contains_key(&artifact)
        {
            state.entries.clear();
        }
        state.entries.insert(artifact, entry);
        true
    }
}

async fn load_plugin_metadata(
    marketplace_name: &str,
    plugin: &ConfiguredMarketplacePlugin,
    restriction_product: Option<Product>,
) -> ToolSuggestMetadataEntry {
    let plugin_id = PluginId::new(plugin.name.clone(), marketplace_name.to_string()).map_err(
        |err| match err {
            PluginIdError::Invalid(message) => message,
        },
    )?;

    let MarketplacePluginSource::Local { path: plugin_root } = &plugin.source else {
        return Ok(Arc::new(ToolSuggestMetadataFragment {
            config_name: plugin.id.clone(),
            display_name: plugin.name.clone(),
            description: prompt_safe_plugin_description(Some(
                &remote_plugin_install_required_description(&plugin.source),
            )),
            mcp_server_names: Vec::new(),
            app_declarations: Vec::new(),
            skill_inventory: None,
        }));
    };
    if !plugin_root.as_path().is_dir() {
        return Err("path does not exist or is not a directory".to_string());
    }
    let manifest = load_plugin_manifest(plugin_root.as_path())
        .ok_or_else(|| "missing or invalid plugin.json".to_string())?;
    let skill_inventory = load_plugin_skill_inventory(
        plugin_root,
        &plugin_id,
        &manifest,
        restriction_product,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    let mut mcp_server_names =
        load_plugin_mcp_servers(plugin_root.as_path(), /*auth_mode*/ None)
            .await
            .into_keys()
            .collect::<Vec<_>>();
    mcp_server_names.sort_unstable();
    mcp_server_names.dedup();
    let app_declarations = load_plugin_apps(plugin_root.as_path()).await;

    Ok(Arc::new(ToolSuggestMetadataFragment {
        config_name: plugin.id.clone(),
        display_name: plugin.name.clone(),
        description: prompt_safe_plugin_description(manifest.description.as_deref()),
        mcp_server_names,
        app_declarations,
        skill_inventory: Some(skill_inventory),
    }))
}
