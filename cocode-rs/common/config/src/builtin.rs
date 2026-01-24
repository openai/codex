//! Built-in model and provider defaults.
//!
//! This module provides default configurations for well-known models
//! that are compiled into the binary. These serve as the lowest-priority
//! layer in the configuration resolution.

use crate::types::ProviderConfig;
use crate::types::ProviderType;
use cocode_protocol::Capability;
use cocode_protocol::ConfigShellToolType;
use cocode_protocol::ModelInfo;
use cocode_protocol::ReasoningEffort;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Get built-in model defaults for a model ID.
///
/// Returns `None` if no built-in defaults exist for this model.
pub fn get_model_defaults(model_id: &str) -> Option<ModelInfo> {
    BUILTIN_MODELS.get().and_then(|m| m.get(model_id).cloned())
}

/// Get built-in provider defaults for a provider name.
///
/// Returns `None` if no built-in defaults exist for this provider.
pub fn get_provider_defaults(provider_name: &str) -> Option<ProviderConfig> {
    BUILTIN_PROVIDERS
        .get()
        .and_then(|p| p.get(provider_name).cloned())
}

/// Get all built-in model IDs.
pub fn list_builtin_models() -> Vec<&'static str> {
    BUILTIN_MODELS
        .get()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Get all built-in provider names.
pub fn list_builtin_providers() -> Vec<&'static str> {
    BUILTIN_PROVIDERS
        .get()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

// Lazily initialized built-in models
static BUILTIN_MODELS: OnceLock<HashMap<String, ModelInfo>> = OnceLock::new();
static BUILTIN_PROVIDERS: OnceLock<HashMap<String, ProviderConfig>> = OnceLock::new();

