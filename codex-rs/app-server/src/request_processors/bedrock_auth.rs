use super::config_processor::map_error as map_config_error;
use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MergeStrategy;
use codex_model_provider::AMAZON_BEDROCK_PROVIDER_ID;

pub(super) struct UserModelProviderState {
    model_provider: Option<String>,
    version: Option<String>,
}

pub(super) async fn set_user_model_provider_to_bedrock(
    config_manager: &ConfigManager,
) -> Result<(), JSONRPCErrorError> {
    write_user_model_provider(
        config_manager,
        serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
        /*expected_version*/ None,
    )
    .await
}

pub(super) async fn clear_user_model_provider_if_bedrock(
    config_manager: &ConfigManager,
    user_model_provider: UserModelProviderState,
) -> Result<(), JSONRPCErrorError> {
    if user_model_provider.model_provider.as_deref() == Some(AMAZON_BEDROCK_PROVIDER_ID) {
        write_user_model_provider(
            config_manager,
            serde_json::Value::Null,
            user_model_provider.version,
        )
        .await?;
    }

    Ok(())
}

pub(super) async fn user_model_provider_state(
    config_manager: &ConfigManager,
) -> Result<UserModelProviderState, JSONRPCErrorError> {
    let user_config_path = config_manager
        .user_config_path()
        .map_err(|err| internal_error(format!("failed to resolve user config path: {err}")))?;
    let response = config_manager
        .read(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await
        .map_err(|err| internal_error(format!("failed to read user config: {err}")))?;
    let layer = response
        .layers
        .unwrap_or_default()
        .into_iter()
        .find(|layer| {
            matches!(
                &layer.name,
                ConfigLayerSource::User { file, .. } if file == &user_config_path
            )
        });
    let Some(layer) = layer else {
        return Ok(UserModelProviderState {
            model_provider: None,
            version: None,
        });
    };
    let model_provider = layer
        .config
        .get("model_provider")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok(UserModelProviderState {
        model_provider,
        version: Some(layer.version),
    })
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
