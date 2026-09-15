//! Adds model, policy context and live network state to the extension's reviewer configuration.
//! Both prewarming and reviews finish this setup before context preparation and reuse checks.

use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::openai_models::ModelMessages;

use crate::config::Config;
use crate::config::NetworkProxySpec;

use super::prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
use super::prompt::guardian_policy_prompt_with_config_and_template;

/// Adds the captured model, policy prompt and live network rules before reuse selection.
pub fn build_guardian_review_session_config(
    mut guardian_config: Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    reasoning_summary: codex_protocol::config_types::ReasoningSummary,
    personality: Option<codex_protocol::config_types::Personality>,
    model_messages: Option<&ModelMessages>,
) -> anyhow::Result<Config> {
    guardian_config.model = Some(active_model.to_owned());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.model_reasoning_summary = Some(reasoning_summary);
    guardian_config.personality = personality;
    let catalog_auto_review = model_messages.and_then(|messages| messages.auto_review.as_ref());
    let tenant_policy_config = guardian_config.resolve_guardian_policy(model_messages);
    let policy_template = guardian_config
        .guardian_policy_template
        .as_deref()
        .or_else(|| catalog_auto_review.and_then(|messages| messages.policy_template.as_deref()))
        .unwrap_or(BUNDLED_GUARDIAN_POLICY_TEMPLATE);
    guardian_config.base_instructions = Some(guardian_policy_prompt_with_config_and_template(
        tenant_policy_config,
        policy_template,
    ));
    guardian_config.base_instructions_provenance = Some(BaseInstructionsProvenance::Custom);
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
    Ok(guardian_config)
}
