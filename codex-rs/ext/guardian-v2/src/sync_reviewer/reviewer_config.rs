//! Applies Guardian's reviewer settings to a fresh copy of the captured parent configuration.
//! Managed constraints stay enforced by Config; context and live network state are added by
//! the temporary host adapter before it decides whether an existing reviewer can be reused.

use super::Config;
use super::Constrained;
use super::TokenBudgetConfig;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use std::collections::HashMap;

pub(crate) fn build_reviewer_config(parent_config: &Config) -> anyhow::Result<Config> {
    let mut config = parent_config.clone();
    config.model_provider.request_max_retries = Some(1);
    config.model_provider.stream_max_retries = Some(1);
    config.include_skill_instructions = false;
    config.memories.use_memories = false;
    config.memories.dedicated_tools = false;
    // An explicit disabled config prevents model defaults from reactivating it.
    config.token_budget_startup_config = None;
    config.token_budget = Some(TokenBudgetConfig::default());
    config.notify = None;
    config.developer_instructions = None;
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    config
        .permissions
        .set_permission_profile(codex_guardian_reviewer::reviewer_permission_profile(
            config.permissions.permission_profile(),
        ))
        .map_err(|err| {
            anyhow::anyhow!("guardian review session could not set permission profile: {err}")
        })?;
    config.include_apps_instructions = false;
    config.mcp_servers.set(HashMap::new()).map_err(|err| {
        anyhow::anyhow!("guardian review session could not clear MCP servers: {err}")
    })?;
    for feature in [
        Feature::Collab,
        Feature::MultiAgentV2,
        Feature::GuardianV2,
        Feature::TokenBudget,
        Feature::ContextManagement,
        Feature::CodexHooks,
        Feature::Apps,
        Feature::Plugins,
        Feature::WebSearchRequest,
        Feature::WebSearchCached,
    ] {
        config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "guardian review session could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if config.features.enabled(feature) {
            tracing::warn!(
                "guardian review session could not disable `features.{}`; continuing with the feature enabled",
                feature.key()
            );
        }
    }
    Ok(config)
}
