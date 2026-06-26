use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use codex_config::McpServerTransportConfig;
use codex_connectors::ConnectorSnapshot;
use codex_core::McpManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginsManager;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::ThreadDataInitializer;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;

use super::apply_apps_server_policy;
use crate::apps::CodexAppsConnectionKey;
use crate::apps::CodexAppsMcpExtension;
use crate::apps::ConnectedCodexApps;
use crate::apps::config::apps_connect_config;
use crate::apps::config::auth_revision_access_guard;
use crate::apps::config::current_auth_revision;
use crate::apps::presentation::AppsConnectionPreparation;
use crate::apps::presentation::AppsThreadState;
use crate::apps::test_support::connector_tool;
use crate::apps::test_support::gmail_tool;
use crate::apps::test_support::mcp_manager_for_servers;
use crate::apps::test_support::start_blocked_http_apps_server;
use crate::apps::test_support::start_gated_http_apps_server;
use crate::apps::test_support::test_apps;
use crate::apps::test_support::test_apps_with_access_guard;

fn auth_json(account_id: &str, jwt_payload: &str) -> codex_login::AuthDotJson {
    serde_json::from_value(serde_json::json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": format!("e30.{jwt_payload}.sig"),
            "access_token": "not-a-jwt",
            "refresh_token": "test",
            "account_id": account_id,
        },
    }))
    .expect("valid test auth")
}

async fn app_server_plugin_display_names_for_step(
    registry: &ExtensionRegistry<Config>,
    config: &Config,
    thread_init: &ExtensionDataInit,
    thread_store: &ExtensionData,
    available_environment_ids: &[String],
    server_name: &str,
) -> Vec<String> {
    for contributor in registry.mcp_server_contributors() {
        for contribution in contributor
            .contribute(McpServerContributionContext::for_step(
                config,
                thread_init,
                thread_store,
                available_environment_ids,
            ))
            .await
        {
            if let McpServerContribution::SetEffective { name, server } = contribution
                && name == server_name
            {
                return server.runtime_metadata().plugin_display_names().to_vec();
            }
        }
    }
    panic!("missing Apps server contribution for {server_name}")
}

