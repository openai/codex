use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;

use crate::config::Config;
use crate::features::Feature;
use crate::truncate::approx_bytes_for_tokens;
use tracing::warn;

pub const BASE_INSTRUCTIONS: &str = include_str!("../../prompt.md");
const BASE_INSTRUCTIONS_WITH_APPLY_PATCH: &str =
    include_str!("../../prompt_with_apply_patch_instructions.md");

const GPT_5_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt_5_codex_prompt.md");
const GPT_5_1_INSTRUCTIONS: &str = include_str!("../../gpt_5_1_prompt.md");
const GPT_5_2_INSTRUCTIONS: &str = include_str!("../../gpt_5_2_prompt.md");
const GPT_5_1_CODEX_MAX_INSTRUCTIONS: &str = include_str!("../../gpt-5.1-codex-max_prompt.md");

const GPT_5_2_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt-5.2-codex_prompt.md");
const GPT_5_2_CODEX_INSTRUCTIONS_TEMPLATE: &str =
    include_str!("../../templates/model_instructions/gpt-5.2-codex_instructions_template.md");

const GPT_5_2_CODEX_PERSONALITY_FRIENDLY: &str =
    include_str!("../../templates/personalities/gpt-5.2-codex_friendly.md");
const GPT_5_2_CODEX_PERSONALITY_PRAGMATIC: &str =
    include_str!("../../templates/personalities/gpt-5.2-codex_pragmatic.md");

pub(crate) const CONTEXT_WINDOW_272K: i64 = 272_000;

macro_rules! model_info {
    (
        $slug:expr $(, $key:ident : $value:expr )* $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut model = ModelInfo {
            slug: $slug.to_string(),
            display_name: $slug.to_string(),
            description: None,
            // This is primarily used when remote metadata is available. When running
            // offline, core generally omits the effort field unless explicitly
            // configured by the user.
            default_reasoning_level: None,
            supported_reasoning_levels: supported_reasoning_level_low_medium_high(),
            shell_type: ConfigShellToolType::Default,
            visibility: ModelVisibility::None,
            supported_in_api: true,
            priority: 99,
            upgrade: None,
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            model_messages: None,
            supports_reasoning_summaries: false,
            support_verbosity: false,
            default_verbosity: None,
            apply_patch_tool_type: None,
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            supports_parallel_tool_calls: false,
            context_window: Some(CONTEXT_WINDOW_272K),
            auto_compact_token_limit: None,
            effective_context_window_percent: 95,
            experimental_supported_tools: Vec::new(),
        };

        $(
            model.$key = $value;
        )*
        model
    }};
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

