//! Extension functions for model popup to support custom providers.

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use codex_core::config::Config;
use codex_core::models_manager::provider_preset::provider_to_preset;
use codex_protocol::openai_models::ModelPreset;

/// Built-in provider IDs that should not appear in the custom providers list.
const BUILTIN_PROVIDER_IDS: &[&str] = &["openai", "ollama", "lmstudio"];

/// Get custom provider presets from Config.model_providers.
///
/// This filters out the built-in providers (openai, ollama, lmstudio) and
/// converts remaining user-defined providers to ModelPreset format.
///
/// Note: This function ensures `derive_model_info()` is called on each provider
/// to populate `model_family` before conversion, enabling proper `default_reasoning_effort`
/// resolution.
pub fn get_custom_provider_presets(config: &Config) -> Vec<ModelPreset> {
    config
        .model_providers
        .iter()
        .filter(|(id, _)| !BUILTIN_PROVIDER_IDS.contains(&id.as_str()))
        .map(|(id, provider)| {
            // Ensure model_info is derived for default_reasoning_level resolution
            let mut provider = provider.clone();
            provider.ext.derive_model_info();
            provider_to_preset(id, &provider)
        })
        .collect()
}

/// Extract provider_id from a preset ID in "providername/model" format.
pub fn extract_provider_id(preset_id: &str) -> Option<String> {
    preset_id.split('/').next().map(String::from)
}

/// Build SelectionItems for custom provider presets.
/// Moved from chatwidget.rs to minimize upstream merge conflicts.
pub fn build_custom_provider_selection_items(config: &Config) -> Vec<SelectionItem> {
    let custom_providers = get_custom_provider_presets(config);

    custom_providers
        .into_iter()
        .map(|preset| {
            let provider_id = extract_provider_id(&preset.id);
            let preset_for_action = preset.clone();
            let provider_id_for_action = provider_id.clone();
            let single_supported_effort = preset.supported_reasoning_efforts.len() == 1;

            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenReasoningPopup {
                    model: preset_for_action.clone(),
                    provider_id: provider_id_for_action.clone(),
                });
            })];

            let is_current = provider_id.as_deref() == Some(config.model_provider_id.as_str());

            SelectionItem {
                name: preset.display_name.clone(),
                description: Some(preset.description.clone()),
                is_current,
                actions,
                dismiss_on_select: single_supported_effort,
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_provider_id() {
        assert_eq!(
            extract_provider_id("deepseek/deepseek-r1"),
            Some("deepseek".to_string())
        );
        assert_eq!(
            extract_provider_id("azure/gpt-4-turbo"),
            Some("azure".to_string())
        );
        assert_eq!(
            extract_provider_id("no_slash"),
            Some("no_slash".to_string())
        );
    }
}