#[tokio::test]
async fn app_servers_carry_plugin_provenance_as_runtime_metadata() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let apps = test_apps(vec![
        gmail_tool("GmailSearch", /*destructive*/ false),
        connector_tool(
            "calendar",
            "Calendar",
            "CalendarList",
            /*destructive*/ false,
        ),
    ])
    .await;
    let snapshot = apps.snapshot();
    let plugin_connectors = ConnectorSnapshot::from_plugin_capability_summaries(&[
        PluginCapabilitySummary {
            config_name: "workspace".to_string(),
            display_name: "Workspace".to_string(),
            app_connector_ids: vec![
                AppConnectorId("gmail".to_string()),
                AppConnectorId("calendar".to_string()),
            ],
            ..Default::default()
        },
        PluginCapabilitySummary {
            config_name: "mail-tools".to_string(),
            display_name: "Mail Tools".to_string(),
            app_connector_ids: vec![AppConnectorId("gmail".to_string())],
            ..Default::default()
        },
    ]);

    let servers = apply_apps_server_policy(
        &config,
        &snapshot,
        &plugin_connectors,
        snapshot
            .effective_mcp_servers()
            .into_iter()
            .collect::<Vec<_>>(),
    );

    let plugin_names = |server_name: &str| {
        servers
            .iter()
            .find(|(name, _)| name == server_name)
            .map(|(_, server)| server.runtime_metadata().plugin_display_names().to_vec())
            .expect("virtual Apps server")
    };
    assert_eq!(
        plugin_names("codex_apps__gmail"),
        vec!["Mail Tools".to_string(), "Workspace".to_string()]
    );
    assert_eq!(
        plugin_names("codex_apps__calendar"),
        vec!["Workspace".to_string()]
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn selected_executor_connector_attribution_follows_step_availability() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let plugin_root = tempfile::tempdir().expect("temp plugin root");
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))
        .expect("create manifest directory");
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{
  "name": "selected-demo",
  "apps": "./.app.json",
  "interface": {"displayName": "Selected Demo"}
}"#,
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_root.path().join(".app.json"),
        r#"{"apps":{"gmail":{"id":"gmail"}}}"#,
    )
    .expect("write plugin apps");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(Arc::clone(
        &auth_manager,
    )));
    let apps = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let auth = auth_manager.auth().await.expect("test auth");
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config, &auth),
            auth_revision: current_auth_revision(&auth_manager),
        },
        apps: Arc::clone(&apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });

    let mut builder = ExtensionRegistryBuilder::new();
    crate::install_with_executor_plugins(
        &mut builder,
        Arc::clone(&service),
        Arc::new(EnvironmentManager::default_for_tests()),
    );
    let registry = builder.build();
    assert_eq!(
        registry
            .mcp_server_contributors()
            .iter()
            .map(|contributor| contributor.id())
            .collect::<Vec<_>>(),
        vec!["selected_executor_plugin_mcp", "codex_apps"]
    );

    let mut thread_init = ExtensionDataInit::new();
    thread_init.insert(vec![SelectedCapabilityRoot {
        id: "selected-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            path: PathUri::from_host_native_path(plugin_root.path()).expect("plugin root path URI"),
        },
    }]);
    registry.initialize_thread_data(&mut thread_init);
    let thread_store = ExtensionData::new_with_init("test-thread", thread_init.clone());

    assert_eq!(
        app_server_plugin_display_names_for_step(
            &registry,
            &config,
            &thread_init,
            &thread_store,
            &[],
            "codex_apps__gmail",
        )
        .await,
        Vec::<String>::new(),
        "an unavailable selected root must not contribute connector attribution"
    );
    let ready_environments = [LOCAL_ENVIRONMENT_ID.to_string()];
    assert_eq!(
        app_server_plugin_display_names_for_step(
            &registry,
            &config,
            &thread_init,
            &thread_store,
            &ready_environments,
            "codex_apps__gmail",
        )
        .await,
        vec!["Selected Demo".to_string()],
        "a ready selected root contributes its display-name attribution"
    );
    assert_eq!(
        app_server_plugin_display_names_for_step(
            &registry,
            &config,
            &thread_init,
            &thread_store,
            &[],
            "codex_apps__gmail",
        )
        .await,
        Vec::<String>::new(),
        "a later unavailable step must clear the prior ready projection"
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn app_server_wins_a_configured_server_name_collision() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
            (
                "mcp_servers.codex_apps__gmail.url".to_string(),
                "https://configured.example/mcp".into(),
            ),
        ])
        .build()
        .await
        .expect("load config");
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(Arc::clone(
        &auth_manager,
    )));
    let apps = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let auth = auth_manager.auth().await.expect("test auth");
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config, &auth),
            auth_revision: current_auth_revision(&auth_manager),
        },
        apps: Arc::clone(&apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(service);
    let manager = McpManager::new_with_extensions(
        Arc::new(PluginsManager::new(config.codex_home.to_path_buf())),
        Arc::new(extensions.build()),
    );

    let configured_servers = manager.current_runtime_servers(&config).await;
    assert!(
        !configured_servers.contains_key("codex_apps__gmail"),
        "the published effective Apps winner must hide the configured collision"
    );
    let servers = manager.effective_servers(&config).await;
    let app = servers
        .get("codex_apps__gmail")
        .expect("Apps extension must win the standard catalog collision");
    assert_eq!(app.config().enabled_tools, Some(vec!["search".to_string()]));
    let McpServerTransportConfig::StreamableHttp { url, .. } = &app.config().transport else {
        panic!("Apps extension should contribute an HTTP MCP server")
    };
    assert!(url.starts_with("http://127.0.0.1:"));

    apps.shutdown().await;
}

#[tokio::test]
async fn explicitly_disabled_codex_apps_server_vetoes_all_apps_contributions() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
            (
                format!(
                    "mcp_servers.{}.url",
                    codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME
                ),
                "https://configured.example/mcp".into(),
            ),
            (
                format!(
                    "mcp_servers.{}.enabled",
                    codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME
                ),
                false.into(),
            ),
        ])
        .build()
        .await
        .expect("load config");
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    let apps = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let auth = auth_manager.auth().await.expect("test auth");
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config, &auth),
            auth_revision: current_auth_revision(&auth_manager),
        },
        apps: Arc::clone(&apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });

    let contributions =
        McpServerContributor::contribute(&service, McpServerContributionContext::global(&config))
            .await;

    assert!(
        contributions.is_empty(),
        "the explicit Apps bundle disable must suppress per-connector servers"
    );
    apps.shutdown().await;
}