// todo(aibrahim): remove most of the entries here when enabling models.json
pub(crate) fn find_model_info_for_slug(slug: &str) -> ModelInfo {
    if slug.starts_with("o3") || slug.starts_with("o4-mini") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            supports_reasoning_summaries: true,
            context_window: Some(200_000),
        )
    } else if slug.starts_with("codex-mini-latest") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            shell_type: ConfigShellToolType::Local,
            supports_reasoning_summaries: true,
            context_window: Some(200_000),
        )
    } else if slug.starts_with("gpt-4.1") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            supports_reasoning_summaries: false,
            context_window: Some(1_047_576),
        )
    } else if slug.starts_with("gpt-oss") || slug.starts_with("openai/gpt-oss") {
        model_info!(
            slug,
            apply_patch_tool_type: Some(ApplyPatchToolType::Function),
            context_window: Some(96_000),
        )
    } else if slug.starts_with("gpt-4o") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            supports_reasoning_summaries: false,
            context_window: Some(128_000),
        )
    } else if slug.starts_with("gpt-3.5") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            supports_reasoning_summaries: false,
            context_window: Some(16_385),
        )
    } else if slug.starts_with("test-gpt-5") {
        model_info!(
            slug,
            base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string(),
            experimental_supported_tools: vec![
                "grep_files".to_string(),
                "list_dir".to_string(),
                "read_file".to_string(),
                "test_sync_tool".to_string(),
            ],
            supports_parallel_tool_calls: true,
            supports_reasoning_summaries: true,
            shell_type: ConfigShellToolType::ShellCommand,
            support_verbosity: true,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
        )
    } else if slug.starts_with("exp-codex") || slug.starts_with("codex-1p") {
        model_info!(
            slug,
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            model_messages: Some(ModelMessages {
                instructions_template: Some(GPT_5_2_CODEX_INSTRUCTIONS_TEMPLATE.to_string()),
                instructions_variables: Some(ModelInstructionsVariables {
                    personality_default: Some("".to_string()),
                    personality_friendly: Some(GPT_5_2_CODEX_PERSONALITY_FRIENDLY.to_string()),
                    personality_pragmatic: Some(GPT_5_2_CODEX_PERSONALITY_PRAGMATIC.to_string()),
                }),
            }),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            supports_reasoning_summaries: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("exp-") {
        model_info!(
            slug,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            supports_reasoning_summaries: true,
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            default_reasoning_level: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            shell_type: ConfigShellToolType::UnifiedExec,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5.2-codex") || slug.starts_with("bengalfox") {
        model_info!(
            slug,
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            supports_reasoning_summaries: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
            supported_reasoning_levels: supported_reasoning_level_low_medium_high_xhigh(),
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            model_messages: Some(ModelMessages {
                instructions_template: Some(GPT_5_2_CODEX_INSTRUCTIONS_TEMPLATE.to_string()),
                instructions_variables: Some(ModelInstructionsVariables {
                    personality_default: Some("".to_string()),
                    personality_friendly: Some(GPT_5_2_CODEX_PERSONALITY_FRIENDLY.to_string()),
                    personality_pragmatic: Some(GPT_5_2_CODEX_PERSONALITY_PRAGMATIC.to_string()),
                }),
            }),
        )
    } else if slug.starts_with("gpt-5.1-codex-max") {
        model_info!(
            slug,
            base_instructions: GPT_5_1_CODEX_MAX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: false,
            supports_reasoning_summaries: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
            supported_reasoning_levels: supported_reasoning_level_low_medium_high_xhigh(),
        )
    } else if (slug.starts_with("gpt-5-codex")
        || slug.starts_with("gpt-5.1-codex")
        || slug.starts_with("codex-"))
        && !slug.contains("-mini")
    {
        model_info!(
            slug,
            base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: false,
            supports_reasoning_summaries: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
            supported_reasoning_levels: supported_reasoning_level_low_medium_high(),
        )
    } else if slug.starts_with("gpt-5-codex")
        || slug.starts_with("gpt-5.1-codex")
        || slug.starts_with("codex-")
    {
        model_info!(
            slug,
            base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: false,
            supports_reasoning_summaries: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicyConfig::tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5.2") || slug.starts_with("boomslang") {
        model_info!(
            slug,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            supports_reasoning_summaries: true,
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: GPT_5_2_INSTRUCTIONS.to_string(),
            default_reasoning_level: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
            supported_reasoning_levels: supported_reasoning_level_low_medium_high_xhigh_non_codex(),
        )
    } else if slug.starts_with("gpt-5.1") {
        model_info!(
            slug,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            supports_reasoning_summaries: true,
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: GPT_5_1_INSTRUCTIONS.to_string(),
            default_reasoning_level: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
            supported_reasoning_levels: supported_reasoning_level_low_medium_high_non_codex(),
        )
    } else if slug.starts_with("gpt-5") {
        model_info!(
            slug,
            base_instructions: BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string(),
            shell_type: ConfigShellToolType::Default,
            supports_reasoning_summaries: true,
            support_verbosity: true,
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else {
        warn!("Unknown model {slug} is used. This will degrade the performance of Codex.");
        model_info!(
            slug,
            context_window: None,
            supported_reasoning_levels: Vec::new(),
            default_reasoning_level: None
        )
    }
}

fn supported_reasoning_level_low_medium_high() -> Vec<ReasoningEffortPreset> {
    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Fast responses with lighter reasoning".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Balances speed and reasoning depth for everyday tasks".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Greater reasoning depth for complex problems".to_string(),
        },
    ]
}

fn supported_reasoning_level_low_medium_high_non_codex() -> Vec<ReasoningEffortPreset> {
    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Balances speed with some reasoning; useful for straightforward queries and short explanations".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
        },
    ]
}

fn supported_reasoning_level_low_medium_high_xhigh() -> Vec<ReasoningEffortPreset> {
    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Fast responses with lighter reasoning".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Balances speed and reasoning depth for everyday tasks".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Greater reasoning depth for complex problems".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::XHigh,
            description: "Extra high reasoning depth for complex problems".to_string(),
        },
    ]
}

fn supported_reasoning_level_low_medium_high_xhigh_non_codex() -> Vec<ReasoningEffortPreset> {
    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Balances speed with some reasoning; useful for straightforward queries and short explanations".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::XHigh,
            description: "Extra high reasoning for complex problems".to_string(),
        },
    ]
}
