mod discoverable;
mod injection;
mod mentions;
mod render;
#[cfg(test)]
#[path = "skill_snapshot_tests.rs"]
mod skill_snapshot_tests;
#[cfg(test)]
pub(crate) mod test_support;

use crate::config::Config;
use codex_core_plugins::PluginsManager;
use codex_skills_extension::HostSkillsService;
use std::sync::Arc;

pub(crate) use codex_plugin::PluginCapabilitySummary;

pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub(crate) use render::render_explicit_plugin_instructions;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;

/// Constructs a standalone plugin manager with extension-owned plugin skill loading.
///
/// Callers that already own a host skills service should inject that existing service instead.
pub fn plugins_manager_for_config(config: &Config) -> PluginsManager {
    let skill_root_loader = Arc::new(HostSkillsService::new(
        config.codex_home.clone(),
        /*bundled_skills_enabled*/ false,
    ));
    PluginsManager::new(config.codex_home.to_path_buf(), skill_root_loader)
}