#[tokio::test]
async fn contributor_rebuild_replaces_thread_snapshot_and_servers_exactly() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    let original = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let auth = auth_manager.auth().await.expect("test auth");
    let key = CodexAppsConnectionKey {
        config: apps_connect_config(&config, &auth),
        auth_revision: current_auth_revision(&auth_manager),
    };
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key,
        apps: Arc::clone(&original),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);
    let contributions = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    let to_effective_servers = |contributions: Vec<McpServerContribution>| {
        contributions
            .into_iter()
            .filter_map(|contribution| match contribution {
                McpServerContribution::SetEffective { name, server } => Some((name, *server)),
                _ => None,
            })
            .collect::<HashMap<_, _>>()
    };
    let original_servers = to_effective_servers(contributions);
    assert!(original_servers.contains_key(codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME));
    assert!(original_servers.contains_key("codex_apps__gmail"));
    assert_eq!(
        original_servers
            .get("codex_apps__gmail")
            .expect("Gmail runtime registration")
            .config()
            .enabled_tools,
        Some(vec!["search".to_string()])
    );
    assert_eq!(
        thread_init
            .get::<AppsThreadState>()
            .expect("Apps thread state")
            .snapshot()
            .expect("initial Apps snapshot")
            .apps()
            .iter()
            .map(codex_apps::CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["gmail"]
    );

    let connector_id = "connector_76869538009648d5b282a4bb21c3d157";
    let mut unlocked = connector_tool(
        connector_id,
        "GitHub",
        "GitHubAddComment",
        /*destructive*/ false,
    );
    unlocked.title = Some("GitHub_add_comment_to_issue".to_string());
    let refreshed = test_apps(vec![
        unlocked,
        connector_tool(
            "calendar",
            "Calendar",
            "CalendarList",
            /*destructive*/ false,
        ),
    ])
    .await;
    service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
        .expect("connected Apps")
        .apps = Arc::clone(&refreshed);
    service
        .connection
        .publication_revision
        .fetch_add(1, Ordering::AcqRel);
    let refreshed_servers = to_effective_servers(
        McpServerContributor::contribute(
            &service,
            McpServerContributionContext::for_thread(&config, &thread_init),
        )
        .await,
    );

    assert!(!refreshed_servers.contains_key("codex_apps__gmail"));
    assert_eq!(
        refreshed_servers
            .get("codex_apps__calendar")
            .expect("Calendar runtime registration")
            .config()
            .enabled_tools,
        Some(vec!["list".to_string()])
    );
    assert_eq!(
        refreshed_servers
            .get("codex_apps__github")
            .expect("GitHub runtime registration")
            .config()
            .enabled_tools,
        Some(vec!["addcomment".to_string()])
    );
    assert_eq!(
        thread_init
            .get::<AppsThreadState>()
            .expect("Apps thread state")
            .snapshot()
            .expect("refreshed Apps snapshot")
            .apps()
            .iter()
            .map(codex_apps::CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar", connector_id]
    );

    original.shutdown().await;
    refreshed.shutdown().await;
}

#[tokio::test]
async fn contribution_applies_changed_app_policy_for_an_existing_thread() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let build_config = |disable_gmail: bool| {
        let codex_home = codex_home.path().to_path_buf();
        async move {
            let mut overrides = vec![
                ("features.apps".to_string(), true.into()),
                ("orchestrator.mcp.enabled".to_string(), true.into()),
            ];
            if disable_gmail {
                overrides.push(("apps.gmail.enabled".to_string(), false.into()));
            }
            ConfigBuilder::default()
                .codex_home(codex_home.clone())
                .fallback_cwd(Some(codex_home))
                .cli_overrides(overrides)
                .build()
                .await
                .expect("load config")
        }
    };
    let enabled_config = build_config(false).await;
    let disabled_config = build_config(true).await;
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    let apps = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let auth = auth_manager.auth().await.expect("test auth");
    let connection_key = CodexAppsConnectionKey {
        config: apps_connect_config(&enabled_config, &auth),
        auth_revision: current_auth_revision(&auth_manager),
    };
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: connection_key,
        apps: Arc::clone(&apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);

    let enabled = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&enabled_config, &thread_init),
    )
    .await;
    let enabled_gmail = enabled
        .iter()
        .find_map(|contribution| match contribution {
            McpServerContribution::SetEffective { name, server } if name == "codex_apps__gmail" => {
                Some(server.as_ref())
            }
            _ => None,
        })
        .expect("enabled Gmail server");
    assert_eq!(
        enabled_gmail.config().enabled_tools.as_deref(),
        Some(["search".to_string()].as_slice())
    );

    let disabled = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&disabled_config, &thread_init),
    )
    .await;
    let disabled_gmail = disabled
        .iter()
        .find_map(|contribution| match contribution {
            McpServerContribution::SetEffective { name, server } if name == "codex_apps__gmail" => {
                Some(server.as_ref())
            }
            _ => None,
        })
        .expect("disabled Gmail registration remains a normal MCP server");
    assert_eq!(disabled_gmail.config().enabled_tools, Some(Vec::new()));

    apps.shutdown().await;
}

