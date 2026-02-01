//! ThinkingLevel to provider-specific options conversion.
//!
//! This module provides conversion functions to translate the unified
//! `ThinkingLevel` from `cocode_protocol` to provider-specific options
//! for each supported provider.
//!
//! # Design Principle
//!
//! Each provider uses its native thinking parameters:
//! - **Budget-based providers** (Anthropic, Volcengine, Z.AI): Use `budget_tokens`
//! - **Effort-based providers** (OpenAI, Gemini): Use reasoning effort level
//!
//! Additionally, per-model settings from `ModelInfo` are applied:
//! - `reasoning_summary`: For OpenAI models (none/auto/concise/detailed)
//! - `include_thoughts`: For Gemini models (defaults to true when thinking enabled)
//! - `include_encrypted_content`: Always true when thinking is enabled for OpenAI
//!
//! # Example
//!
//! ```ignore
//! use cocode_api::thinking_convert::to_provider_options;
//! use cocode_protocol::{ThinkingLevel, ProviderType, ModelInfo};
//!
//! let level = ThinkingLevel::high().set_budget(32000);
//! let model_info = ModelInfo::default();
//! let opts = to_provider_options(&level, &model_info, ProviderType::Anthropic);
//! assert!(opts.is_some());
//! ```

use cocode_protocol::ModelInfo;
use cocode_protocol::ProviderType;
use cocode_protocol::ThinkingLevel;
use cocode_protocol::model::ReasoningEffort;
use cocode_protocol::model::ReasoningSummary;
use hyper_sdk::AnthropicOptions;
use hyper_sdk::GeminiOptions;
use hyper_sdk::OpenAIOptions;
use hyper_sdk::VolcengineOptions;
use hyper_sdk::ZaiOptions;
use hyper_sdk::options::ProviderOptions;

/// Convert ThinkingLevel and ModelInfo to provider-specific options.
///
/// Returns `None` if:
/// - Thinking is disabled (`effort == None`)
/// - The provider doesn't support the specified thinking configuration
///
/// # Provider-Specific Behavior
///
/// | Provider    | Primary Config         | From ModelInfo        |
/// |-------------|------------------------|-----------------------|
/// | Anthropic   | `budget_tokens`        | (none)                |
/// | OpenAI      | `effort` -> reasoning  | `reasoning_summary`, encrypted content always |
/// | Gemini      | `effort` -> level      | `include_thoughts`    |
/// | Volcengine  | `budget_tokens` + `effort` | (none)            |
/// | Z.AI        | `budget_tokens`        | (none)                |
pub fn to_provider_options(
    level: &ThinkingLevel,
    model_info: &ModelInfo,
    provider: ProviderType,
) -> Option<ProviderOptions> {
    // If thinking is disabled, return None
    if !level.is_enabled() {
        return None;
    }

    match provider {
        ProviderType::Anthropic => to_anthropic_options(level),
        ProviderType::Openai | ProviderType::OpenaiCompat => to_openai_options(level, model_info),
        ProviderType::Gemini => to_gemini_options(level, model_info),
        ProviderType::Volcengine => to_volcengine_options(level),
        ProviderType::Zai => to_zai_options(level),
    }
}

/// Convert to Anthropic options.
///
/// Anthropic uses `budget_tokens` for extended thinking.
/// Returns `None` if `budget_tokens` is not set.
fn to_anthropic_options(level: &ThinkingLevel) -> Option<ProviderOptions> {
    level
        .budget_tokens
        .map(|budget| AnthropicOptions::new().with_thinking_budget(budget).boxed())
}

/// Convert to OpenAI options.
///
/// OpenAI uses reasoning effort levels (Low/Medium/High).
/// Also applies reasoning_summary from ModelInfo and always enables encrypted content.
fn to_openai_options(level: &ThinkingLevel, model_info: &ModelInfo) -> Option<ProviderOptions> {
    let effort = map_to_openai_effort(&level.effort)?;

    let mut opts = OpenAIOptions::new().with_reasoning_effort(effort);

    // Apply reasoning summary from ModelInfo
    if let Some(summary) = &model_info.reasoning_summary {
        if let Some(oai_summary) = map_to_openai_summary(summary) {
            opts = opts.with_reasoning_summary(oai_summary);
        }
    }

    // Always include encrypted content when thinking is enabled
    opts = opts.with_include_encrypted_content(true);

    Some(opts.boxed())
}

/// Convert to Gemini options.
///
/// Gemini uses thinking levels (None/Low/Medium/High).
/// Also applies include_thoughts from ModelInfo (defaults to true when thinking enabled).
fn to_gemini_options(level: &ThinkingLevel, model_info: &ModelInfo) -> Option<ProviderOptions> {
    let gem_level = map_to_gemini_level(&level.effort);

    // Only return options if thinking is enabled
    if gem_level != hyper_sdk::options::gemini::ThinkingLevel::None {
        let mut opts = GeminiOptions::new().with_thinking_level(gem_level);

        // Apply include_thoughts from ModelInfo (default true when thinking enabled)
        let include = model_info.include_thoughts.unwrap_or(true);
        opts = opts.with_include_thoughts(include);

        Some(opts.boxed())
    } else {
        None
    }
}

