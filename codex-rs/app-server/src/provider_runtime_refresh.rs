use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::ThreadManager;

pub(crate) async fn refresh_default_model_provider(
    config_manager: &ConfigManager,
    thread_manager: &ThreadManager,
) -> Result<bool, JSONRPCErrorError> {
    let config = config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await
        .map_err(|err| {
            internal_error(format!(
                "failed to reload config for model provider refresh: {err}"
            ))
        })?;
    Ok(thread_manager.refresh_default_model_provider(&config))
}
