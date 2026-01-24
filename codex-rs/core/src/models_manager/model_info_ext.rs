//! Model info extensions for downstream features
//!
//! This module defines model info for models with downstream-specific
//! features (e.g., Gemini models with smart edit support).

use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationPolicyConfig;

const GEMINI_INSTRUCTIONS: &str = include_str!("../../gemini_prompt.md");

const GEMINI_CONTEXT_WINDOW_300K: i64 = 300_000;

/// Find model info for Gemini models
pub fn find_gemini_model_info(slug: &str) -> ModelInfo {
    if slug.starts_with("gemini-3-pro") {
        gemini_3_0_pro()
    } else if slug.starts_with("gemini-3-flash") {
        gemini_3_0_flash()
    } else {
        gemini_default(slug)
    }
}

/// Gemini 3.0 Pro - Smart Edit optimized
fn gemini_3_0_pro() -> ModelInfo {
    ModelInfo {
        slug: "gemini-3-pro".to_string(),
        display_name: "Gemini 3 Pro".to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: vec![],
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 0,
        upgrade: None,
        base_instructions: GEMINI_INSTRUCTIONS.to_string(),
        model_instructions_template: None,
        supports_reasoning_summaries: true,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::tokens(10_000),
        supports_parallel_tool_calls: true,
        context_window: Some(GEMINI_CONTEXT_WINDOW_300K),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: vec![],
    }
}

/// Gemini 3.0 Flash - Same capabilities as Pro
fn gemini_3_0_flash() -> ModelInfo {
    ModelInfo {
        slug: "gemini-3-flash".to_string(),
        display_name: "Gemini 3 Flash".to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: vec![],
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 0,
        upgrade: None,
        base_instructions: GEMINI_INSTRUCTIONS.to_string(),
        model_instructions_template: None,
        supports_reasoning_summaries: true,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::tokens(10_000),
        supports_parallel_tool_calls: true,
        context_window: Some(GEMINI_CONTEXT_WINDOW_300K),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: vec![],
    }
}

/// Default Gemini model - For unknown Gemini variants
fn gemini_default(slug: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: "Gemini".to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: vec![],
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 0,
        upgrade: None,
        base_instructions: GEMINI_INSTRUCTIONS.to_string(),
        model_instructions_template: None,
        supports_reasoning_summaries: true,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::tokens(10_000),
        supports_parallel_tool_calls: true,
        context_window: Some(GEMINI_CONTEXT_WINDOW_300K),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: vec![],
    }
}
