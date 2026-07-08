use super::*;
use crate::config::ConfigBuilder;
use crate::config::test_config;
use codex_config::SessionThreadConfig;
use codex_config::StaticThreadConfigLoader;
use codex_config::ThreadConfigSource;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::tempdir;

#[tokio::test]
async fn codex_apps_parallel_tool_calls_follow_feature_flag() {
    let mut config = test_config().await;
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.to_path_buf()));
    let manager = McpManager::new(plugins_manager);

    let servers = manager.runtime_servers(&config).await;
    assert_eq!(
        servers
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .map(|server| server.supports_parallel_tool_calls),
        Some(false)
    );

    let _ = config.features.enable(Feature::CodexAppsParallelToolCalls);
    let servers = manager.runtime_servers(&config).await;
    assert_eq!(
        servers
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .map(|server| server.supports_parallel_tool_calls),
        Some(true)
    );
}

#[tokio::test]
async fn session_feature_enables_codex_apps_parallel_tool_calls() {
    let codex_home = tempdir().expect("create temp dir");
    let thread_config_loader =
        StaticThreadConfigLoader::new(vec![ThreadConfigSource::Session(SessionThreadConfig {
            features: BTreeMap::from([("codex_apps_parallel_tool_calls".to_string(), true)]),
            ..Default::default()
        })]);
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![(
            "mcp_servers.third_party.url".to_string(),
            toml::Value::String("https://third-party.example/mcp".to_string()),
        )])
        .thread_config_loader(Arc::new(thread_config_loader))
        .build()
        .await
        .expect("load config with session feature");
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.to_path_buf()));
    let manager = McpManager::new(plugins_manager);

    let servers = manager.runtime_servers(&config).await;

    assert_eq!(
        servers
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .map(|server| server.supports_parallel_tool_calls),
        Some(true)
    );
    assert_eq!(
        servers
            .get("third_party")
            .map(|server| server.supports_parallel_tool_calls),
        Some(false)
    );
}
