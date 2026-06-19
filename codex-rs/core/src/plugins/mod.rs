mod discoverable;
mod injection;
mod mentions;
mod render;
#[cfg(test)]
pub(crate) mod test_support;

pub(crate) use codex_plugin::PluginCapabilitySummary;

pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub(crate) use render::render_explicit_plugin_instructions;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;

use crate::config::Config;
use codex_core_plugins::PluginLoadOutcome;
use codex_features::Feature;

pub(crate) fn apply_connector_skills_feature(
    config: &Config,
    loaded_plugins: PluginLoadOutcome,
) -> PluginLoadOutcome {
    if config.features.enabled(Feature::ConnectorSkills) {
        loaded_plugins
    } else {
        loaded_plugins.without_app_backed_skills()
    }
}
