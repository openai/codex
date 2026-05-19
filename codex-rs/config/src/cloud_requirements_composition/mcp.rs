use super::CloudRequirementsCompositionError;
use super::CloudRequirementsFragmentSource;
use super::composition_conflict;
use super::merge_output_source;
use crate::McpServerRequirement;
use crate::PluginRequirementsToml;
use crate::Sourced;
use std::collections::BTreeMap;

// MCP requirements merge as keyed unions. Repeating an identical server
// definition is allowed, but conflicting definitions for the same key fail
// closed because transport/identity fields do not have independent semantics.
// Plugin-scoped MCP servers follow the same rule within each plugin id.

#[derive(Default)]
pub(super) struct McpMergeState {
    mcp_server_sources: BTreeMap<String, CloudRequirementsFragmentSource>,
    plugin_mcp_server_sources: BTreeMap<(String, String), CloudRequirementsFragmentSource>,
}

impl McpMergeState {
    pub(super) fn merge_mcp_servers(
        &mut self,
        target: &mut Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
        incoming: Option<BTreeMap<String, McpServerRequirement>>,
        source_ref: &CloudRequirementsFragmentSource,
    ) -> Result<(), CloudRequirementsCompositionError> {
        let Some(incoming) = incoming else {
            return Ok(());
        };
        let Some(existing) = target.as_mut() else {
            self.mcp_server_sources.extend(
                incoming
                    .keys()
                    .map(|server_id| (server_id.clone(), source_ref.clone())),
            );
            *target = Some(Sourced::new(incoming, source_ref.requirement_source()));
            return Ok(());
        };

        for (server_id, server_requirement) in incoming {
            match existing.value.get(&server_id) {
                Some(existing_requirement) if existing_requirement != &server_requirement => {
                    let existing_source = self
                        .mcp_server_sources
                        .get(&server_id)
                        .cloned()
                        .unwrap_or_else(|| source_ref.clone());
                    return Err(composition_conflict(
                        format!("mcp_servers.{server_id}"),
                        existing_source,
                        source_ref.clone(),
                        "server definitions differ",
                    ));
                }
                Some(_) => {}
                None => {
                    existing.value.insert(server_id.clone(), server_requirement);
                    self.mcp_server_sources
                        .insert(server_id, source_ref.clone());
                    merge_output_source(&mut existing.source, source_ref);
                }
            }
        }
        Ok(())
    }

    pub(super) fn merge_plugins(
        &mut self,
        target: &mut Option<Sourced<BTreeMap<String, PluginRequirementsToml>>>,
        incoming: Option<BTreeMap<String, PluginRequirementsToml>>,
        source_ref: &CloudRequirementsFragmentSource,
    ) -> Result<(), CloudRequirementsCompositionError> {
        let Some(incoming) =
            incoming.filter(|plugins| !plugins.values().all(PluginRequirementsToml::is_empty))
        else {
            return Ok(());
        };
        let Some(existing) = target.as_mut() else {
            track_plugin_mcp_sources(&mut self.plugin_mcp_server_sources, &incoming, source_ref);
            *target = Some(Sourced::new(incoming, source_ref.requirement_source()));
            return Ok(());
        };

        for (plugin_id, plugin_requirement) in incoming {
            let existing_plugin = existing.value.entry(plugin_id.clone()).or_default();
            if merge_plugin_requirement(
                &plugin_id,
                existing_plugin,
                plugin_requirement,
                &mut self.plugin_mcp_server_sources,
                source_ref,
            )? {
                merge_output_source(&mut existing.source, source_ref);
            }
        }
        Ok(())
    }
}

fn track_plugin_mcp_sources(
    target: &mut BTreeMap<(String, String), CloudRequirementsFragmentSource>,
    plugins: &BTreeMap<String, PluginRequirementsToml>,
    source_ref: &CloudRequirementsFragmentSource,
) {
    for (plugin_id, plugin) in plugins {
        let Some(mcp_servers) = plugin.mcp_servers.as_ref() else {
            continue;
        };
        for server_id in mcp_servers.keys() {
            target.insert((plugin_id.clone(), server_id.clone()), source_ref.clone());
        }
    }
}

fn merge_plugin_requirement(
    plugin_id: &str,
    existing: &mut PluginRequirementsToml,
    incoming: PluginRequirementsToml,
    sources: &mut BTreeMap<(String, String), CloudRequirementsFragmentSource>,
    source_ref: &CloudRequirementsFragmentSource,
) -> Result<bool, CloudRequirementsCompositionError> {
    // Destructure without `..` so new plugin requirement fields cannot silently
    // skip cloud composition.
    let PluginRequirementsToml { mcp_servers } = incoming;
    let Some(incoming_servers) = mcp_servers else {
        return Ok(false);
    };
    let Some(existing_servers) = existing.mcp_servers.as_mut() else {
        for server_id in incoming_servers.keys() {
            sources.insert(
                (plugin_id.to_string(), server_id.clone()),
                source_ref.clone(),
            );
        }
        existing.mcp_servers = Some(incoming_servers);
        return Ok(true);
    };

    let mut changed = false;
    for (server_id, server_requirement) in incoming_servers {
        match existing_servers.get(&server_id) {
            Some(existing_requirement) if existing_requirement != &server_requirement => {
                let existing_source = sources
                    .get(&(plugin_id.to_string(), server_id.clone()))
                    .cloned()
                    .unwrap_or_else(|| source_ref.clone());
                return Err(composition_conflict(
                    format!("plugins.{plugin_id}.mcp_servers.{server_id}"),
                    existing_source,
                    source_ref.clone(),
                    "server definitions differ",
                ));
            }
            Some(_) => {}
            None => {
                existing_servers.insert(server_id.clone(), server_requirement);
                sources.insert((plugin_id.to_string(), server_id), source_ref.clone());
                changed = true;
            }
        }
    }
    Ok(changed)
}
