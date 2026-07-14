use super::config_processor::map_error as map_config_error;
use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MergeStrategy;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerSource;
use codex_config::format_config_layer_source;
use codex_model_provider::AMAZON_BEDROCK_PROVIDER_ID;

pub(super) async fn set_user_model_provider_to_bedrock(
    config_manager: &ConfigManager,
) -> Result<(), JSONRPCErrorError> {
    let layers = config_manager
        .load_config_layers(/*cwd*/ None)
        .await
        .map_err(|err| internal_error(format!("failed to load configuration layers: {err}")))?;
    let user_precedence = match layers.get_active_user_layer() {
        Some(layer) => layer.name.precedence(),
        None => ConfigLayerSource::User {
            file: config_manager.user_config_path().map_err(|err| {
                internal_error(format!("failed to resolve user config path: {err}"))
            })?,
            profile: None,
        }
        .precedence(),
    };
    if let Some((overriding_layer, effective_provider)) = layers
        .layers_high_to_low()
        .into_iter()
        .filter(|layer| layer.name.precedence() > user_precedence)
        .find_map(|layer| {
            layer
                .config
                .get("model_provider")
                .map(|value| (layer, value))
        })
        && effective_provider.as_str() != Some(AMAZON_BEDROCK_PROVIDER_ID)
    {
        let source = format_config_layer_source(&overriding_layer.name, CONFIG_TOML_FILE);
        return Err(invalid_request(format!(
            "Amazon Bedrock login cannot select `{AMAZON_BEDROCK_PROVIDER_ID}` because {source} sets `model_provider` to {effective_provider}"
        )));
    }

    write_user_model_provider(
        config_manager,
        serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
        /*expected_version*/ None,
    )
    .await
}

async fn write_user_model_provider(
    config_manager: &ConfigManager,
    value: serde_json::Value,
    expected_version: Option<String>,
) -> Result<(), JSONRPCErrorError> {
    config_manager
        .write_value(ConfigValueWriteParams {
            key_path: "model_provider".to_string(),
            value,
            merge_strategy: MergeStrategy::Replace,
            file_path: None,
            expected_version,
        })
        .await
        .map(|_| ())
        .map_err(map_config_error)
}
