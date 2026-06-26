use std::sync::Arc;

use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_core::McpManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginsManager;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_mcp::EffectiveMcpServer;
use serde_json::json;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SERVER_NAME: &str = "runtime_bridge";
const RUNTIME_SECRET: &str = "runtime-secret";

#[derive(Clone)]
struct RuntimeServerContributor {
    server: EffectiveMcpServer,
}

impl McpServerContributor<Config> for RuntimeServerContributor {
    fn id(&self) -> &'static str {
        "runtime_server_test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> codex_extension_api::ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            vec![McpServerContribution::SetEffective {
                name: SERVER_NAME.to_string(),
                server: Box::new(self.server.clone()),
            }]
        })
    }
}

fn runtime_server() -> TestResult<EffectiveMcpServer> {
    let config: McpServerConfig = serde_json::from_value(json!({
        "url": "http://127.0.0.1:4321/mcp",
    }))?;
    Ok(EffectiveMcpServer::configured_with_runtime_bearer_token(
        config,
        RUNTIME_SECRET.to_string(),
    )?)
}

#[test]
fn effective_contribution_debug_redacts_runtime_credentials() -> TestResult {
    let contribution = McpServerContribution::SetEffective {
        name: SERVER_NAME.to_string(),
        server: Box::new(runtime_server()?),
    };

    let debug = format!("{contribution:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(RUNTIME_SECRET));

    Ok(())
}

#[tokio::test]
async fn effective_contribution_wins_without_entering_configured_views() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![(
            format!("mcp_servers.{SERVER_NAME}.url"),
            "https://configured.example/mcp".into(),
        )])
        .build()
        .await?;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(RuntimeServerContributor {
        server: runtime_server()?,
    }));
    let manager = McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
        Arc::new(extensions.build()),
    );

    let configured = manager.configured_servers(&config).await;
    assert!(configured.contains_key(SERVER_NAME));
    let runtime_configured = manager.runtime_servers(&config).await;
    assert!(!runtime_configured.contains_key(SERVER_NAME));

    let runtime_config = manager.runtime_config(&config).await;
    let config_debug = format!("{runtime_config:?}");
    assert!(config_debug.contains("[REDACTED]"));
    assert!(!config_debug.contains(RUNTIME_SECRET));

    let effective = manager.effective_servers(&config).await;
    let server = effective
        .get(SERVER_NAME)
        .ok_or("runtime contribution should be effective")?
        .config();
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.transport else {
        panic!("runtime contribution should use streamable HTTP");
    };
    assert_eq!(url, "http://127.0.0.1:4321/mcp");
    let effective_debug = format!("{effective:?}");
    assert!(effective_debug.contains("[REDACTED]"));
    assert!(!effective_debug.contains(RUNTIME_SECRET));

    Ok(())
}