#[tokio::test]
async fn thread_pins_its_connection_when_another_config_becomes_process_current() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config_a = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config A");
    config_a.chatgpt_base_url = "https://config-a.example".to_string();
    let mut config_b = config_a.clone();
    config_b.chatgpt_base_url = "https://config-b.example".to_string();
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    let auth = auth_manager.auth().await.expect("test auth");
    let auth_revision = current_auth_revision(&auth_manager);
    let apps_a = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    let apps_b = test_apps(vec![connector_tool(
        "beta", "Beta", "BetaPing", /*destructive*/ false,
    )])
    .await;
    let mut thread_a = codex_extension_api::ExtensionDataInit::new();
    let mut thread_b = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_a);
    ThreadDataInitializer::initialize(&service, &mut thread_b);

    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config_a, &auth),
            auth_revision,
        },
        apps: Arc::clone(&apps_a),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    let first_a = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config_a, &thread_a),
    )
    .await;
    assert!(first_a.iter().any(|contribution| matches!(
        contribution,
        McpServerContribution::SetEffective { name, .. } if name == "codex_apps__alpha"
    )));

    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config_b, &auth),
            auth_revision,
        },
        apps: Arc::clone(&apps_b),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    let first_b = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config_b, &thread_b),
    )
    .await;
    assert!(first_b.iter().any(|contribution| matches!(
        contribution,
        McpServerContribution::SetEffective { name, .. } if name == "codex_apps__beta"
    )));

    let pinned_a = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config_a, &thread_a),
    )
    .await;
    assert!(pinned_a.iter().any(|contribution| matches!(
        contribution,
        McpServerContribution::SetEffective { name, .. } if name == "codex_apps__alpha"
    )));
    assert!(!pinned_a.iter().any(|contribution| matches!(
        contribution,
        McpServerContribution::SetEffective { name, .. } if name == "codex_apps__beta"
    )));

    service.connection.clear_connected_through(u64::MAX);
    apps_a.shutdown().await;
    apps_b.shutdown().await;
}