/// Initialize built-in defaults (called automatically on first access).
fn init_builtin_models() -> HashMap<String, ModelInfo> {
    let mut models = HashMap::new();

    // OpenAI GPT-5
    models.insert(
        "gpt-5".to_string(),
        ModelInfo {
            display_name: Some("GPT-5".to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(32000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::StructuredOutput,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            supported_reasoning_levels: Some(vec![
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
            ]),
            ..Default::default()
        },
    );

    // OpenAI GPT-5.2
    models.insert(
        "gpt-5.2".to_string(),
        ModelInfo {
            display_name: Some("GPT-5.2".to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(64000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ExtendedThinking,
                Capability::ReasoningSummaries,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            supported_reasoning_levels: Some(vec![
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
            ]),
            shell_type: Some(ConfigShellToolType::ShellCommand),
            ..Default::default()
        },
    );

    // OpenAI GPT-5.2 Codex (optimized for coding)
    models.insert(
        "gpt-5.2-codex".to_string(),
        ModelInfo {
            display_name: Some("GPT-5.2 Codex".to_string()),
            description: Some("GPT-5.2 optimized for coding tasks".to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(64000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ExtendedThinking,
                Capability::ReasoningSummaries,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_reasoning_effort: Some(ReasoningEffort::Medium),
            supported_reasoning_levels: Some(vec![
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
            ]),
            shell_type: Some(ConfigShellToolType::ShellCommand),
            ..Default::default()
        },
    );

    // Google Gemini 3 Pro
    models.insert(
        "gemini-3-pro".to_string(),
        ModelInfo {
            display_name: Some("Gemini 3 Pro".to_string()),
            context_window: Some(300000),
            max_output_tokens: Some(32000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(280000),
            effective_context_window_percent: Some(95),
            ..Default::default()
        },
    );

    // Google Gemini 3 Flash
    models.insert(
        "gemini-3-flash".to_string(),
        ModelInfo {
            display_name: Some("Gemini 3 Flash".to_string()),
            context_window: Some(300000),
            max_output_tokens: Some(16000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(280000),
            effective_context_window_percent: Some(95),
            ..Default::default()
        },
    );

    models
}

fn init_builtin_providers() -> HashMap<String, ProviderConfig> {
    let mut providers = HashMap::new();

    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            name: "OpenAI".to_string(),
            provider_type: ProviderType::Openai,
            env_key: Some("OPENAI_API_KEY".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            default_model: Some("gpt-5".to_string()),
            timeout_secs: Some(600),
            ..Default::default()
        },
    );

    providers.insert(
        "gemini".to_string(),
        ProviderConfig {
            name: "Google Gemini".to_string(),
            provider_type: ProviderType::Gemini,
            env_key: Some("GOOGLE_API_KEY".to_string()),
            base_url: Some("https://generativelanguage.googleapis.com".to_string()),
            default_model: Some("gemini-3-flash".to_string()),
            timeout_secs: Some(600),
            ..Default::default()
        },
    );

    providers
}

// Force initialization by accessing the locks
pub(crate) fn ensure_initialized() {
    let _ = BUILTIN_MODELS.get_or_init(init_builtin_models);
    let _ = BUILTIN_PROVIDERS.get_or_init(init_builtin_providers);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_defaults() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert_eq!(gpt5.display_name, Some("GPT-5".to_string()));
        assert_eq!(gpt5.context_window, Some(272000));

        let gemini = get_model_defaults("gemini-3-pro").unwrap();
        assert_eq!(gemini.display_name, Some("Gemini 3 Pro".to_string()));

        let unknown = get_model_defaults("unknown-model");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_get_provider_defaults() {
        ensure_initialized();

        let openai = get_provider_defaults("openai").unwrap();
        assert_eq!(openai.name, "OpenAI");
        assert_eq!(openai.env_key, Some("OPENAI_API_KEY".to_string()));

        let gemini = get_provider_defaults("gemini").unwrap();
        assert_eq!(gemini.provider_type, ProviderType::Gemini);

        let unknown = get_provider_defaults("unknown-provider");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_list_builtin_models() {
        ensure_initialized();

        let models = list_builtin_models();
        assert!(models.contains(&"gpt-5"));
        assert!(models.contains(&"gpt-5.2"));
        assert!(models.contains(&"gpt-5.2-codex"));
        assert!(models.contains(&"gemini-3-pro"));
        assert!(models.contains(&"gemini-3-flash"));
    }

    #[test]
    fn test_list_builtin_providers() {
        ensure_initialized();

        let providers = list_builtin_providers();
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"gemini"));
    }

    #[test]
    fn test_model_capabilities() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        let caps = gpt5.capabilities.unwrap();
        assert!(caps.contains(&Capability::TextGeneration));
        assert!(caps.contains(&Capability::Vision));
        assert!(caps.contains(&Capability::ToolCalling));
        assert!(caps.contains(&Capability::ParallelToolCalls));

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        let caps = gpt52.capabilities.unwrap();
        assert!(caps.contains(&Capability::ExtendedThinking));
        assert!(caps.contains(&Capability::ReasoningSummaries));
    }

    #[test]
    fn test_reasoning_models() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert!(gpt5.default_reasoning_effort.is_some());
        assert!(gpt5.supported_reasoning_levels.is_some());

        let levels = gpt5.supported_reasoning_levels.unwrap();
        assert!(levels.contains(&ReasoningEffort::Low));
        assert!(levels.contains(&ReasoningEffort::Medium));
        assert!(levels.contains(&ReasoningEffort::High));

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        let levels = gpt52.supported_reasoning_levels.unwrap();
        assert!(levels.contains(&ReasoningEffort::XHigh));
    }

    #[test]
    fn test_shell_type() {
        ensure_initialized();

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        assert_eq!(gpt52.shell_type, Some(ConfigShellToolType::ShellCommand));

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert_eq!(gpt5.shell_type, None); // Default
    }

    #[test]
    fn test_gpt52_codex() {
        ensure_initialized();

        let codex = get_model_defaults("gpt-5.2-codex").unwrap();
        assert_eq!(codex.display_name, Some("GPT-5.2 Codex".to_string()));
        assert_eq!(codex.context_window, Some(272000));
        assert_eq!(codex.max_output_tokens, Some(64000));
        assert_eq!(codex.shell_type, Some(ConfigShellToolType::ShellCommand));

        let caps = codex.capabilities.unwrap();
        assert!(caps.contains(&Capability::ExtendedThinking));
        assert!(caps.contains(&Capability::ReasoningSummaries));
        assert!(caps.contains(&Capability::ParallelToolCalls));

        let levels = codex.supported_reasoning_levels.unwrap();
        assert!(levels.contains(&ReasoningEffort::Low));
        assert!(levels.contains(&ReasoningEffort::Medium));
        assert!(levels.contains(&ReasoningEffort::High));
        assert!(levels.contains(&ReasoningEffort::XHigh));
    }
}