/// Convert to Volcengine options.
///
/// Volcengine supports both `budget_tokens` and `reasoning_effort`.
fn to_volcengine_options(level: &ThinkingLevel) -> Option<ProviderOptions> {
    let mut opts = VolcengineOptions::new();
    let mut has_thinking = false;

    if let Some(budget) = level.budget_tokens {
        opts = opts.with_thinking_budget(budget);
        has_thinking = true;
    }

    if let Some(effort) = map_to_volcengine_effort(&level.effort) {
        opts = opts.with_reasoning_effort(effort);
        has_thinking = true;
    }

    if has_thinking {
        Some(opts.boxed())
    } else {
        None
    }
}

/// Convert to Z.AI options.
///
/// Z.AI uses `budget_tokens` for extended thinking.
/// Returns `None` if `budget_tokens` is not set.
fn to_zai_options(level: &ThinkingLevel) -> Option<ProviderOptions> {
    level
        .budget_tokens
        .map(|budget| ZaiOptions::new().with_thinking_budget(budget).boxed())
}

// =============================================================================
// Effort Level Mappings
// =============================================================================

/// Map protocol ReasoningSummary to hyper-sdk OpenAI ReasoningSummary.
fn map_to_openai_summary(
    summary: &ReasoningSummary,
) -> Option<hyper_sdk::options::openai::ReasoningSummary> {
    use hyper_sdk::options::openai::ReasoningSummary as OaiSummary;

    match summary {
        ReasoningSummary::None => None, // No summary
        ReasoningSummary::Auto => Some(OaiSummary::Auto),
        ReasoningSummary::Concise => Some(OaiSummary::Concise),
        ReasoningSummary::Detailed => Some(OaiSummary::Detailed),
    }
}

/// Map ReasoningEffort to OpenAI's ReasoningEffort.
fn map_to_openai_effort(
    effort: &ReasoningEffort,
) -> Option<hyper_sdk::options::openai::ReasoningEffort> {
    use hyper_sdk::options::openai::ReasoningEffort as OE;

    match effort {
        ReasoningEffort::None => None,
        ReasoningEffort::Minimal | ReasoningEffort::Low => Some(OE::Low),
        ReasoningEffort::Medium => Some(OE::Medium),
        ReasoningEffort::High | ReasoningEffort::XHigh => Some(OE::High),
    }
}

/// Map ReasoningEffort to Gemini's ThinkingLevel.
fn map_to_gemini_level(effort: &ReasoningEffort) -> hyper_sdk::options::gemini::ThinkingLevel {
    use hyper_sdk::options::gemini::ThinkingLevel as GL;

    match effort {
        ReasoningEffort::None => GL::None,
        ReasoningEffort::Minimal | ReasoningEffort::Low => GL::Low,
        ReasoningEffort::Medium => GL::Medium,
        ReasoningEffort::High | ReasoningEffort::XHigh => GL::High,
    }
}