#[tokio::test]
async fn auth_changes_revision_switch_connections_and_clear_servers_on_logout() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    let initial_auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let auth_manager = codex_login::AuthManager::from_auth_for_testing_with_home(
        initial_auth,
        codex_home.path().to_path_buf(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    let (initial_apps, initial_calls) = test_apps_with_access_guard(
        vec![gmail_tool("GmailSearch", /*destructive*/ false)],
        auth_revision_access_guard(&auth_manager, current_auth_revision(&auth_manager)),
    )
    .await;
    let initial_manager =
        mcp_manager_for_servers(&initial_apps.snapshot().effective_mcp_servers()).await;
    initial_manager
        .call_tool(
            "codex_apps__gmail",
            "search",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("initial account endpoint should be callable");
    assert_eq!(initial_calls.load(Ordering::Acquire), 1);
    let initial_auth = auth_manager.auth().await.expect("initial auth");
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: CodexAppsConnectionKey {
            config: apps_connect_config(&config, &initial_auth),
            auth_revision: current_auth_revision(&auth_manager),
        },
        apps: Arc::clone(&initial_apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });
    assert_eq!(
        service
            .current_snapshot(&config)
            .await
            .expect("initial snapshot")
            .apps()
            .iter()
            .map(codex_apps::CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["gmail"]
    );
    let initial_revision = McpServerContributor::<codex_core::config::Config>::revision(&service);

    let switched_auth = auth_json(
        "account-b",
        "eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC1iIiwiY2hhdGdwdF91c2VyX2lkIjoidXNlci1iIn19",
    );
    codex_login::save_auth(
        codex_home.path(),
        &switched_auth,
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::default(),
    )
    .expect("save switched auth");
    auth_manager.reload().await;
    assert_eq!(
        auth_manager
            .auth()
            .await
            .and_then(|auth| auth.get_account_id())
            .as_deref(),
        Some("account-b")
    );
    let switched_revision = McpServerContributor::<codex_core::config::Config>::revision(&service);
    assert!(switched_revision > initial_revision);
    let stale_call = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        initial_manager.call_tool(
            "codex_apps__gmail",
            "search",
            /*arguments*/ None,
            /*meta*/ None,
        ),
    )
    .await
    .expect("stale account call should fail promptly");
    assert!(stale_call.is_err());
    assert_eq!(
        initial_calls.load(Ordering::Acquire),
        1,
        "the stale registration must fail before reaching the old upstream"
    );

    let (switched_apps, switched_calls) = test_apps_with_access_guard(
        vec![connector_tool(
            "beta", "Beta", "BetaPing", /*destructive*/ false,
        )],
        auth_revision_access_guard(&auth_manager, current_auth_revision(&auth_manager)),
    )
    .await;
    let switched_auth = auth_manager.auth().await.expect("switched auth");
    let switched_snapshot = service
        .connection
        .apps_for_key(
            CodexAppsConnectionKey {
                config: apps_connect_config(&config, &switched_auth),
                auth_revision: current_auth_revision(&auth_manager),
            },
            /*refresh*/ false,
            {
                let switched_apps = Arc::clone(&switched_apps);
                move || async move { Ok(switched_apps) }
            },
        )
        .await
        .expect("connect switched account")
        .expect("switched auth is current")
        .snapshot();
    let switched_manager =
        mcp_manager_for_servers(&switched_snapshot.effective_mcp_servers()).await;
    switched_manager
        .call_tool(
            "codex_apps__beta",
            "ping",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("switched account endpoint should be callable");
    assert_eq!(switched_calls.load(Ordering::Acquire), 1);
    assert_eq!(
        service
            .current_snapshot(&config)
            .await
            .expect("switched snapshot")
            .apps()
            .iter()
            .map(codex_apps::CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["beta"]
    );

    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);
    thread_init
        .get::<AppsThreadState>()
        .expect("Apps thread state")
        .replace(Some(Arc::clone(&switched_apps)), &config);
    auth_manager.logout().await.expect("log out");
    let logged_out_revision =
        McpServerContributor::<codex_core::config::Config>::revision(&service);
    assert!(logged_out_revision > switched_revision);
    let logged_out_call = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        switched_manager.call_tool(
            "codex_apps__beta",
            "ping",
            /*arguments*/ None,
            /*meta*/ None,
        ),
    )
    .await
    .expect("logged-out call should fail promptly");
    assert!(logged_out_call.is_err());
    assert_eq!(
        switched_calls.load(Ordering::Acquire),
        1,
        "logout must fail before reaching the authenticated upstream"
    );

    let contributions = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert!(contributions.is_empty());
    assert!(
        thread_init
            .get::<AppsThreadState>()
            .expect("Apps thread state")
            .snapshot()
            .is_none()
    );
    assert!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
    );
    initial_manager.shutdown().await;
    switched_manager.shutdown().await;
}

