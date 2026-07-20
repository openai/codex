use std::sync::Arc;

use codex_config::McpServerTransportConfig;
use codex_core::McpManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginsManager;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn contributes_hosted_plugin_runtime_without_an_executor() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("chatgpt_base_url".to_string(), "https://chatgpt.com".into()),
        ])
        .build()
        .await?;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = installed_manager(&config);

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .and_then(|server| server.configured_config())
        .ok_or("hosted plugin runtime should be contributed as a configured server")?;
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.transport else {
        panic!("hosted plugin runtime should use streamable HTTP");
    };
    assert_eq!(url, "https://chatgpt.com/backend-api/ps/mcp");

    Ok(())
}

#[tokio::test]
async fn runtime_overlay_preserves_disabled_server() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            (
                "mcp_servers.codex_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
            ("mcp_servers.codex_apps.enabled".to_string(), false.into()),
        ])
        .build()
        .await?;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = installed_manager(&config);

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .ok_or("hosted plugin runtime should remain configured")?;

    assert!(!server.enabled());
    Ok(())
}

#[tokio::test]
async fn default_fallback_overwrites_reserved_config_without_an_extension() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            (
                "mcp_servers.codex_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
        ])
        .build()
        .await?;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = McpManager::new(Arc::new(PluginsManager::new(
        config.codex_home.to_path_buf(),
    )));

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .and_then(|server| server.configured_config())
        .ok_or("default Apps MCP should be present")?;
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.transport else {
        panic!("default Apps MCP should use streamable HTTP");
    };
    assert_eq!(url, "https://chatgpt.com/backend-api/ps/mcp");

    Ok(())
}

#[tokio::test]
async fn later_extension_can_remove_same_name_registration() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![("features.apps".to_string(), true.into())])
        .build()
        .await?;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut builder = ExtensionRegistryBuilder::new();
    codex_mcp_extension::install(&mut builder);
    builder.mcp_server_contributor(Arc::new(RemoveCodexApps));
    let manager = McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
        Arc::new(builder.build()),
        codex_core::CodexAppsToolsCache::default(),
    );

    let servers = manager.effective_servers(&config, Some(&auth)).await;

    assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));
    Ok(())
}

#[tokio::test]
async fn hosted_apps_mcp_requires_chatgpt_auth() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![("features.apps".to_string(), true.into())])
        .build()
        .await?;
    let auth = CodexAuth::from_api_key("test");
    let manager = installed_manager(&config);

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));

    Ok(())
}

#[tokio::test]
async fn disabled_apps_remove_reserved_server_config_for_all_hosts() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), false.into()),
            (
                "mcp_servers.codex_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
        ])
        .build()
        .await?;
    let managers = [
        installed_manager(&config),
        McpManager::new(Arc::new(PluginsManager::new(
            config.codex_home.to_path_buf(),
        ))),
    ];
    for manager in managers {
        let servers = manager.runtime_servers(&config).await;
        assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));
    }
    Ok(())
}

fn installed_manager(config: &Config) -> McpManager {
    let mut builder = ExtensionRegistryBuilder::new();
    codex_mcp_extension::install(&mut builder);
    McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
        Arc::new(builder.build()),
        codex_core::CodexAppsToolsCache::default(),
    )
}

struct RemoveCodexApps;

impl McpServerContributor<Config> for RemoveCodexApps {
    fn id(&self) -> &'static str {
        "remove_codex_apps"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> codex_extension_api::ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            vec![McpServerContribution::Remove {
                name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            }]
        })
    }
}
