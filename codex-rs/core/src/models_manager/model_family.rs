use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelFamily;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningSummaryFormat;
use codex_protocol::openai_models::TruncationPolicy;

use crate::config::Config;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../../prompt.md");

const GPT_5_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt_5_codex_prompt.md");
const GPT_5_1_INSTRUCTIONS: &str = include_str!("../../gpt_5_1_prompt.md");
const GPT_5_2_INSTRUCTIONS: &str = include_str!("../../gpt_5_2_prompt.md");
const GPT_5_1_CODEX_MAX_INSTRUCTIONS: &str = include_str!("../../gpt-5.1-codex-max_prompt.md");
const GPT_5_2_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt-5.2-codex_prompt.md");
pub(crate) const CONTEXT_WINDOW_272K: i64 = 272_000;

pub fn with_config_overrides(mut mf: ModelFamily, config: &Config) -> ModelFamily {
    if let Some(supports_reasoning_summaries) = config.model_supports_reasoning_summaries {
        mf.supports_reasoning_summaries = supports_reasoning_summaries;
    }
    if let Some(reasoning_summary_format) = config.model_reasoning_summary_format.as_ref() {
        mf.reasoning_summary_format = reasoning_summary_format.clone();
    }
    if let Some(context_window) = config.model_context_window {
        mf.context_window = Some(context_window);
    }
    if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
        mf.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    mf
}

macro_rules! model_family {
    (
        $slug:expr, $family:expr $(, $key:ident : $value:expr )* $(,)?
    ) => {{
        // defaults
        #[allow(unused_mut)]
        let mut mf = ModelFamily {
            slug: $slug.to_string(),
            family: $family.to_string(),
            needs_special_apply_patch_instructions: false,
            context_window: Some(CONTEXT_WINDOW_272K),
            auto_compact_token_limit: None,
            supports_reasoning_summaries: false,
            reasoning_summary_format: ReasoningSummaryFormat::None,
            supports_parallel_tool_calls: false,
            apply_patch_tool_type: None,
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            experimental_supported_tools: Vec::new(),
            effective_context_window_percent: 95,
            support_verbosity: false,
            shell_type: ConfigShellToolType::Default,
            default_verbosity: None,
            default_reasoning_effort: None,
            truncation_policy: TruncationPolicy::Bytes(10_000),
        };

        // apply overrides
        $(
            mf.$key = $value;
        )*
        mf
    }};
}