#[tokio::test]
async fn stale_contributor_returns_last_good_and_refreshes_once_in_background() {
    let (base_url, list_calls, list_gate, server) =
        start_blocked_http_apps_server(vec![gmail_tool("GmailSearch", /*destructive*/ false)])
            .await;
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = base_url;
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    list_gate.add_permits(1);
    let initial = service
        .snapshot(&config)
        .await
        .expect("initialize Apps inventory")
        .expect("initial Apps snapshot");
    assert_eq!(list_calls.load(Ordering::Acquire), 1);
    let initial_url = initial
        .effective_mcp_servers()
        .get("codex_apps__gmail")
        .and_then(|server| match &server.config().transport {
            McpServerTransportConfig::StreamableHttp { url, .. } => Some(url.clone()),
            McpServerTransportConfig::Stdio { .. } => None,
        })
        .expect("initial Apps HTTP server");
    service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
        .expect("current Apps connection")
        .refresh_after = Some(std::time::Instant::now());
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(service.as_ref(), &mut thread_init);
    let server_url = |contributions: &[McpServerContribution]| {
        contributions
            .iter()
            .find_map(|contribution| match contribution {
                McpServerContribution::SetEffective { name, server }
                    if name == "codex_apps__gmail" =>
                {
                    match &server.config().transport {
                        McpServerTransportConfig::StreamableHttp { url, .. } => Some(url.clone()),
                        McpServerTransportConfig::Stdio { .. } => None,
                    }
                }
                McpServerContribution::Set { .. }
                | McpServerContribution::SetEffective { .. }
                | McpServerContribution::SelectedPlugin { .. }
                | McpServerContribution::Remove { .. } => None,
            })
    };

    let revision_before_refresh =
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref());
    let first = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        McpServerContributor::contribute(
            service.as_ref(),
            McpServerContributionContext::for_thread(&config, &thread_init),
        ),
    )
    .await
    .expect("stale contribution must not await inventory refresh");
    assert_eq!(server_url(&first).as_deref(), Some(initial_url.as_str()));
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while list_calls.load(Ordering::Acquire) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background stale refresh starts");
    assert!(service.connection.background_initialization_is_active());

    let concurrent = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert_eq!(
        server_url(&concurrent).as_deref(),
        Some(initial_url.as_str())
    );
    assert_eq!(
        list_calls.load(Ordering::Acquire),
        2,
        "stale contribution refreshes must be single-flight"
    );

    list_gate.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while service.connection.background_initialization_is_active()
            || McpServerContributor::<codex_core::config::Config>::revision(service.as_ref())
                == revision_before_refresh
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background refresh publishes a new contribution revision");
    let refreshed = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert_ne!(
        server_url(&refreshed).as_deref(),
        Some(initial_url.as_str())
    );
    assert_eq!(list_calls.load(Ordering::Acquire), 2);

    service.shutdown().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn current_only_contributor_uses_stale_published_servers_without_refreshing() {
    let (base_url, list_calls, list_gate, server) =
        start_blocked_http_apps_server(vec![gmail_tool("GmailSearch", /*destructive*/ false)])
            .await;
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = base_url;
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    list_gate.add_permits(1);
    service
        .snapshot(&config)
        .await
        .expect("initialize Apps inventory")
        .expect("initial Apps snapshot");
    service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
        .expect("current Apps connection")
        .refresh_after = Some(std::time::Instant::now());

    let contributions = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::global_current(&config),
    )
    .await;

    assert!(contributions.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__gmail"
        )
    }));
    assert_eq!(list_calls.load(Ordering::Acquire), 1);
    assert!(!service.connection.background_initialization_is_active());

    service.shutdown().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn cold_contributor_returns_immediately_and_shutdown_joins_initialization() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind gated Apps upstream");
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = format!(
        "http://{}",
        listener.local_addr().expect("gated Apps address")
    );
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);

    let contributions = {
        let contribution = McpServerContributor::contribute(
            &service,
            McpServerContributionContext::for_thread(&config, &thread_init),
        );
        tokio::pin!(contribution);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::select! {
                biased;
                contributions = &mut contribution => contributions,
                accepted = listener.accept() => {
                    accepted.expect("accept gated Apps connection");
                    panic!("cold contribution awaited the upstream connection");
                }
            }
        })
        .await
        .expect("cold contribution deadlocked")
    };
    assert!(contributions.is_empty());
    assert!(
        thread_init
            .get::<AppsThreadState>()
            .expect("Apps thread state")
            .snapshot()
            .is_none()
    );
    let (_gated_connection, _) =
        tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept())
            .await
            .expect("background initialization reaches the gated upstream")
            .expect("accept gated Apps connection");
    assert!(service.connection.background_initialization_is_active());
    assert_eq!(
        service
            .initialization_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        1,
    );

    service.shutdown().await;

    assert!(!service.connection.background_initialization_is_active());
    assert_eq!(
        service
            .initialization_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        0,
    );
}

#[tokio::test]
async fn cold_thread_adopts_connection_rekeyed_during_discovery() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);
    let thread_state = thread_init
        .get::<AppsThreadState>()
        .expect("Apps thread state");
    let current_key = service
        .connection
        .connection_key(&config)
        .await
        .expect("eligible Apps connection key");
    let mut discovery_key = current_key.clone();
    discovery_key.auth_revision ^= 1;
    let AppsConnectionPreparation::Initialize {
        revision: discovery_revision,
    } = thread_state.prepare_connection_if_revision(
        thread_state.revision(),
        discovery_key.clone(),
        &config,
    )
    else {
        panic!("cold thread should start discovery")
    };

    let apps = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    *service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ConnectedCodexApps {
        key: current_key.clone(),
        apps: Arc::clone(&apps),
        refresh_after: Some(std::time::Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL),
    });

    let contributions = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert!(contributions.iter().any(|contribution| matches!(
        contribution,
        McpServerContribution::SetEffective { name, .. } if name == "codex_apps__alpha"
    )));
    assert!(thread_state.apps_for_key(&current_key).is_some());
    assert!(
        !thread_state.replace_apps_if_revision(
            discovery_revision,
            discovery_key,
            Arc::clone(&apps),
            apps.snapshot(),
            &config,
        ),
        "the revision must still reject an actually stale completion"
    );

    service.shutdown().await;
}

