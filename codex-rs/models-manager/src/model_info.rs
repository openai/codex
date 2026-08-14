use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::openai_models::default_input_modalities;

use crate::config::ModelsManagerConfig;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use tracing::warn;

pub const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");
const DEFAULT_PERSONALITY_HEADER: &str = "You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals.";
const LOCAL_FRIENDLY_TEMPLATE: &str =
    "You optimize for team morale and being a supportive teammate as much as code quality.";
const LOCAL_PRAGMATIC_TEMPLATE: &str = "You are a deeply pragmatic, effective software engineer.";
const PERSONALITY_PLACEHOLDER: &str = "{{ personality }}";
const PERSONALITY_SECTION_HEADER: &str = "# Personality";

pub fn with_config_overrides(mut model: ModelInfo, config: &ModelsManagerConfig) -> ModelInfo {
    if let Some(context_window) = config.model_context_window {
        model.context_window = Some(
            model
                .max_context_window
                .map_or(context_window, |max_context_window| {
                    context_window.min(max_context_window)
                }),
        );
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
        let model_messages = model.model_messages.get_or_insert(ModelMessages {
            instructions_template: None,
            instructions_variables: None,
            approvals: None,
            collaboration_modes: None,
            auto_review: None,
            permissions: None,
            token_budget: None,
        });
        model_messages.instructions_template = Some(base_instructions.clone());
        model_messages.instructions_variables = None;
    } else {
        if config.personality_enabled
            && config.personality == Some(Personality::None)
            && let Some(instructions_template) = model
                .model_messages
                .as_mut()
                .and_then(|messages| messages.instructions_template.as_mut())
        {
            *instructions_template =
                strip_personality_section(std::mem::take(instructions_template));
        }
        let uses_local_personality_template = model.used_fallback_model_metadata
            && matches!(
                model.slug.as_str(),
                "gpt-5.2-codex" | "exp-codex-personality"
            );
        if !config.personality_enabled
            && let Some(model_messages) = model.model_messages.as_mut()
        {
            if uses_local_personality_template {
                model_messages.instructions_template = Some(BASE_INSTRUCTIONS.to_string());
            } else {
                let personality_default = model_messages
                    .get_personality_message(/*personality*/ None)
                    .unwrap_or_default();
                if let Some(instructions_template) = model_messages.instructions_template.as_mut() {
                    *instructions_template = instructions_template
                        .replace(PERSONALITY_PLACEHOLDER, &personality_default);
                }
            }
            model_messages.instructions_variables = None;
        }
    }

    model
}

fn strip_personality_section(mut instructions: String) -> String {
    let mut section_start = None;
    let mut section_end = None;
    let mut offset = 0;

    for line_with_ending in instructions.split_inclusive('\n') {
        let line = match line_with_ending.strip_suffix('\n') {
            Some(line) => line.strip_suffix('\r').unwrap_or(line),
            None => line_with_ending,
        };
        if section_start.is_some() {
            if is_h1_heading(line) {
                section_end = Some(offset);
                break;
            }
        } else if line == PERSONALITY_SECTION_HEADER {
            section_start = Some(offset);
        }
        offset += line_with_ending.len();
    }

    if let Some(section_start) = section_start {
        let section_end = section_end.unwrap_or(instructions.len());
        instructions.replace_range(section_start..section_end, "");
    }

    instructions
}

fn is_h1_heading(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('#') else {
        return false;
    };
    rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')
}

/// Build a minimal fallback model descriptor for missing/unknown slugs.
pub fn model_info_from_slug(slug: &str) -> ModelInfo {
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
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        availability_nux: None,
        upgrade: None,
        model_messages: Some(local_model_messages_for_slug(slug)),
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: Some(272_000),
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: true, // this is the fallback model metadata
        supports_search_tool: false,
        use_responses_lite: false,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: None,
    }
}

fn local_model_messages_for_slug(slug: &str) -> ModelMessages {
    match slug {
        "gpt-5.2-codex" | "exp-codex-personality" => ModelMessages {
            instructions_template: Some(format!(
                "{DEFAULT_PERSONALITY_HEADER}\n\n{PERSONALITY_PLACEHOLDER}\n\n{BASE_INSTRUCTIONS}"
            )),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: Some(String::new()),
                personality_friendly: Some(LOCAL_FRIENDLY_TEMPLATE.to_string()),
                personality_pragmatic: Some(LOCAL_PRAGMATIC_TEMPLATE.to_string()),
            }),
            approvals: None,
            collaboration_modes: None,
            auto_review: None,
            permissions: None,
            token_budget: None,
        },
        _ => ModelMessages {
            instructions_template: Some(BASE_INSTRUCTIONS.to_string()),
            instructions_variables: None,
            approvals: None,
            collaboration_modes: None,
            auto_review: None,
            permissions: None,
            token_budget: None,
        },
    }
}

#[cfg(test)]
#[path = "model_info_tests.rs"]
mod tests;
