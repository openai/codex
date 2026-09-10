//! Applies extension-owned reviewer settings to host configuration and builds context.
//! Managed constraints, live network rules and policy prompt construction stay in the host.

use std::collections::HashMap;

use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::openai_models::ModelMessages;
use tracing::warn;

use crate::config::Config;
use crate::config::Constrained;
use crate::config::NetworkProxySpec;
use crate::config::TokenBudgetConfig;

use super::prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
use super::prompt::guardian_policy_prompt_with_config_and_template;

/// Builds the existing read-only reviewer configuration with its policy and live network rules.
pub fn build_guardian_review_session_config(
    parent_config: &Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    reasoning_summary: codex_protocol::config_types::ReasoningSummary,
    personality: Option<codex_protocol::config_types::Personality>,
    model_messages: Option<&ModelMessages>,
) -> anyhow::Result<Config> {
    let mut guardian_config = parent_config.clone();
    let overrides = codex_guardian_reviewer::reviewer_config_overrides(
        parent_config.permissions.permission_profile(),
        active_model,
        reasoning_effort,
    );
    guardian_config.model = Some(overrides.model);
    guardian_config.model_reasoning_effort = overrides.reasoning_effort;
    guardian_config.model_reasoning_summary = Some(reasoning_summary);
    guardian_config.personality = personality;
    guardian_config.model_provider.request_max_retries = Some(overrides.request_max_retries);
    guardian_config.model_provider.stream_max_retries = Some(overrides.stream_max_retries);
    guardian_config.include_skill_instructions = overrides.include_skill_instructions;
    guardian_config.memories.use_memories = overrides.use_memories;
    guardian_config.memories.dedicated_tools = overrides.dedicated_memory_tools;
    if !overrides.inherit_token_budget {
        // An explicit disabled config prevents model defaults from reactivating it.
        guardian_config.token_budget_startup_config = None;
        guardian_config.token_budget = Some(TokenBudgetConfig::default());
    }
    let catalog_auto_review = model_messages.and_then(|messages| messages.auto_review.as_ref());
    let tenant_policy_config = parent_config.resolve_guardian_policy(model_messages);
    let policy_template = catalog_auto_review
        .and_then(|messages| messages.policy_template.as_deref())
        .unwrap_or(BUNDLED_GUARDIAN_POLICY_TEMPLATE);
    guardian_config.base_instructions = Some(guardian_policy_prompt_with_config_and_template(
        tenant_policy_config,
        policy_template,
    ));
    guardian_config.base_instructions_provenance = Some(BaseInstructionsProvenance::Custom);
    guardian_config.notify = overrides.notify;
    guardian_config.developer_instructions = overrides.developer_instructions;
    guardian_config.permissions.approval_policy =
        Constrained::allow_only(overrides.approval_policy);
    guardian_config
        .permissions
        .set_permission_profile(overrides.permission_profile)
        .map_err(|err| {
            anyhow::anyhow!("guardian review session could not set permission profile: {err}")
        })?;
    guardian_config.include_apps_instructions = overrides.include_apps_instructions;
    if !overrides.inherit_mcp_servers {
        guardian_config
            .mcp_servers
            .set(HashMap::new())
            .map_err(|err| {
                anyhow::anyhow!("guardian review session could not clear MCP servers: {err}")
            })?;
    }
    if let Some(live_network_config) = live_network_config
        && guardian_config.permissions.network.is_some()
    {
        let network_constraints = guardian_config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()
            .map(|network| network.value.clone());
        guardian_config.permissions.network = Some(NetworkProxySpec::from_config_and_constraints(
            live_network_config,
            network_constraints,
            guardian_config.permissions.permission_profile(),
        )?);
    }
    for feature in overrides.disabled_features {
        guardian_config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "guardian review session could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if guardian_config.features.enabled(feature) {
            warn!(
                "guardian review session could not disable `features.{}`; continuing with the feature enabled",
                feature.key()
            );
        }
    }
    Ok(guardian_config)
}