#[tokio::test]
async fn current_only_cold_contributor_does_not_start_discovery() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind gated Apps upstream");
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = format!(
        "http://{}",
        listener.local_addr().expect("gated Apps address")
    );
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));

    let contributions = McpServerContributor::contribute(
        &service,
        McpServerContributionContext::global_current(&config),
    )
    .await;
    McpServerContributor::refresh(
        &service,
        McpServerContributionContext::global_current(&config),
    )
    .await;

    assert!(contributions.is_empty());
    assert!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none(),
        "discovery-free contribution must not publish an Apps connection"
    );
    assert!(!service.connection.background_initialization_is_active());
    assert_eq!(
        service
            .initialization_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        0,
    );
    service.shutdown().await;
}

#[tokio::test]
async fn cold_contributor_coalesces_one_inventory_and_publishes_on_next_boundary() {
    let (base_url, list_calls, list_gate, server) =
        start_blocked_http_apps_server(vec![gmail_tool("GmailSearch", /*destructive*/ false)])
            .await;
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = base_url;
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(service.as_ref(), &mut thread_init);
    let thread_state = thread_init
        .get::<AppsThreadState>()
        .expect("Apps thread state");
    let initial_revision =
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref());

    let first = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert!(first.is_empty(), "the cold boundary must stay nonblocking");
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while list_calls.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background inventory request starts");

    let mut joining_thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(service.as_ref(), &mut joining_thread_init);
    let joining_thread_state = joining_thread_init
        .get::<AppsThreadState>()
        .expect("joining Apps thread state");
    let waiting = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        McpServerContributor::contribute(
            service.as_ref(),
            McpServerContributionContext::for_thread(&config, &joining_thread_init),
        ),
    )
    .await
    .expect("same-key waiter must not await inventory discovery");
    assert!(waiting.is_empty());
    let pending = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        McpServerContributor::contribute(
            service.as_ref(),
            McpServerContributionContext::for_thread(&config, &thread_init),
        ),
    )
    .await
    .expect("discovering thread must remain nonblocking");
    assert!(pending.is_empty());
    assert_eq!(
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref()),
        initial_revision,
        "pending discovery must not publish before it has new state"
    );
    assert_eq!(list_calls.load(Ordering::Acquire), 1);

    list_gate.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while thread_state.snapshot().is_none()
            || service.connection.background_initialization_is_active()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background inventory publishes");
    assert!(
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref())
            > initial_revision
    );
    let first_ready = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    let joining_ready = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config, &joining_thread_init),
    )
    .await;
    assert!(first_ready.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__gmail"
        )
    }));
    assert!(joining_ready.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__gmail"
        )
    }));
    assert_eq!(
        list_calls.load(Ordering::Acquire),
        1,
        "cold and joining boundaries must share one upstream inventory"
    );
    assert!(thread_state.snapshot().is_some());
    assert!(joining_thread_state.snapshot().is_some());

    service.shutdown().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn cold_contributor_initializes_distinct_connection_keys_without_losing_publication() {
    let (base_url_a, list_calls_a, list_gate_a, server_a) =
        start_blocked_http_apps_server(vec![gmail_tool("GmailSearch", /*destructive*/ false)])
            .await;
    let (base_url_b, list_calls_b, list_gate_b, server_b) =
        start_blocked_http_apps_server(vec![connector_tool(
            "calendar",
            "Calendar",
            "CalendarList",
            /*destructive*/ false,
        )])
        .await;
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config_a = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    let mut config_b = config_a.clone();
    config_a.chatgpt_base_url = base_url_a;
    config_b.chatgpt_base_url = base_url_b;
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    let mut thread_init_a = codex_extension_api::ExtensionDataInit::new();
    let mut thread_init_b = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(service.as_ref(), &mut thread_init_a);
    ThreadDataInitializer::initialize(service.as_ref(), &mut thread_init_b);
    let thread_state_a = thread_init_a
        .get::<AppsThreadState>()
        .expect("Apps thread A state");
    let thread_state_b = thread_init_b
        .get::<AppsThreadState>()
        .expect("Apps thread B state");
    let key_a = service
        .connection
        .connection_key(&config_a)
        .await
        .expect("Apps connection key A");
    let key_b = service
        .connection
        .connection_key(&config_b)
        .await
        .expect("Apps connection key B");
    assert_ne!(key_a, key_b);

    let initial_revision =
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref());
    let first_a = McpServerContributor::contribute_with_revision(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config_a, &thread_init_a),
    )
    .await;
    assert_eq!(first_a.revision, initial_revision);
    assert!(first_a.contributions.is_empty());
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while list_calls_a.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("connection A starts inventory discovery");
    assert_eq!(
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref()),
        initial_revision,
        "starting discovery must not publish before new state exists"
    );

    let first_b = McpServerContributor::contribute_with_revision(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config_b, &thread_init_b),
    )
    .await;
    assert_eq!(first_b.revision, initial_revision);
    assert!(first_b.contributions.is_empty());
    assert_eq!(
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref()),
        initial_revision,
        "parallel discovery must not publish before either connection completes"
    );
    assert!(
        service
            .connection
            .background_initialization_is_active_for(&key_a)
    );
    assert!(
        service
            .connection
            .background_initialization_is_active_for(&key_b)
    );
    assert_eq!(
        list_calls_b.load(Ordering::Acquire),
        0,
        "connection B waits behind the shared cold-connect lock"
    );

    list_gate_a.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while thread_state_a.snapshot().is_none() || list_calls_b.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("connection A publishes and connection B starts");
    list_gate_b.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while thread_state_b.snapshot().is_none()
            || service.connection.background_initialization_is_active()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("connection B publishes");
    assert_eq!(
        McpServerContributor::<codex_core::config::Config>::revision(service.as_ref()),
        initial_revision + 2,
        "each successfully published connection advances the revision once"
    );

    let contributions_a = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config_a, &thread_init_a),
    )
    .await;
    let contributions_b = McpServerContributor::contribute(
        service.as_ref(),
        McpServerContributionContext::for_thread(&config_b, &thread_init_b),
    )
    .await;
    assert!(contributions_a.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__gmail"
        )
    }));
    assert!(contributions_b.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__calendar"
        )
    }));
    assert_eq!(list_calls_a.load(Ordering::Acquire), 1);
    assert_eq!(list_calls_b.load(Ordering::Acquire), 1);

    service.connection.clear_connected_through(u64::MAX);
    drop(service);
    server_a.abort();
    server_b.abort();
    let _ = server_a.await;
    let _ = server_b.await;
}

