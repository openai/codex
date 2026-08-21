use super::config_processor::map_error as map_config_error;
use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MergeStrategy;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerSource;
use codex_config::format_config_layer_source;
use codex_model_provider::AMAZON_BEDROCK_PROVIDER_ID;

pub(super) async fn configure_bedrock_for_managed_auth(
    config_manager: &ConfigManager,
) -> Result<(), JSONRPCErrorError> {
    ensure_user_model_provider_can_be_bedrock(config_manager).await?;

    config_manager
        .batch_write(ConfigBatchWriteParams {
            edits: [
                (
                    "model_provider",
                    serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
                ),
                (
                    "model_providers.amazon-bedrock.aws.profile",
                    serde_json::Value::Null,
                ),
            ]
            .into_iter()
            .map(|(key_path, value)| ConfigEdit {
                key_path: key_path.to_string(),
                value,
                merge_strategy: MergeStrategy::Replace,
            })
            .collect(),
            file_path: None,
            expected_version: None,
            reload_user_config: false,
        })
        .await
        .map(|_| ())
        .map_err(map_config_error)
}

pub(super) async fn ensure_user_model_provider_can_be_bedrock(
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

    Ok(())
}

pub(super) async fn clear_user_model_provider_if_bedrock(
    config_manager: &ConfigManager,
) -> Result<(), JSONRPCErrorError> {
    let result = config_manager
        .clear_user_value_if_matches(
            "model_provider",
            serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
        )
        .await;
    if let Err(err) = &result
        && err.write_error_code() == Some(ConfigWriteErrorCode::ConfigVersionConflict)
    {
        tracing::warn!(
            "configuration changed while clearing the managed Amazon Bedrock model provider; retrying once"
        );
        return config_manager
            .clear_user_value_if_matches(
                "model_provider",
                serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
            )
            .await
            .map_err(map_config_error);
    }
    result.map_err(map_config_error)
}
