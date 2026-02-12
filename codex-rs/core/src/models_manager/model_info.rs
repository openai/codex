use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;

use crate::config::Config;
use crate::features::Feature;
use crate::truncate::approx_bytes_for_tokens;
use tracing::warn;

pub const BASE_INSTRUCTIONS: &str = include_str!("../../prompt.md");
const DEFAULT_PERSONALITY_HEADER: &str = "You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals.";
const LOCAL_FRIENDLY_TEMPLATE: &str =
    "You optimize for team morale and being a supportive teammate as much as code quality.";
const LOCAL_PRAGMATIC_TEMPLATE: &str = "You are a deeply pragmatic, effective software engineer.";
const PERSONALITY_PLACEHOLDER: &str = "{{ personality }}";

pub(crate) fn with_model_info_patch(
    mut model: ModelInfo,
    model_key: &str,
    config: &Config,
) -> ModelInfo {
    let Some(model_info_patch) = config.model_info_overrides.get(model_key) else {
        return model;
    };

    model.slug = model_key.to_string();

    if let Some(display_name) = &model_info_patch.display_name {
        model.display_name = display_name.clone();
    }
    if let Some(description) = &model_info_patch.description {
        model.description = Some(description.clone());
    }
    if let Some(default_reasoning_level) = model_info_patch.default_reasoning_level {
        model.default_reasoning_level = Some(default_reasoning_level);
    }
    if let Some(supported_reasoning_levels) = &model_info_patch.supported_reasoning_levels {
        model.supported_reasoning_levels = supported_reasoning_levels.clone();
    }
    if let Some(shell_type) = model_info_patch.shell_type {
        model.shell_type = shell_type;
    }
    if let Some(visibility) = model_info_patch.visibility {
        model.visibility = visibility;
    }
    if let Some(supported_in_api) = model_info_patch.supported_in_api {
        model.supported_in_api = supported_in_api;
    }
    if let Some(priority) = model_info_patch.priority {
        model.priority = priority;
    }
    if let Some(upgrade) = &model_info_patch.upgrade {
        model.upgrade = Some(upgrade.clone());
    }
    if let Some(base_instructions) = &model_info_patch.base_instructions {
        model.base_instructions = base_instructions.clone();
        // Keep parity with top-level config behavior: explicit base instructions
        // should disable template-driven model messages unless the patch sets them again.
        model.model_messages = None;
    }
    if let Some(model_messages) = &model_info_patch.model_messages {
        model.model_messages = Some(model_messages.clone());
    }
    if let Some(supports_reasoning_summaries) = model_info_patch.supports_reasoning_summaries {
        model.supports_reasoning_summaries = supports_reasoning_summaries;
    }
    if let Some(support_verbosity) = model_info_patch.support_verbosity {
        model.support_verbosity = support_verbosity;
    }
    if let Some(default_verbosity) = model_info_patch.default_verbosity {
        model.default_verbosity = Some(default_verbosity);
    }
    if let Some(apply_patch_tool_type) = &model_info_patch.apply_patch_tool_type {
        model.apply_patch_tool_type = Some(apply_patch_tool_type.clone());
    }
    if let Some(truncation_policy) = model_info_patch.truncation_policy {
        model.truncation_policy = truncation_policy;
    }
    if let Some(supports_parallel_tool_calls) = model_info_patch.supports_parallel_tool_calls {
        model.supports_parallel_tool_calls = supports_parallel_tool_calls;
    }
    if let Some(context_window) = model_info_patch.context_window {
        model.context_window = Some(context_window);
    }
    if let Some(auto_compact_token_limit) = model_info_patch.auto_compact_token_limit {
        model.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    if let Some(effective_context_window_percent) =
        model_info_patch.effective_context_window_percent
    {
        model.effective_context_window_percent = effective_context_window_percent;
    }
    if let Some(experimental_supported_tools) = &model_info_patch.experimental_supported_tools {
        model.experimental_supported_tools = experimental_supported_tools.clone();
    }
    if let Some(input_modalities) = &model_info_patch.input_modalities {
        model.input_modalities = input_modalities.clone();
    }
    if let Some(prefer_websockets) = model_info_patch.prefer_websockets {
        model.prefer_websockets = prefer_websockets;
    }

    model
}

pub(crate) fn with_model_info_patches(
    mut model_info: ModelInfo,
    requested_model: &str,
    config: &Config,
) -> ModelInfo {
    let resolved_slug = model_info.slug.clone();
    model_info = with_model_info_patch(model_info, &resolved_slug, config);
    if requested_model != resolved_slug {
        model_info = with_model_info_patch(model_info, requested_model, config);
    }
    model_info
}

pub(crate) fn with_config_overrides(mut model: ModelInfo, config: &Config) -> ModelInfo {
    if let Some(supports_reasoning_summaries) = config.model_supports_reasoning_summaries {
        model.supports_reasoning_summaries = supports_reasoning_summaries;
    }
    if let Some(context_window) = config.model_context_window {
        model.context_window = Some(context_window);
    }
    if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
        model.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    if let Some(token_limit) = config.tool_output_token_limit {
        model.truncation_policy = match model.truncation_policy.mode {
            TruncationMode::Bytes => {
                let byte_limit =
                    i64::try_from(approx_bytes_for_tokens(token_limit)).unwrap_or(i64::MAX);
                TruncationPolicyConfig::bytes(byte_limit)
            }
            TruncationMode::Tokens => {
                let limit = i64::try_from(token_limit).unwrap_or(i64::MAX);
                TruncationPolicyConfig::tokens(limit)
            }
        };
    }

    if let Some(base_instructions) = &config.base_instructions {
        model.base_instructions = base_instructions.clone();
        model.model_messages = None;
    } else if !config.features.enabled(Feature::Personality) {
        model.model_messages = None;
    }

    model
}

/// Build a minimal fallback model descriptor for missing/unknown slugs.
pub(crate) fn model_info_from_slug(slug: &str) -> ModelInfo {
    warn!("Unknown model {slug} is used. This will use fallback model metadata.");
    ModelInfo {
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: Vec::new(),
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::None,
        supported_in_api: true,
        priority: 99,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        model_messages: local_personality_messages_for_slug(slug),
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        prefer_websockets: false,
    }
}

fn local_personality_messages_for_slug(slug: &str) -> Option<ModelMessages> {
    match slug {
        "gpt-5.2-codex" | "exp-codex-personality" => Some(ModelMessages {
            instructions_template: Some(format!(
                "{DEFAULT_PERSONALITY_HEADER}\n\n{PERSONALITY_PLACEHOLDER}\n\n{BASE_INSTRUCTIONS}"
            )),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: Some(String::new()),
                personality_friendly: Some(LOCAL_FRIENDLY_TEMPLATE.to_string()),
                personality_pragmatic: Some(LOCAL_PRAGMATIC_TEMPLATE.to_string()),
            }),
        }),
        _ => None,
    }
}