#[tokio::test]
async fn failed_cold_initialization_retries_at_the_next_contribution_boundary() {
    let (base_url, reject_requests, server) =
        start_gated_http_apps_server(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
        ])
        .build()
        .await
        .expect("load config");
    config.chatgpt_base_url = base_url;
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let mut thread_init = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut thread_init);

    let initial_revision = McpServerContributor::<codex_core::config::Config>::revision(&service);
    let first = McpServerContributor::contribute_with_revision(
        &service,
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert_eq!(first.revision, initial_revision);
    assert!(first.contributions.is_empty());
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let published_revision =
                McpServerContributor::<codex_core::config::Config>::revision(&service);
            if published_revision > initial_revision
                && !service.connection.background_initialization_is_active()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed initialization publishes a retry revision");
    assert!(service.current_snapshot(&config).await.is_none());
    reject_requests.store(false, Ordering::Release);

    let retry_revision = McpServerContributor::<codex_core::config::Config>::revision(&service);
    let retry = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        McpServerContributor::contribute_with_revision(
            &service,
            McpServerContributionContext::for_thread(&config, &thread_init),
        ),
    )
    .await
    .expect("retry boundary must not await Apps recovery");
    assert_eq!(retry.revision, retry_revision);
    assert!(retry.contributions.is_empty());
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if service.current_snapshot(&config).await.is_some()
                && !service.connection.background_initialization_is_active()
                && McpServerContributor::<codex_core::config::Config>::revision(&service)
                    > retry_revision
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background Apps retry publishes a live connection");

    let ready = McpServerContributor::contribute_with_revision(
        &service,
        McpServerContributionContext::for_thread(&config, &thread_init),
    )
    .await;
    assert!(ready.contributions.iter().any(|contribution| {
        matches!(
            contribution,
            McpServerContribution::SetEffective { name, .. }
                if name == "codex_apps__gmail"
        )
    }));
    assert!(service.current_snapshot(&config).await.is_some());
    assert_eq!(
        ready.revision,
        McpServerContributor::<codex_core::config::Config>::revision(&service),
        "the ready boundary observes the published recovery"
    );
    let recovered_servers = ready
        .contributions
        .iter()
        .filter_map(|contribution| match contribution {
            McpServerContribution::SetEffective { name, server } => {
                Some((name.clone(), server.as_ref().clone()))
            }
            McpServerContribution::Set { .. }
            | McpServerContribution::SelectedPlugin { .. }
            | McpServerContribution::Remove { .. } => None,
        })
        .collect::<HashMap<_, _>>();
    let manager = mcp_manager_for_servers(&recovered_servers).await;
    assert!(manager.list_all_tools().await.iter().any(|tool| {
        tool.server_name == "codex_apps__gmail" && tool.tool.name.as_ref() == "search"
    }));
    manager.shutdown().await;
    service.shutdown().await;
    server.abort();
    let _ = server.await;
}