/// Map ReasoningEffort to Volcengine's ReasoningEffort.
fn map_to_volcengine_effort(
    effort: &ReasoningEffort,
) -> Option<hyper_sdk::options::volcengine::ReasoningEffort> {
    use hyper_sdk::options::volcengine::ReasoningEffort as VE;

    match effort {
        ReasoningEffort::None => None,
        ReasoningEffort::Minimal => Some(VE::Minimal),
        ReasoningEffort::Low => Some(VE::Low),
        ReasoningEffort::Medium => Some(VE::Medium),
        ReasoningEffort::High | ReasoningEffort::XHigh => Some(VE::High),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper_sdk::options::downcast_options;

    fn default_model_info() -> ModelInfo {
        ModelInfo::default()
    }

    #[test]
    fn test_to_anthropic_options_with_budget() {
        let level = ThinkingLevel::high().set_budget(32000);
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Anthropic);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let ant_opts = downcast_options::<AnthropicOptions>(&opts).unwrap();
        assert_eq!(ant_opts.thinking_budget_tokens, Some(32000));
    }

    #[test]
    fn test_to_anthropic_options_no_budget() {
        // Anthropic requires budget_tokens, so effort alone returns None
        let level = ThinkingLevel::high();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Anthropic);

        assert!(opts.is_none());
    }

    #[test]
    fn test_to_openai_options_high() {
        let level = ThinkingLevel::high();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_effort,
            Some(hyper_sdk::options::openai::ReasoningEffort::High)
        );
        // Should always include encrypted content
        assert_eq!(openai_opts.include_encrypted_content, Some(true));
    }

    #[test]
    fn test_to_openai_options_medium() {
        let level = ThinkingLevel::medium();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_effort,
            Some(hyper_sdk::options::openai::ReasoningEffort::Medium)
        );
    }

    #[test]
    fn test_to_openai_options_low() {
        let level = ThinkingLevel::low();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_effort,
            Some(hyper_sdk::options::openai::ReasoningEffort::Low)
        );
    }

    #[test]
    fn test_to_openai_options_none() {
        let level = ThinkingLevel::none();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai);

        assert!(opts.is_none());
    }

    #[test]
    fn test_to_openai_options_with_reasoning_summary() {
        let level = ThinkingLevel::high();
        let mut model_info = default_model_info();
        model_info.reasoning_summary = Some(ReasoningSummary::Detailed);

        let opts = to_provider_options(&level, &model_info, ProviderType::Openai);
        assert!(opts.is_some());
        let opts = opts.unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_summary,
            Some(hyper_sdk::options::openai::ReasoningSummary::Detailed)
        );
    }

    #[test]
    fn test_to_gemini_options_high() {
        let level = ThinkingLevel::high();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Gemini);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let gem_opts = downcast_options::<GeminiOptions>(&opts).unwrap();
        assert_eq!(
            gem_opts.thinking_level,
            Some(hyper_sdk::options::gemini::ThinkingLevel::High)
        );
        // Default include_thoughts is true
        assert_eq!(gem_opts.include_thoughts, Some(true));
    }

    #[test]
    fn test_to_gemini_options_none() {
        let level = ThinkingLevel::none();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Gemini);

        assert!(opts.is_none());
    }

    #[test]
    fn test_to_gemini_options_include_thoughts_false() {
        let level = ThinkingLevel::high();
        let mut model_info = default_model_info();
        model_info.include_thoughts = Some(false);

        let opts = to_provider_options(&level, &model_info, ProviderType::Gemini);
        assert!(opts.is_some());
        let opts = opts.unwrap();
        let gem_opts = downcast_options::<GeminiOptions>(&opts).unwrap();
        assert_eq!(gem_opts.include_thoughts, Some(false));
    }

    #[test]
    fn test_to_volcengine_options_budget() {
        let level = ThinkingLevel::high().set_budget(16000);
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Volcengine);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let volc_opts = downcast_options::<VolcengineOptions>(&opts).unwrap();
        assert_eq!(volc_opts.thinking_budget_tokens, Some(16000));
        assert_eq!(
            volc_opts.reasoning_effort,
            Some(hyper_sdk::options::volcengine::ReasoningEffort::High)
        );
    }

    #[test]
    fn test_to_volcengine_options_effort_only() {
        let level = ThinkingLevel::medium();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Volcengine);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let volc_opts = downcast_options::<VolcengineOptions>(&opts).unwrap();
        assert!(volc_opts.thinking_budget_tokens.is_none());
        assert_eq!(
            volc_opts.reasoning_effort,
            Some(hyper_sdk::options::volcengine::ReasoningEffort::Medium)
        );
    }

    #[test]
    fn test_to_zai_options_with_budget() {
        let level = ThinkingLevel::high().set_budget(8192);
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Zai);

        assert!(opts.is_some());
        let opts = opts.unwrap();
        let zai_opts = downcast_options::<ZaiOptions>(&opts).unwrap();
        assert_eq!(zai_opts.thinking_budget_tokens, Some(8192));
    }

    #[test]
    fn test_to_zai_options_no_budget() {
        // Z.AI requires budget_tokens, so effort alone returns None
        let level = ThinkingLevel::high();
        let model_info = default_model_info();
        let opts = to_provider_options(&level, &model_info, ProviderType::Zai);

        assert!(opts.is_none());
    }

    #[test]
    fn test_xhigh_maps_to_high() {
        let level = ThinkingLevel::xhigh();
        let model_info = default_model_info();

        // OpenAI: XHigh -> High
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai).unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_effort,
            Some(hyper_sdk::options::openai::ReasoningEffort::High)
        );

        // Gemini: XHigh -> High
        let opts = to_provider_options(&level, &model_info, ProviderType::Gemini).unwrap();
        let gem_opts = downcast_options::<GeminiOptions>(&opts).unwrap();
        assert_eq!(
            gem_opts.thinking_level,
            Some(hyper_sdk::options::gemini::ThinkingLevel::High)
        );
    }

    #[test]
    fn test_minimal_maps_to_low() {
        let level = ThinkingLevel::new(ReasoningEffort::Minimal);
        let model_info = default_model_info();

        // OpenAI: Minimal -> Low
        let opts = to_provider_options(&level, &model_info, ProviderType::Openai).unwrap();
        let openai_opts = downcast_options::<OpenAIOptions>(&opts).unwrap();
        assert_eq!(
            openai_opts.reasoning_effort,
            Some(hyper_sdk::options::openai::ReasoningEffort::Low)
        );

        // Gemini: Minimal -> Low
        let opts = to_provider_options(&level, &model_info, ProviderType::Gemini).unwrap();
        let gem_opts = downcast_options::<GeminiOptions>(&opts).unwrap();
        assert_eq!(
            gem_opts.thinking_level,
            Some(hyper_sdk::options::gemini::ThinkingLevel::Low)
        );

        // Volcengine: Minimal is preserved
        let opts = to_provider_options(&level, &model_info, ProviderType::Volcengine).unwrap();
        let volc_opts = downcast_options::<VolcengineOptions>(&opts).unwrap();
        assert_eq!(
            volc_opts.reasoning_effort,
            Some(hyper_sdk::options::volcengine::ReasoningEffort::Minimal)
        );
    }
}
