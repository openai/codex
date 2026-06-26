use std::sync::Arc;

use codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;
use codex_core::McpManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginsManager;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_login::AuthManager;
use codex_login::CodexAuth;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn test_apps_extension(
    auth_manager: Arc<AuthManager>,
    plugins_manager: Arc<PluginsManager>,
) -> codex_mcp_extension::CodexAppsMcpExtension {
    codex_mcp_extension::CodexAppsMcpExtension::new(
        auth_manager,
        Arc::new(EnvironmentManager::without_environments()),
        plugins_manager,
    )
}

#[tokio::test]
async fn manager_without_apps_extension_has_no_reserved_singleton() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ true,
        /*orchestrator_mcp_enabled*/ true,
    )
    .await?;
    let manager = McpManager::new(Arc::new(PluginsManager::new(
        config.codex_home.to_path_buf(),
    )));

    let servers = manager.effective_servers(&config).await;

    assert!(!servers.contains_key(CODEX_APPS_RESOURCE_MCP_SERVER_NAME));
    Ok(())
}

#[tokio::test]
async fn guardian_apps_feature_gate_does_not_touch_the_shared_service() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ false,
        /*orchestrator_mcp_enabled*/ true,
    )
    .await?;
    let service = test_apps_extension(
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
    );

    let contributions = service
        .contribute(McpServerContributionContext::global(&config))
        .await;

    assert!(contributions.is_empty());
    assert!(service.snapshot(&config).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn orchestrator_gate_does_not_touch_the_shared_service() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ true,
        /*orchestrator_mcp_enabled*/ false,
    )
    .await?;
    let service = test_apps_extension(
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
    );

    let contributions = service
        .contribute(McpServerContributionContext::global(&config))
        .await;

    assert!(contributions.is_empty());
    Ok(())
}

#[tokio::test]
async fn apps_extension_requires_codex_backend_auth_without_connecting() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ true,
        /*orchestrator_mcp_enabled*/ true,
    )
    .await?;
    let service = test_apps_extension(
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
    );

    assert!(service.snapshot(&config).await?.is_none());
    assert!(
        service
            .contribute(McpServerContributionContext::global(&config))
            .await
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn install_registers_the_shared_instance() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ false,
        /*orchestrator_mcp_enabled*/ true,
    )
    .await?;
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.to_path_buf()));
    let service = Arc::new(test_apps_extension(
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        Arc::clone(&plugins_manager),
    ));
    let mut builder = ExtensionRegistryBuilder::new();
    codex_mcp_extension::install(&mut builder, service);
    let manager = McpManager::new_with_extensions(plugins_manager, Arc::new(builder.build()));

    assert!(manager.effective_servers(&config).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn host_extension_bundle_owns_registration_and_shutdown() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = apps_config(
        codex_home.path(),
        /*apps_enabled*/ false,
        /*orchestrator_mcp_enabled*/ true,
    )
    .await?;
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.to_path_buf()));
    let extensions = codex_mcp_extension::McpHostExtensions::new(
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        Arc::new(EnvironmentManager::without_environments()),
        Arc::clone(&plugins_manager),
    );
    let manager = McpManager::new_with_extensions(plugins_manager, extensions.registry());

    assert!(manager.effective_servers(&config).await.is_empty());
    extensions.shutdown().await;
    Ok(())
}

async fn apps_config(
    codex_home: &std::path::Path,
    apps_enabled: bool,
    orchestrator_mcp_enabled: bool,
) -> Result<Config, std::io::Error> {
    ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .fallback_cwd(Some(codex_home.to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), apps_enabled.into()),
            (
                "orchestrator.mcp.enabled".to_string(),
                orchestrator_mcp_enabled.into(),
            ),
        ])
        .build()
        .await
}
