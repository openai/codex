use codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;
use codex_apps::CodexAppsAccessGuard;
use codex_apps::CodexAppsCacheContext;
use codex_apps::CodexAppsCacheIdentity;
use codex_apps::CodexAppsConnectConfig;
use codex_core::config::Config;
use codex_login::AuthManager;
use codex_login::CodexAuth;

pub(super) fn apps_connect_config(config: &Config, auth: &CodexAuth) -> CodexAppsConnectConfig {
    let connect_config = CodexAppsConnectConfig::new(
        config.chatgpt_base_url.clone(),
        apps_mcp_product_sku(config),
        config.mcp_oauth_credentials_store_mode,
        config.auth_keyring_backend_kind(),
    )
    .with_auth_elicitation(
        config
            .features
            .enabled(codex_features::Feature::AuthElicitation),
    );
    let account_id = auth.get_account_id();
    let chatgpt_user_id = auth.get_chatgpt_user_id();
    if account_id.is_none() && chatgpt_user_id.is_none() {
        return connect_config;
    }
    connect_config.with_cache_context(CodexAppsCacheContext::new(
        config.codex_home.to_path_buf(),
        CodexAppsCacheIdentity::default()
            .with_account_id(account_id)
            .with_chatgpt_user_id(chatgpt_user_id)
            .with_workspace_account(auth.is_workspace_account()),
    ))
}

pub(super) fn current_auth_revision(auth_manager: &AuthManager) -> u64 {
    let receiver = auth_manager.auth_change_receiver();
    *receiver.borrow()
}

pub(super) fn auth_revision_access_guard(
    auth_manager: &AuthManager,
    expected_revision: u64,
) -> CodexAppsAccessGuard {
    let revision = auth_manager.auth_change_receiver();
    CodexAppsAccessGuard::new(move || *revision.borrow() == expected_revision)
}

pub(super) fn apps_inventory_eligible(config: &Config) -> bool {
    config.features.enabled(codex_features::Feature::Apps)
}

pub(super) fn apps_mcp_eligible(config: &Config) -> bool {
    // Preserve the legacy singleton's explicit opt-out as a veto for the whole Apps MCP bundle.
    apps_inventory_eligible(config)
        && config.orchestrator_mcp_enabled
        && config
            .mcp_servers
            .get()
            .get(CODEX_APPS_RESOURCE_MCP_SERVER_NAME)
            .is_none_or(|server| server.enabled)
}

pub(super) fn apps_mcp_product_sku(config: &Config) -> Option<String> {
    config
        .config_layer_stack
        .effective_config()
        .as_table()
        .and_then(|table| table.get("apps_mcp_product_sku"))
        .and_then(codex_config::TomlValue::as_str)
        .map(str::trim)
        .filter(|sku| !sku.is_empty())
        .map(str::to_string)
}

pub(super) fn include_apps_instructions(config: &Config) -> bool {
    config
        .config_layer_stack
        .effective_config()
        .as_table()
        .and_then(|table| table.get("include_apps_instructions"))
        .and_then(codex_config::TomlValue::as_bool)
        .unwrap_or(true)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
