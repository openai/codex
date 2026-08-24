use super::ConfigEditsBuilder;
use crate::config::Config;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;

impl ConfigEditsBuilder {
    /// Return the user config paths to clear for the selected Bedrock provider.
    pub fn bedrock_provider_config_paths_to_clear(config: &Config) -> Option<Vec<Vec<String>>> {
        let provider_id = config.model_provider_id.as_str();
        if !matches!(
            provider_id,
            AMAZON_BEDROCK_PROVIDER_ID | AMAZON_BEDROCK_RUNTIME_PROVIDER_ID
        ) {
            return None;
        }

        let user_layer = config.config_layer_stack.get_active_user_layer()?;
        if user_layer
            .config
            .get("model_provider")
            .and_then(toml::Value::as_str)
            != Some(provider_id)
        {
            return None;
        }

        Some(vec![
            vec!["model_provider".to_string()],
            vec![
                "model_providers".to_string(),
                provider_id.to_string(),
                "aws".to_string(),
            ],
            vec!["model".to_string()],
        ])
    }
}
