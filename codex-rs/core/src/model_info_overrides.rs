use codex_features::Feature;
use codex_models::ModelInfoConfigOverrides;

use crate::config::Config;

pub(crate) fn model_info_config_overrides(config: &Config) -> ModelInfoConfigOverrides {
    ModelInfoConfigOverrides {
        model_supports_reasoning_summaries: config.model_supports_reasoning_summaries,
        model_context_window: config.model_context_window,
        model_auto_compact_token_limit: config.model_auto_compact_token_limit,
        tool_output_token_limit: config.tool_output_token_limit,
        base_instructions: config.base_instructions.clone(),
        personality_enabled: config.features.enabled(Feature::Personality),
    }
}
