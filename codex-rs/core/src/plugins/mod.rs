mod discoverable;
mod injection;
mod mentions;
pub(crate) mod metrics;
mod render;
#[cfg(test)]
#[path = "skill_snapshot_tests.rs"]
mod skill_snapshot_tests;
#[cfg(test)]
pub(crate) mod test_support;

use crate::config::Config;
use codex_core_plugins::PluginsManager;
use codex_login::AuthManager;
use codex_skills_extension::HostSkillsService;
use std::sync::Arc;

pub(crate) use codex_plugin::PluginCapabilitySummary;

pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub(crate) use render::render_explicit_plugin_instructions;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;

/// Constructs a standalone plugin manager with extension-owned plugin skill loading.
///
/// Callers that already own a host skills service should inject that existing service instead.
pub fn plugins_manager_for_config(
    config: &Config,
    auth_manager: Arc<AuthManager>,
) -> PluginsManager {
    let skill_root_loader = Arc::new(HostSkillsService::new(
        config.codex_home.clone(),
        /*bundled_skills_enabled*/ false,
    ));
    PluginsManager::new(
        config.codex_home.to_path_buf(),
        auth_manager,
        skill_root_loader,
    )
}

/// Initialize a standalone command's runtime before it can make first-party plugin requests.
/// Callers that own a ThreadManager should reuse that manager's PluginsManager instead.
pub async fn plugins_manager_for_config_with_runtime(
    config: &Config,
    auth_manager: Arc<AuthManager>,
) -> anyhow::Result<PluginsManager> {
    if !config
        .features
        .enabled(codex_features::Feature::RuntimeAgentInstances)
    {
        return Ok(plugins_manager_for_config(config, auth_manager));
    }
    codex_model_provider::validate_runtime_destination(Some(&config.chatgpt_base_url))?;
    if !codex_runtime_agent_instance::is_openai_api_url(&config.chatgpt_base_url) {
        return Ok(plugins_manager_for_config(config, auth_manager));
    }
    let installation_id =
        crate::installation_id::resolve_installation_id(&config.codex_home).await?;
    let runtime = crate::runtime_auth::build_runtime_auth_manager(
        config,
        Arc::clone(&auth_manager),
        &installation_id,
    );
    let slot = Arc::new(runtime.map(std::sync::OnceLock::from).unwrap_or_default());
    Ok(PluginsManager::new_with_options(
        config.codex_home.to_path_buf(),
        Some(codex_protocol::protocol::Product::Codex),
        auth_manager,
        slot,
        Arc::new(HostSkillsService::new(
            config.codex_home.clone(),
            /*bundled_skills_enabled*/ false,
        )),
    ))
}
