use codex_core::AuthManager;
use codex_core::config::Config;
use codex_login::token_data::TokenData;
use std::collections::HashSet;
use std::time::Duration;

use crate::chatgpt_client::chatgpt_get_request_with_timeout;
use crate::chatgpt_token::get_chatgpt_token_data;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

use codex_connectors::AllConnectorsCacheKey;
use codex_connectors::DirectoryListResponse;

pub use codex_core::connectors::AppInfo;
pub use codex_core::connectors::connector_display_label;
use codex_core::connectors::filter_disallowed_connectors;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools_with_options;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status;
pub use codex_core::connectors::list_cached_accessible_connectors_from_mcp_tools;
use codex_core::connectors::merge_connectors;
use codex_core::connectors::merge_plugin_apps;
pub use codex_core::connectors::with_app_enabled_state;
use codex_core::plugins::AppConnectorId;
use codex_core::plugins::PluginsManager;

const DIRECTORY_CONNECTORS_TIMEOUT: Duration = Duration::from_secs(60);

async fn apps_enabled(config: &Config) -> bool {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        /*enable_codex_api_key_env*/ false,
        config.cli_auth_credentials_store_mode,
    );
    config.features.apps_enabled(Some(&auth_manager)).await
}
pub async fn list_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !apps_enabled(config).await {
        return Ok(Vec::new());
    }
    let (connectors_result, accessible_result) = tokio::join!(
        list_all_connectors(config),
        list_accessible_connectors_from_mcp_tools(config),
    );
    let connectors = connectors_result?;
    let accessible = accessible_result?;
    Ok(with_app_enabled_state(
        merge_connectors_with_accessible(
            connectors, accessible, /*all_connectors_loaded*/ true,
        ),
        config,
    ))
}

pub async fn list_all_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    list_all_connectors_with_options(config, /*force_refetch*/ false).await
}

pub async fn list_cached_all_connectors(config: &Config) -> Option<Vec<AppInfo>> {
    if !apps_enabled(config).await {
        return Some(Vec::new());
    }

    if init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await
        .is_err()
    {
        return None;
    }
    let token_data = get_chatgpt_token_data()?;
    let cache_key = all_connectors_cache_key(config, &token_data);
    codex_connectors::cached_all_connectors(&cache_key).map(|connectors| {
        let connectors = merge_plugin_apps(connectors, plugin_apps_for_config(config));
        filter_disallowed_connectors(connectors)
    })
}

pub async fn list_all_connectors_with_options(
    config: &Config,
    force_refetch: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    if !apps_enabled(config).await {
        return Ok(Vec::new());
    }
    init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await?;

    let token_data =
        get_chatgpt_token_data().ok_or_else(|| anyhow::anyhow!("ChatGPT token not available"))?;
    let cache_key = all_connectors_cache_key(config, &token_data);
    let connectors = codex_connectors::list_all_connectors_with_options(
        cache_key,
        token_data.id_token.is_workspace_account(),
        force_refetch,
        |path| async move {
            chatgpt_get_request_with_timeout::<DirectoryListResponse>(
                config,
                path,
                Some(DIRECTORY_CONNECTORS_TIMEOUT),
            )
            .await
        },
    )
    .await?;
    let connectors = merge_plugin_apps(connectors, plugin_apps_for_config(config));
    Ok(filter_disallowed_connectors(connectors))
}

fn all_connectors_cache_key(config: &Config, token_data: &TokenData) -> AllConnectorsCacheKey {
    AllConnectorsCacheKey::new(
        config.chatgpt_base_url.clone(),
        token_data.account_id.clone(),
        token_data.id_token.chatgpt_user_id.clone(),
        token_data.id_token.is_workspace_account(),
    )
}

fn plugin_apps_for_config(config: &Config) -> Vec<codex_core::plugins::AppConnectorId> {
    PluginsManager::new(config.codex_home.clone())
        .plugins_for_config(config)
        .effective_apps()
}

