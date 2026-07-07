use super::config_processor::map_error as map_config_error;
use crate::config_manager::ConfigManager;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MergeStrategy;
use codex_model_provider::AMAZON_BEDROCK_PROVIDER_ID;

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
