use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::TruncationPolicyConfig;

use crate::config::Config;

const BASE_INSTRUCTIONS: &str = include_str!("../../prompt.md");
const BASE_INSTRUCTIONS_WITH_APPLY_PATCH: &str =
    include_str!("../../prompt_with_apply_patch_instructions.md");

const GPT_5_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt_5_codex_prompt.md");
const GPT_5_1_INSTRUCTIONS: &str = include_str!("../../gpt_5_1_prompt.md");
const GPT_5_2_INSTRUCTIONS: &str = include_str!("../../gpt_5_2_prompt.md");
const GPT_5_1_CODEX_MAX_INSTRUCTIONS: &str = include_str!("../../gpt-5.1-codex-max_prompt.md");
const GPT_5_2_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt-5.2-codex_prompt.md");

pub(crate) const CONTEXT_WINDOW_272K: i64 = 272_000;

fn default_model_info(slug: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: None,
        // This is primarily used when remote metadata is available. When running
        // offline, core generally omits the effort field unless explicitly
        // configured by the user.
        default_reasoning_level: ReasoningEffort::Medium,
        supported_reasoning_levels: Vec::new(),
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::None,
        supported_in_api: true,
        priority: 0,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: CONTEXT_WINDOW_272K,
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}

pub(crate) fn with_config_overrides(mut model: ModelInfo, config: &Config) -> ModelInfo {
    if let Some(supports_reasoning_summaries) = config.model_supports_reasoning_summaries {
        model.supports_reasoning_summaries = supports_reasoning_summaries;
    }
    if let Some(context_window) = config.model_context_window {
        model.context_window = context_window;
    }
    if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
        model.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    model
}

pub(crate) fn merge_remote_overrides(mut model: ModelInfo, remote: Option<ModelInfo>) -> ModelInfo {
    let Some(remote) = remote else {
        return model;
    };

    // Remote metadata is authoritative for most fields, but some optional
    // fields should preserve locally-derived defaults when absent.
    let auto_compact_token_limit = remote
        .auto_compact_token_limit
        .or(model.auto_compact_token_limit);

    model = remote;
    model.auto_compact_token_limit = auto_compact_token_limit;
    model
}

// todo(aibrahim): remove most of the entries here when enabling models.json
pub(crate) fn find_model_info_for_slug(slug: &str) -> ModelInfo {
    let mut model = default_model_info(slug);

    if slug.starts_with("o3") || slug.starts_with("o4-mini") {
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.supports_reasoning_summaries = true;
        model.context_window = 200_000;
    } else if slug.starts_with("codex-mini-latest") {
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.supports_reasoning_summaries = true;
        model.shell_type = ConfigShellToolType::Local;
        model.context_window = 200_000;
    } else if slug.starts_with("gpt-4.1") {
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.context_window = 1_047_576;
    } else if slug.starts_with("gpt-oss") || slug.starts_with("openai/gpt-oss") {
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Function);
        model.context_window = 96_000;
    } else if slug.starts_with("gpt-4o") {
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.context_window = 128_000;
    } else if slug.starts_with("gpt-3.5") {
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.context_window = 16_385;
    } else if slug.starts_with("test-gpt-5") {
        model.supports_reasoning_summaries = true;
        model.base_instructions = GPT_5_CODEX_INSTRUCTIONS.to_string();
        model.experimental_supported_tools = vec![
            "grep_files".to_string(),
            "list_dir".to_string(),
            "read_file".to_string(),
            "test_sync_tool".to_string(),
        ];
        model.supports_parallel_tool_calls = true;
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.support_verbosity = true;
        model.truncation_policy = TruncationPolicyConfig::tokens(10_000);
    } else if slug.starts_with("exp-codex") || slug.starts_with("codex-1p") {
        model.supports_reasoning_summaries = true;
        model.base_instructions = GPT_5_2_CODEX_INSTRUCTIONS.to_string();
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = true;
        model.support_verbosity = false;
        model.truncation_policy = TruncationPolicyConfig::tokens(10_000);
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("exp-") {
        model.supports_reasoning_summaries = true;
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.support_verbosity = true;
        model.default_verbosity = Some(Verbosity::Low);
        model.base_instructions = BASE_INSTRUCTIONS.to_string();
        model.truncation_policy = TruncationPolicyConfig::bytes(10_000);
        model.shell_type = ConfigShellToolType::UnifiedExec;
        model.supports_parallel_tool_calls = true;
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5.2-codex") || slug.starts_with("bengalfox") {
        model.supports_reasoning_summaries = true;
        model.base_instructions = GPT_5_2_CODEX_INSTRUCTIONS.to_string();
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = true;
        model.support_verbosity = false;
        model.truncation_policy = TruncationPolicyConfig::tokens(10_000);
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5.1-codex-max") {
        model.supports_reasoning_summaries = true;
        model.base_instructions = GPT_5_1_CODEX_MAX_INSTRUCTIONS.to_string();
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = false;
        model.support_verbosity = false;
        model.truncation_policy = TruncationPolicyConfig::tokens(10_000);
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5-codex")
        || slug.starts_with("gpt-5.1-codex")
        || slug.starts_with("codex-")
    {
        model.supports_reasoning_summaries = true;
        model.base_instructions = GPT_5_CODEX_INSTRUCTIONS.to_string();
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = false;
        model.support_verbosity = false;
        model.truncation_policy = TruncationPolicyConfig::tokens(10_000);
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5.2") || slug.starts_with("boomslang") {
        model.supports_reasoning_summaries = true;
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.support_verbosity = true;
        model.default_verbosity = Some(Verbosity::Low);
        model.base_instructions = GPT_5_2_INSTRUCTIONS.to_string();
        model.truncation_policy = TruncationPolicyConfig::bytes(10_000);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = true;
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5.1") {
        model.supports_reasoning_summaries = true;
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
        model.support_verbosity = true;
        model.default_verbosity = Some(Verbosity::Low);
        model.base_instructions = GPT_5_1_INSTRUCTIONS.to_string();
        model.truncation_policy = TruncationPolicyConfig::bytes(10_000);
        model.shell_type = ConfigShellToolType::ShellCommand;
        model.supports_parallel_tool_calls = true;
        model.context_window = CONTEXT_WINDOW_272K;
    } else if slug.starts_with("gpt-5") {
        model.supports_reasoning_summaries = true;
        model.base_instructions = BASE_INSTRUCTIONS_WITH_APPLY_PATCH.to_string();
        model.shell_type = ConfigShellToolType::Default;
        model.support_verbosity = true;
        model.truncation_policy = TruncationPolicyConfig::bytes(10_000);
        model.context_window = CONTEXT_WINDOW_272K;
    }

    model
}
