//! Searchable usage diagnostics from an explicit scalar-only configuration allowlist.
//! Never include prompts, paths, credentials, provider endpoints, or raw configuration.

use crate::config::Config;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::openai_models::ModelInfo;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) fn usage_tags(
    config: &Config,
    features: &Features,
    model_info: &ModelInfo,
    service_tier: Option<&str>,
) -> BTreeMap<String, String> {
    // Keep configured overrides separate from the step's resolved model limits.
    // An explicit "unset" overwrites a prior turn's override in the collector.
    let values = json!({
        "model_context_window": config.model_context_window,
        "model_context_window_override": config.model_context_window.is_some(),
        "model_context_window_effective": model_info.usable_context_window(),
        "model_auto_compact_token_limit": config.model_auto_compact_token_limit,
        "model_auto_compact_token_limit_scope": config.model_auto_compact_token_limit_scope,
        "tool_output_token_limit": config.tool_output_token_limit,
        "project_doc_max_bytes": config.project_doc_max_bytes,
        "model_verbosity": config.model_verbosity,
        "model_reasoning_summary": config.model_reasoning_summary,
        "service_tier": service_tier,
        "review_model": config.review_model,
        "agent_default_subagent_model": config.agent_default_subagent_model,
        "agent_default_subagent_reasoning_effort": config.agent_default_subagent_reasoning_effort,
        "multi_agent_v2.max_concurrent_threads_per_session": config.multi_agent_v2.max_concurrent_threads_per_session,
        "multi_agent_v2.wait_agent_enabled": config.multi_agent_v2.wait_agent_enabled,
        "max_goal_token_budget": config.max_goal_token_budget,
        "memories.generate_memories": config.memories.generate_memories,
        "memories.use_memories": config.memories.use_memories,
        "memories.max_rollouts_per_startup": config.memories.max_rollouts_per_startup,
        "memories.max_raw_memories_for_consolidation": config.memories.max_raw_memories_for_consolidation,
        "memories.extract_model": config.memories.extract_model,
        "memories.consolidation_model": config.memories.consolidation_model,
        "token_budget.present": config.token_budget.is_some(),
        "token_budget.reminder_threshold_tokens": config.token_budget.as_ref().and_then(|budget| budget.reminder_threshold_tokens),
        "token_budget.auto_compact_fallback_buffer_tokens": config.token_budget.as_ref().and_then(|budget| budget.auto_compact_fallback_buffer_tokens),
        "rollout_budget.limit_tokens": config.rollout_budget.as_ref().map(|budget| budget.limit_tokens),
        "rollout_budget.sampling_token_weight": config.rollout_budget.as_ref().map(|budget| budget.sampling_token_weight),
        "rollout_budget.prefill_token_weight": config.rollout_budget.as_ref().map(|budget| budget.prefill_token_weight),
    });
    let Value::Object(values) = values else {
        unreachable!("the usage tag allowlist is an object");
    };
    let mut tags: BTreeMap<_, _> = values
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                Value::Null => "unset".to_string(),
                Value::String(value) => value,
                value => value.to_string(),
            };
            (key, value)
        })
        .collect();
    tags.extend(
        FEATURES
            .iter()
            .filter(|spec| !matches!(spec.id, Feature::Collab | Feature::SpawnCsv))
            .map(|spec| {
                (
                    format!("feature.{}", spec.key),
                    features.enabled(spec.id).to_string(),
                )
            }),
    );
    tags
}

#[cfg(test)]
#[path = "feedback_config_tests.rs"]
mod tests;
