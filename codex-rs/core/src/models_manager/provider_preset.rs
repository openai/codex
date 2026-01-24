//! Convert user-configured model providers to ModelPreset format for UI display.

use crate::model_provider_info::ModelProviderInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;

/// Convert a ModelProviderInfo to a ModelPreset for display in the /model picker.
///
/// The preset ID uses the format `providername/model` to uniquely identify
/// custom provider models.
///
/// Note: `derive_model_info()` should be called on the provider before calling
/// this function to ensure `model_info` is populated for `default_reasoning_level`.
pub fn provider_to_preset(provider_id: &str, provider: &ModelProviderInfo) -> ModelPreset {
    let model_name = provider
        .ext
        .model_name
        .clone()
        .unwrap_or_else(|| provider_id.to_string());

    // Use configured reasoning efforts, else default [Low, Medium, High]
    let supported_efforts = if provider.ext.supported_reasoning_efforts.is_empty() {
        default_reasoning_efforts()
    } else {
        provider.ext.supported_reasoning_efforts.clone()
    };

    // Get default_reasoning_level from model_info if available, else None
    let default_effort = provider
        .ext
        .model_info
        .as_ref()
        .and_then(|f| f.default_reasoning_level)
        .unwrap_or(ReasoningEffort::None);

    ModelPreset {
        id: format!("{}/{}", provider_id, model_name),
        model: model_name.clone(),
        display_name: format!("{}/{}", provider_id, model_name),
        description: provider.base_url.clone().unwrap_or_default(),
        default_reasoning_effort: default_effort,
        supported_reasoning_efforts: supported_efforts,
        is_default: false,
        upgrade: None,
        show_in_picker: true,
        supported_in_api: true,
    }
}

/// Default reasoning efforts for all custom providers.
/// All custom providers support full reasoning effort options.
fn default_reasoning_efforts() -> Vec<ReasoningEffortPreset> {
    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Fast".into(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Balanced".into(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Thorough".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_provider_info::ModelProviderInfo;
    use crate::model_provider_info_ext::ModelProviderInfoExt;

    #[test]
    fn test_provider_to_preset_with_model_name() {
        let provider = ModelProviderInfo {
            name: "DeepSeek".to_string(),
            base_url: Some("https://api.deepseek.com".to_string()),
            ext: ModelProviderInfoExt {
                model_name: Some("deepseek-r1".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let preset = provider_to_preset("deepseek", &provider);

        assert_eq!(preset.id, "deepseek/deepseek-r1");
        assert_eq!(preset.model, "deepseek-r1");
        assert_eq!(preset.display_name, "deepseek/deepseek-r1");
        assert_eq!(preset.description, "https://api.deepseek.com");
        assert_eq!(preset.supported_reasoning_efforts.len(), 3);
    }

    #[test]
    fn test_provider_to_preset_without_model_name() {
        let provider = ModelProviderInfo {
            name: "Custom".to_string(),
            base_url: None,
            ext: ModelProviderInfoExt::default(),
            ..Default::default()
        };

        let preset = provider_to_preset("custom_provider", &provider);

        assert_eq!(preset.id, "custom_provider/custom_provider");
        assert_eq!(preset.model, "custom_provider");
        assert_eq!(preset.display_name, "custom_provider/custom_provider");
    }

    #[test]
    fn test_provider_to_preset_with_configured_reasoning_efforts() {
        let custom_efforts = vec![
            ReasoningEffortPreset {
                effort: ReasoningEffort::High,
                description: "Deep thinking".into(),
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::XHigh,
                description: "Maximum".into(),
            },
        ];

        let provider = ModelProviderInfo {
            name: "Custom".to_string(),
            base_url: None,
            ext: ModelProviderInfoExt {
                model_name: Some("test-model".to_string()),
                supported_reasoning_efforts: custom_efforts.clone(),
                ..Default::default()
            },
            ..Default::default()
        };

        let preset = provider_to_preset("custom", &provider);

        assert_eq!(preset.supported_reasoning_efforts.len(), 2);
        assert_eq!(
            preset.supported_reasoning_efforts[0].effort,
            ReasoningEffort::High
        );
        assert_eq!(
            preset.supported_reasoning_efforts[1].effort,
            ReasoningEffort::XHigh
        );
    }

    #[test]
    fn test_provider_to_preset_default_reasoning_effort_none() {
        // Without model_info, default_reasoning_effort should be None
        let provider = ModelProviderInfo {
            name: "Custom".to_string(),
            base_url: None,
            ext: ModelProviderInfoExt::default(),
            ..Default::default()
        };

        let preset = provider_to_preset("custom", &provider);

        assert_eq!(preset.default_reasoning_effort, ReasoningEffort::None);
    }
}
