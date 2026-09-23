//! Loads embedded application policy and builds serving authentication from effective requirements.
//! Workspace policy uses stored auth even when the caller uses an environment API key.
//! Caller credential storage remains intact; initialization receives the effective URL and residency.

use crate::config_manager::ConfigManager;
use codex_core::config::Config;
use codex_login::AuthManager;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::sync::Arc;

pub(super) async fn configure(
    config_manager: &ConfigManager,
    config: &mut Arc<Config>,
    enable_codex_api_key_env: bool,
) -> IoResult<Arc<AuthManager>> {
    let bootstrap_config = config_manager
        .load_startup_config(Some(config.cwd.to_path_buf()))
        .await?;
    let caller_auth = config.auth_config();
    let bootstrap_auth = codex_login::AuthConfig {
        chatgpt_base_url: Some(bootstrap_config.chatgpt_base_url.clone()),
        forced_login_method: bootstrap_config.forced_login_method,
        forced_chatgpt_workspace_id: bootstrap_config.forced_chatgpt_workspace_id.clone(),
        managed_auth_policy: bootstrap_config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        auth_route_config: bootstrap_config.auth_route_config(),
        ..caller_auth.clone()
    };
    bootstrap_auth.validate()?;
    let bootstrap_auth_manager = AuthManager::shared_from_auth_config(
        bootstrap_auth,
        /*enable_codex_api_key_env*/ false,
    )
    .await
    .map_err(IoError::other)?;
    config_manager.replace_cloud_config_bundle_loader(
        bootstrap_auth_manager,
        bootstrap_config.chatgpt_base_url.clone(),
        bootstrap_config.http_client_factory(),
    );
    let policy_config = config_manager
        .load_startup_config(Some(config.cwd.to_path_buf()))
        .await?;
    let auth_config = codex_login::AuthConfig {
        chatgpt_base_url: Some(policy_config.chatgpt_base_url.clone()),
        forced_login_method: policy_config.forced_login_method,
        forced_chatgpt_workspace_id: policy_config.forced_chatgpt_workspace_id.clone(),
        managed_auth_policy: policy_config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        auth_route_config: policy_config.auth_route_config(),
        ..caller_auth
    };
    auth_config.validate()?;
    let policy_auth_manager = AuthManager::shared_from_auth_config(
        auth_config.clone(),
        /*enable_codex_api_key_env*/ false,
    )
    .await
    .map_err(IoError::other)?;
    let auth_manager = if enable_codex_api_key_env {
        AuthManager::shared_from_auth_config(auth_config, /*enable_codex_api_key_env*/ true)
            .await
            .map_err(IoError::other)?
    } else {
        policy_auth_manager.clone()
    };
    let config = Arc::make_mut(config);
    config.chatgpt_base_url = policy_config.chatgpt_base_url;
    config.enforce_residency = policy_config.enforce_residency;
    config.application_network_policy = policy_config.application_network_policy;
    config.application_auth_route_config = policy_config.application_auth_route_config;
    config.forced_login_method = policy_config.forced_login_method;
    config.forced_chatgpt_workspace_id = policy_config.forced_chatgpt_workspace_id;
    config_manager.replace_cloud_config_bundle_loader(
        policy_auth_manager,
        config.chatgpt_base_url.clone(),
        config.http_client_factory(),
    );
    config_manager
        .sync_default_client_residency_requirement()
        .await;
    Ok(auth_manager)
}
