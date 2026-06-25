use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributionMode;
use codex_extension_api::McpServerContributor;

use super::*;

fn test_mcp_server(url: &str) -> McpServerConfig {
    McpServerConfig {
        transport: codex_config::McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        auth: Default::default(),
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

struct PublishDuringContribution {
    revision: AtomicU64,
}

struct CurrentOverlayContributor {
    saw_current_only: AtomicBool,
}

impl McpServerContributor<Config> for CurrentOverlayContributor {
    fn id(&self) -> &'static str {
        "current-overlay"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        self.saw_current_only.store(
            context.mode() == McpServerContributionMode::Current,
            Ordering::Release,
        );
        Box::pin(async move {
            vec![
                McpServerContribution::Set {
                    name: "configured-overlay".to_string(),
                    config: Box::new(test_mcp_server("https://overlay.example/mcp")),
                },
                McpServerContribution::Remove {
                    name: "removed-overlay".to_string(),
                },
                McpServerContribution::SetEffective {
                    name: "effective-overlay".to_string(),
                    server: Box::new(EffectiveMcpServer::configured(test_mcp_server(
                        "http://127.0.0.1:4321/mcp",
                    ))),
                },
            ]
        })
    }
}

impl McpServerContributor<Config> for PublishDuringContribution {
    fn id(&self) -> &'static str {
        "publish-during-contribution"
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            self.revision.fetch_add(1, Ordering::AcqRel);
            Vec::new()
        })
    }
}

#[tokio::test]
async fn resolution_stores_pre_contribution_revision_without_retrying_churn() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let config = crate::config::ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");
    let contributor = Arc::new(PublishDuringContribution {
        revision: AtomicU64::new(0),
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.mcp_server_contributor(contributor.clone());
    let manager = McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(codex_home.path().to_path_buf())),
        Arc::new(extensions.build()),
    );
    let thread_init = ExtensionDataInit::new();
    let thread_store = ExtensionData::new("thread");

    let (_, _, first_revision) = tokio::time::timeout(
        Duration::from_secs(1),
        manager.runtime_config_for_step_with_base_and_revision(
            &config,
            &thread_init,
            &thread_store,
            &[],
        ),
    )
    .await
    .expect("resolution should not retry a continuously changing contributor");
    assert_eq!(first_revision, vec![(contributor.id(), 0)]);
    assert_eq!(manager.contributors_revision(), vec![(contributor.id(), 1)]);

    let (_, _, second_revision) = manager
        .runtime_config_for_step_with_base_and_revision(&config, &thread_init, &thread_store, &[])
        .await;
    assert_eq!(second_revision, vec![(contributor.id(), 1)]);
    assert_eq!(manager.contributors_revision(), vec![(contributor.id(), 2)]);
}

#[tokio::test]
async fn current_runtime_servers_preserve_overlay_winners() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let config = crate::config::ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![
            (
                "mcp_servers.configured-overlay.url".to_string(),
                "https://configured.example/mcp".into(),
            ),
            (
                "mcp_servers.removed-overlay.url".to_string(),
                "https://removed.example/mcp".into(),
            ),
            (
                "mcp_servers.effective-overlay.url".to_string(),
                "https://effective.example/mcp".into(),
            ),
        ])
        .build()
        .await
        .expect("config should load");
    let contributor = Arc::new(CurrentOverlayContributor {
        saw_current_only: AtomicBool::new(false),
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.mcp_server_contributor(contributor.clone());
    let manager = McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(codex_home.path().to_path_buf())),
        Arc::new(extensions.build()),
    );

    let servers = manager.current_runtime_servers(&config).await;

    assert!(contributor.saw_current_only.load(Ordering::Acquire));
    assert!(!servers.contains_key("removed-overlay"));
    assert!(!servers.contains_key("effective-overlay"));
    let configured = servers
        .get("configured-overlay")
        .expect("configured overlay winner");
    let codex_config::McpServerTransportConfig::StreamableHttp { url, .. } = &configured.transport
    else {
        panic!("configured overlay should use HTTP");
    };
    assert_eq!(url, "https://overlay.example/mcp");

    let _ = manager.runtime_servers(&config).await;
    assert!(
        !contributor.saw_current_only.load(Ordering::Acquire),
        "ordinary runtime resolution must continue to allow discovery"
    );
}

#[tokio::test]
async fn sourceful_base_preserves_plugin_provenance_during_runtime_resolution() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let config = crate::config::ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");
    let manager = McpManager::new(Arc::new(PluginsManager::new(
        codex_home.path().to_path_buf(),
    )));
    let thread_init = ExtensionDataInit::new();
    let thread_store = ExtensionData::new("thread");
    let mut catalog = codex_mcp::McpCatalogBuilder::default();
    catalog.register(McpServerRegistration::from_plugin(
        "plugin-server".to_string(),
        McpPluginAttribution::new("plugin-id".to_string(), "Plugin".to_string()),
        /*plugin_order*/ 0,
        test_mcp_server("https://plugin.example/mcp"),
    ));
    let sourceful_base = McpConfiguredBase {
        catalog: catalog.build(),
    };

    let (mcp_config, _) = manager
        .runtime_config_for_step_from_base_with_revision(
            &config,
            &thread_init,
            &thread_store,
            &[],
            &sourceful_base,
        )
        .await;
    let provenance = codex_mcp::tool_plugin_provenance(&mcp_config);
    assert_eq!(
        provenance.plugin_id_for_mcp_server_name("plugin-server"),
        Some("plugin-id")
    );

    let source_less_base = McpConfiguredBase::from_servers(sourceful_base.configured_servers());
    let (mcp_config, _) = manager
        .runtime_config_for_step_from_base_with_revision(
            &config,
            &thread_init,
            &thread_store,
            &[],
            &source_less_base,
        )
        .await;
    assert_eq!(
        codex_mcp::tool_plugin_provenance(&mcp_config)
            .plugin_id_for_mcp_server_name("plugin-server"),
        None
    );
}