/// Internal offline helper for `ModelsManager` that returns a `ModelFamily` for the given
/// model slug.
#[allow(clippy::if_same_then_else)]
pub(super) fn find_family_for_model(slug: &str) -> ModelFamily {
    if slug.starts_with("o3") {
        model_family!(
            slug, "o3",
            supports_reasoning_summaries: true,
            needs_special_apply_patch_instructions: true,
            context_window: Some(200_000),
        )
    } else if slug.starts_with("o4-mini") {
        model_family!(
            slug, "o4-mini",
            supports_reasoning_summaries: true,
            needs_special_apply_patch_instructions: true,
            context_window: Some(200_000),
        )
    } else if slug.starts_with("codex-mini-latest") {
        model_family!(
            slug, "codex-mini-latest",
            supports_reasoning_summaries: true,
            needs_special_apply_patch_instructions: true,
            shell_type: ConfigShellToolType::Local,
            context_window: Some(200_000),
        )
    } else if slug.starts_with("gpt-4.1") {
        model_family!(
            slug, "gpt-4.1",
            needs_special_apply_patch_instructions: true,
            context_window: Some(1_047_576),
        )
    } else if slug.starts_with("gpt-oss") || slug.starts_with("openai/gpt-oss") {
        model_family!(
            slug, "gpt-oss",
            apply_patch_tool_type: Some(ApplyPatchToolType::Function),
            context_window: Some(96_000),
        )
    } else if slug.starts_with("gpt-4o") {
        model_family!(
            slug, "gpt-4o",
            needs_special_apply_patch_instructions: true,
            context_window: Some(128_000),
        )
    } else if slug.starts_with("gpt-3.5") {
        model_family!(
            slug, "gpt-3.5",
            needs_special_apply_patch_instructions: true,
            context_window: Some(16_385),
        )
    } else if slug.starts_with("test-gpt-5") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string(),
            experimental_supported_tools: vec![
                "grep_files".to_string(),
                "list_dir".to_string(),
                "read_file".to_string(),
                "test_sync_tool".to_string(),
            ],
            supports_parallel_tool_calls: true,
            shell_type: ConfigShellToolType::ShellCommand,
            support_verbosity: true,
            truncation_policy: TruncationPolicy::Tokens(10_000),
        )

    // Experimental models.
    } else if slug.starts_with("exp-codex") || slug.starts_with("codex-1p") {
        // Same as gpt-5.1-codex-max.
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("exp-") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: BASE_INSTRUCTIONS.to_string(),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicy::Bytes(10_000),
            shell_type: ConfigShellToolType::UnifiedExec,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
        )

    // Production models.
    } else if slug.starts_with("gpt-5.2-codex") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("bengalfox") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_2_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            support_verbosity: false,
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5.1-codex-max") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_1_CODEX_MAX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: false,
            support_verbosity: false,
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5-codex")
        || slug.starts_with("gpt-5.1-codex")
        || slug.starts_with("codex-")
    {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string(),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: false,
            support_verbosity: false,
            truncation_policy: TruncationPolicy::Tokens(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5.2") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: GPT_5_2_INSTRUCTIONS.to_string(),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicy::Bytes(10_000),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("boomslang") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: GPT_5_2_INSTRUCTIONS.to_string(),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicy::Bytes(10_000),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5.1") {
        model_family!(
            slug, "gpt-5.1",
            supports_reasoning_summaries: true,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            support_verbosity: true,
            default_verbosity: Some(Verbosity::Low),
            base_instructions: GPT_5_1_INSTRUCTIONS.to_string(),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            truncation_policy: TruncationPolicy::Bytes(10_000),
            shell_type: ConfigShellToolType::ShellCommand,
            supports_parallel_tool_calls: true,
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else if slug.starts_with("gpt-5") {
        model_family!(
            slug, "gpt-5",
            supports_reasoning_summaries: true,
            needs_special_apply_patch_instructions: true,
            shell_type: ConfigShellToolType::Default,
            support_verbosity: true,
            truncation_policy: TruncationPolicy::Bytes(10_000),
            context_window: Some(CONTEXT_WINDOW_272K),
        )
    } else {
        derive_default_model_family(slug)
    }
}

fn derive_default_model_family(model: &str) -> ModelFamily {
    tracing::warn!("Unknown model {model} is used. This will degrade the performance of Codex.");
    ModelFamily {
        slug: model.to_string(),
        family: model.to_string(),
        needs_special_apply_patch_instructions: false,
        context_window: None,
        auto_compact_token_limit: None,
        supports_reasoning_summaries: false,
        reasoning_summary_format: ReasoningSummaryFormat::None,
        supports_parallel_tool_calls: false,
        apply_patch_tool_type: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        experimental_supported_tools: Vec::new(),
        effective_context_window_percent: 95,
        support_verbosity: false,
        shell_type: ConfigShellToolType::Default,
        default_verbosity: None,
        default_reasoning_effort: None,
        truncation_policy: TruncationPolicy::Bytes(10_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::openai_models::ModelInfo;
    use codex_protocol::openai_models::ModelVisibility;
    use codex_protocol::openai_models::ReasoningEffortPreset;
    use codex_protocol::openai_models::TruncationPolicyConfig;

    fn remote(slug: &str, effort: ReasoningEffort, shell: ConfigShellToolType) -> ModelInfo {
        ModelInfo {
            slug: slug.to_string(),
            display_name: slug.to_string(),
            description: Some(format!("{slug} desc")),
            default_reasoning_level: effort,
            supported_reasoning_levels: vec![ReasoningEffortPreset {
                effort,
                description: effort.to_string(),
            }],
            shell_type: shell,
            visibility: ModelVisibility::List,
            supported_in_api: true,
            priority: 1,
            upgrade: None,
            base_instructions: None,
            supports_reasoning_summaries: false,
            support_verbosity: false,
            default_verbosity: None,
            apply_patch_tool_type: None,
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            supports_parallel_tool_calls: false,
            context_window: None,
            reasoning_summary_format: ReasoningSummaryFormat::None,
            experimental_supported_tools: Vec::new(),
        }
    }

    #[test]
    fn remote_overrides_apply_when_slug_matches() {
        let family = model_family!("gpt-4o-mini", "gpt-4o-mini");
        assert_ne!(family.default_reasoning_effort, Some(ReasoningEffort::High));

        let updated = family.with_remote_overrides(vec![
            remote(
                "gpt-4o-mini",
                ReasoningEffort::High,
                ConfigShellToolType::ShellCommand,
            ),
            remote(
                "other-model",
                ReasoningEffort::Low,
                ConfigShellToolType::UnifiedExec,
            ),
        ]);

        assert_eq!(
            updated.default_reasoning_effort,
            Some(ReasoningEffort::High)
        );
        assert_eq!(updated.shell_type, ConfigShellToolType::ShellCommand);
    }

    #[test]
    fn remote_overrides_skip_non_matching_models() {
        let family = model_family!(
            "codex-mini-latest",
            "codex-mini-latest",
            shell_type: ConfigShellToolType::Local
        );

        let updated = family.clone().with_remote_overrides(vec![remote(
            "other",
            ReasoningEffort::High,
            ConfigShellToolType::ShellCommand,
        )]);

        assert_eq!(
            updated.default_reasoning_effort,
            family.default_reasoning_effort
        );
        assert_eq!(updated.shell_type, family.shell_type);
    }

    #[test]
    fn remote_overrides_apply_extended_metadata() {
        let family = model_family!(
            "gpt-5.1",
            "gpt-5.1",
            supports_reasoning_summaries: false,
            support_verbosity: false,
            default_verbosity: None,
            apply_patch_tool_type: Some(ApplyPatchToolType::Function),
            supports_parallel_tool_calls: false,
            experimental_supported_tools: vec!["local".to_string()],
            truncation_policy: TruncationPolicy::Bytes(10_000),
            context_window: Some(100),
            reasoning_summary_format: ReasoningSummaryFormat::None,
        );

        let updated = family.with_remote_overrides(vec![ModelInfo {
            slug: "gpt-5.1".to_string(),
            display_name: "gpt-5.1".to_string(),
            description: Some("desc".to_string()),
            default_reasoning_level: ReasoningEffort::High,
            supported_reasoning_levels: vec![ReasoningEffortPreset {
                effort: ReasoningEffort::High,
                description: "High".to_string(),
            }],
            shell_type: ConfigShellToolType::ShellCommand,
            visibility: ModelVisibility::List,
            supported_in_api: true,
            priority: 10,
            upgrade: None,
            base_instructions: Some("Remote instructions".to_string()),
            supports_reasoning_summaries: true,
            support_verbosity: true,
            default_verbosity: Some(Verbosity::High),
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
            truncation_policy: TruncationPolicyConfig::tokens(2_000),
            supports_parallel_tool_calls: true,
            context_window: Some(400_000),
            reasoning_summary_format: ReasoningSummaryFormat::Experimental,
            experimental_supported_tools: vec!["alpha".to_string(), "beta".to_string()],
        }]);

        assert_eq!(
            updated.default_reasoning_effort,
            Some(ReasoningEffort::High)
        );
        assert!(updated.supports_reasoning_summaries);
        assert!(updated.support_verbosity);
        assert_eq!(updated.default_verbosity, Some(Verbosity::High));
        assert_eq!(updated.shell_type, ConfigShellToolType::ShellCommand);
        assert_eq!(
            updated.apply_patch_tool_type,
            Some(ApplyPatchToolType::Freeform)
        );
        assert_eq!(updated.truncation_policy, TruncationPolicy::Tokens(2_000));
        assert!(updated.supports_parallel_tool_calls);
        assert_eq!(updated.context_window, Some(400_000));
        assert_eq!(
            updated.reasoning_summary_format,
            ReasoningSummaryFormat::Experimental
        );
        assert_eq!(
            updated.experimental_supported_tools,
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(updated.base_instructions, "Remote instructions");
    }
}