pub fn connectors_for_plugin_apps(
    connectors: Vec<AppInfo>,
    plugin_apps: &[AppConnectorId],
) -> Vec<AppInfo> {
    let plugin_app_ids = plugin_apps
        .iter()
        .map(|connector_id| connector_id.0.as_str())
        .collect::<HashSet<_>>();

    filter_disallowed_connectors(merge_plugin_apps(connectors, plugin_apps.to_vec()))
        .into_iter()
        .filter(|connector| plugin_app_ids.contains(connector.id.as_str()))
        .collect()
}

pub fn merge_connectors_with_accessible(
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
    all_connectors_loaded: bool,
) -> Vec<AppInfo> {
    let accessible_connectors = if all_connectors_loaded {
        let connector_ids: HashSet<&str> = connectors
            .iter()
            .map(|connector| connector.id.as_str())
            .collect();
        accessible_connectors
            .into_iter()
            .filter(|connector| connector_ids.contains(connector.id.as_str()))
            .collect()
    } else {
        accessible_connectors
    };
    let merged = merge_connectors(connectors, accessible_connectors);
    filter_disallowed_connectors(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::connectors::connector_install_url;
    use codex_core::plugins::AppConnectorId;
    use pretty_assertions::assert_eq;

    fn app(id: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn allows_asdk_connectors() {
        let filtered = filter_disallowed_connectors(vec![app("asdk_app_hidden"), app("alpha")]);
        assert_eq!(filtered, vec![app("asdk_app_hidden"), app("alpha")]);
    }

    #[test]
    fn allows_whitelisted_asdk_connectors() {
        let filtered = filter_disallowed_connectors(vec![
            app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
            app("beta"),
        ]);
        assert_eq!(
            filtered,
            vec![
                app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
                app("beta")
            ]
        );
    }

    #[test]
    fn filters_openai_prefixed_connectors() {
        let filtered = filter_disallowed_connectors(vec![
            app("connector_openai_foo"),
            app("connector_openai_bar"),
            app("gamma"),
        ]);
        assert_eq!(filtered, vec![app("gamma")]);
    }

    #[test]
    fn filters_disallowed_connector_ids() {
        let filtered = filter_disallowed_connectors(vec![
            app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
            app("delta"),
        ]);
        assert_eq!(filtered, vec![app("delta")]);
    }

    fn merged_app(id: &str, is_accessible: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url(id, id)),
            is_accessible,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn excludes_accessible_connectors_not_in_all_when_all_loaded() {
        let merged = merge_connectors_with_accessible(
            vec![app("alpha")],
            vec![app("alpha"), app("beta")],
            true,
        );
        assert_eq!(merged, vec![merged_app("alpha", true)]);
    }

    #[test]
    fn keeps_accessible_connectors_not_in_all_while_all_loading() {
        let merged = merge_connectors_with_accessible(
            vec![app("alpha")],
            vec![app("alpha"), app("beta")],
            false,
        );
        assert_eq!(
            merged,
            vec![merged_app("alpha", true), merged_app("beta", true)]
        );
    }

    #[test]
    fn connectors_for_plugin_apps_returns_only_requested_plugin_apps() {
        let connectors = connectors_for_plugin_apps(
            vec![app("alpha"), app("beta")],
            &[
                AppConnectorId("alpha".to_string()),
                AppConnectorId("gmail".to_string()),
            ],
        );
        assert_eq!(connectors, vec![app("alpha"), merged_app("gmail", false)]);
    }

    #[test]
    fn connectors_for_plugin_apps_filters_disallowed_plugin_apps() {
        let connectors = connectors_for_plugin_apps(
            Vec::new(),
            &[AppConnectorId(
                "asdk_app_6938a94a61d881918ef32cb999ff937c".to_string(),
            )],
        );
        assert_eq!(connectors, Vec::<AppInfo>::new());
    }
}
