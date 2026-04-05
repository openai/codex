use std::sync::Arc;

use codex_core::config::Config;
use codex_login::AuthManager;

pub(crate) fn auth_manager_from_config(
    config: &Config,
    enable_codex_api_key_env: bool,
) -> Arc<AuthManager> {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        enable_codex_api_key_env,
        config.cli_auth_credentials_store_mode,
    );
    auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());
    auth_manager
}
