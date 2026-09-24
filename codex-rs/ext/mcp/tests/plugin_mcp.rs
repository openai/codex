use anyhow::Context;
use codex_config::AppToolApproval;
use codex_config::McpServerToolConfig;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::EnvironmentConfig;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginCatalogEntry;
use codex_core_plugins::PluginIdentity;
use codex_core_plugins::PluginListQuery;
use codex_core_plugins::PluginProvider;
use codex_core_plugins::PluginProviderError;
use codex_core_plugins::PluginProviderFuture;
use codex_core_plugins::PluginSourceLocation;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::SelectedPluginContribution;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp_extension::PluginProviders;
use codex_mcp_extension::PluginsThreadState;
use codex_mcp_extension::install_plugin_providers;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::run_test_with_large_stack;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq, Eq)]
struct ContributionSummary {
    name: String,
    plugin_id: String,
    plugin_display_name: String,
    selection_order: usize,
    enabled: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct PackageSummary {
    plugin_id: String,
    plugin_display_name: String,
    connector_ids: Vec<String>,
}

#[tokio::test]
async fn selected_plugin_servers_use_managed_requirements_for_the_selected_root_id() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let plugin_root = tempfile::tempdir()?;
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"different-manifest-name","interface":{"displayName":"Selected Demo"}}"#,
    )?;
    std::fs::write(
        plugin_root.path().join(".mcp.json"),
        r#"{
  "mcpServers": {
    "allowed": {"command":"allowed-command"},
    "mismatched": {"command":"wrong-command"},
    "unlisted": {"command":"unlisted-command"}
  }
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[plugins.\"selected-root\".mcp_servers.mismatched]\nenabled = true\n[plugins.\"selected-root\".mcp_servers.unlisted]\nenabled = true",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"
[plugins."selected-root".mcp_servers.allowed.identity]
command = "allowed-command"

[plugins."selected-root".mcp_servers.mismatched.identity]
command = "expected-command"
"#,
            ),
        )
        .build()
        .await?;

    let contributions = selected_plugin_contributions(&config, plugin_root.path()).await?;

    assert_eq!(
        contributions,
        vec![
            ContributionSummary {
                name: "allowed".to_string(),
                plugin_id: "selected-root".to_string(),
                plugin_display_name: "Selected Demo".to_string(),
                selection_order: 0,
                enabled: true,
            },
            ContributionSummary {
                name: "mismatched".to_string(),
                plugin_id: "selected-root".to_string(),
                plugin_display_name: "Selected Demo".to_string(),
                selection_order: 0,
                enabled: false,
            },
            ContributionSummary {
                name: "unlisted".to_string(),
                plugin_id: "selected-root".to_string(),
                plugin_display_name: "Selected Demo".to_string(),
                selection_order: 0,
                enabled: false,
            },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn selected_plugin_package_is_contributed_without_servers_or_connectors() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let plugin_root = tempfile::tempdir()?;
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.path().join("skills/deploy"))?;
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"skill-only","interface":{"displayName":"Skill Only"}}"#,
    )?;
    std::fs::write(
        plugin_root.path().join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploy the project.\n---\n",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;

    let contributions = raw_selected_plugin_contributions(&config, plugin_root.path()).await?;
    let package = contributions
        .into_iter()
        .next()
        .map(|(_, plugin_id, contribution)| PackageSummary {
            plugin_id,
            plugin_display_name: contribution.plugin_display_name,
            connector_ids: contribution.connector_ids,
        });

    assert_eq!(
        package,
        Some(PackageSummary {
            plugin_id: "selected-root".to_string(),
            plugin_display_name: "Skill Only".to_string(),
            connector_ids: Vec::new(),
        })
    );
    Ok(())
}

#[tokio::test]
async fn managed_plugins_requirement_disables_selected_plugin_capabilities() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let plugin_root = tempfile::tempdir()?;
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"selected-root","interface":{"displayName":"Selected Root"}}"#,
    )?;
    std::fs::write(
        plugin_root.path().join(".mcp.json"),
        r#"{"mcpServers":{"probe":{"command":"probe-command"}}}"#,
    )?;
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"
[features]
plugins = false
"#,
            ),
        )
        .build()
        .await?;
    assert!(!config.features.enabled(Feature::Plugins));

    let direct = raw_selected_plugin_contributions(&config, plugin_root.path()).await?;
    assert!(
        matches!(
            direct.as_slice(),
            [(selected_root_id, _, contribution)]
                if selected_root_id == "selected-root" && contribution.servers.is_empty()
        ),
        "managed Plugins disable should preserve only the direct selected-root identity"
    );

    config
        .features
        .enable(Feature::ExecutorCapabilityDiscovery)
        .expect("test config should allow feature update");
    let discovered = raw_selected_plugin_contributions(&config, plugin_root.path()).await?;
    assert!(
        matches!(
            discovered.as_slice(),
            [(selected_root_id, _, contribution)]
                if selected_root_id == "selected-root" && contribution.servers.is_empty()
        ),
        "managed Plugins disable should preserve only the discovered selected-root identity"
    );
    Ok(())
}

#[tokio::test]
async fn high_level_discovery_matches_the_existing_plugin_provider() -> TestResult {
    let codex_home = tempfile::tempdir()?;
    let plugin_root = tempfile::tempdir()?;
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"demo","interface":{"displayName":"Demo"},"mcpServers":"./servers.json"}"#,
    )?;
    std::fs::write(
        plugin_root.path().join("servers.json"),
        r#"{
  "mcpServers": {
    "first": {
      "command": "first",
      "default_tools_approval_mode": "writes",
      "enabled_tools": ["read", "deploy", "trusted", "package-only"],
      "disabled_tools": ["package-denied"],
      "tools": {
        "read": {"approval_mode": "prompt", "output_token_limit": 12000},
        "deploy": {"approval_mode": "approve", "output_token_limit": 4000},
        "trusted": {"approval_mode": "approve"}
      }
    },
    "second": {
      "command": "second",
      "enabled": false,
      "default_tools_approval_mode": "prompt"
    }
  }
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
[plugins."selected-root".mcp_servers.first]
enabled = false
default_tools_approval_mode = "prompt"
enabled_tools = ["read", "deploy", "trusted", "host-only"]
disabled_tools = ["write"]

[plugins."selected-root".mcp_servers.first.tools.read]
approval_mode = "approve"
output_token_limit = 8000

[plugins."selected-root".mcp_servers.first.tools.deploy]
output_token_limit = 9000

[plugins."selected-root".mcp_servers.first.tools.trusted]
approval_mode = "approve"

[plugins."selected-root".mcp_servers.second]
enabled = true
default_tools_approval_mode = "auto"
"#,
    )?;
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    let existing = selected_plugin_contributions(&config, plugin_root.path()).await?;
    let mut servers = raw_selected_plugin_contributions(&config, plugin_root.path())
        .await?
        .into_iter()
        .flat_map(|(_, _, contribution)| contribution.servers)
        .collect::<HashMap<_, _>>();
    let server = servers
        .remove("first")
        .expect("disabled selected server remains registered");
    let declared_disabled_server = servers
        .remove("second")
        .expect("package-disabled server remains registered");
    assert_eq!(
        (
            server.enabled,
            server.default_tools_approval_mode,
            server.enabled_tools,
            server.disabled_tools,
            server.tools,
            declared_disabled_server.enabled,
            declared_disabled_server.default_tools_approval_mode,
        ),
        (
            false,
            Some(AppToolApproval::Prompt),
            Some(vec![
                "read".to_string(),
                "deploy".to_string(),
                "trusted".to_string(),
            ]),
            Some(vec!["package-denied".to_string(), "write".to_string()]),
            HashMap::from([
                (
                    "read".to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(AppToolApproval::Prompt),
                        output_token_limit: std::num::NonZeroUsize::new(8_000),
                    },
                ),
                (
                    "deploy".to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(AppToolApproval::Prompt),
                        output_token_limit: std::num::NonZeroUsize::new(4_000),
                    },
                ),
                (
                    "trusted".to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(AppToolApproval::Approve),
                        ..Default::default()
                    },
                ),
            ]),
            false,
            Some(AppToolApproval::Prompt),
        )
    );
    config
        .features
        .enable(Feature::ExecutorCapabilityDiscovery)
        .expect("test config should allow feature update");
    let high_level = selected_plugin_contributions(&config, plugin_root.path()).await?;

    assert_eq!(high_level, existing);
    Ok(())
}

async fn selected_plugin_contributions(
    config: &Config,
    plugin_root: &std::path::Path,
) -> Result<Vec<ContributionSummary>, Box<dyn std::error::Error>> {
    Ok(raw_selected_plugin_contributions(config, plugin_root)
        .await?
        .into_iter()
        .enumerate()
        .flat_map(|(selection_order, (_, plugin_id, contribution))| {
            contribution
                .servers
                .into_iter()
                .map(move |(name, config)| ContributionSummary {
                    name,
                    plugin_id: plugin_id.clone(),
                    plugin_display_name: contribution.plugin_display_name.clone(),
                    selection_order,
                    enabled: config.enabled,
                })
        })
        .collect())
}

async fn raw_selected_plugin_contributions(
    config: &Config,
    plugin_root: &std::path::Path,
) -> Result<Vec<(String, String, SelectedPluginContribution)>, Box<dyn std::error::Error>> {
    let mut builder = ExtensionRegistryBuilder::new();
    let environment_manager = Arc::new(EnvironmentManager::default_for_tests());
    codex_mcp_extension::install_plugins(&mut builder, Arc::clone(&environment_manager));
    let registry = builder.build();
    let thread_init = ExtensionDataInit::new();
    let selected_capability_roots = vec![SelectedCapabilityRoot {
        id: "selected-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            path: PathUri::from_host_native_path(plugin_root)?,
        },
    }];
    let thread_store = ExtensionData::new_with_init("test-thread", thread_init.clone());
    let executor_capability_discovery = if config
        .features
        .enabled(Feature::ExecutorCapabilityDiscovery)
    {
        Some(
            ExecutorCapabilityDiscoveryCache::new(environment_manager)
                .snapshot(&selected_capability_roots, &Default::default())
                .await,
        )
    } else {
        None
    };

    let selected = registry.mcp_server_contributors()[0]
        .selected_plugins(McpServerContributionContext::for_step(
            config,
            &thread_init,
            &thread_store,
            "test_originator",
            &selected_capability_roots,
            executor_capability_discovery.as_ref(),
        ))
        .await;
    let mut contributions = Vec::new();
    for plugin in selected {
        contributions.push((plugin.selected_root_id, plugin.plugin_id, plugin.mcp.await));
    }
    Ok(contributions)
}

struct CloudCatalogFixture {
    reply: Result<PluginCatalog, String>,
    calls: AtomicUsize,
}

impl PluginProvider for CloudCatalogFixture {
    fn list(&self, _query: PluginListQuery) -> PluginProviderFuture<'_, PluginCatalog> {
        Box::pin(async {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.reply.clone().map_err(PluginProviderError::Message)
        })
    }
}

#[test]
fn cloud_plugins_append_apps_without_changing_executor_tools() -> anyhow::Result<()> {
    run_test_with_large_stack(
        "cloud-plugin-projection",
        run_cloud_plugin_projection_scenarios,
    )
}

async fn run_cloud_plugin_projection_scenarios() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    core_test_support::skip_if_remote!(Ok(()), "V1 plugin fixtures use host-local files");
    // Fresh sessions keep unavailable-cloud fallback independent of a previously cached catalog.
    for (case, installed, cloud_enabled, cloud_available, cloud_empty) in [
        ("cloud_only", false, true, true, false),
        ("cloud_and_executor", true, true, true, false),
        ("unavailable_cloud", true, true, false, false),
        ("rollout_disabled", true, false, true, false),
        ("empty_cloud", true, true, true, true),
    ] {
        let server = start_mock_server().await;
        let apps = AppsTestServer::mount(&server).await?;
        let plugin_server = start_mock_server().await;
        let plugin_mcp = AppsTestServer::mount_with_tools(
            &plugin_server,
            Arc::new(std::sync::Mutex::new(vec![json!({
                "name": "echo", "description": "Echo a note.",
                "annotations": {"readOnlyHint": true},
                "inputSchema": {"type": "object", "properties": {"title": {"type": "string"}}}
            })])),
        )
        .await?;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_partial_json(json!({"method": "tools/call"})))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body)
                    .unwrap_or_else(|error| panic!("invalid MCP request JSON: {error}"));
                wiremock::ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {"content": [], "structuredContent": {"echo": body["params"]["arguments"]["title"]}, "isError": false}
                }))
            })
            .with_priority(1)
            .mount(&plugin_server).await;
        let declaration =
            json!({"url": format!("{}/api/codex/ps/mcp", plugin_mcp.chatgpt_base_url)});
        let cloud = Arc::new(CloudCatalogFixture {
            reply: if cloud_empty {
                Ok(PluginCatalog::default())
            } else if cloud_available {
                Ok(PluginCatalog {
                    entries: vec![PluginCatalogEntry {
                        id: PluginIdentity::Remote {
                            remote_plugin_id: "plugin_notes".into(),
                        },
                        display_name: "Cloud Notes".into(),
                        version: Some("1".into()),
                        mcp_servers: [("notes".into(), declaration.to_string())].into(),
                        connector_ids: vec!["calendar".into()],
                        locations: vec![PluginSourceLocation::Cloud {
                            resource_uri: "plugin://plugin_notes".into(),
                            bundle_uri: None,
                        }],
                    }],
                    warnings: Vec::new(),
                })
            } else {
                Err("plugin-service unavailable".into())
            },
            calls: AtomicUsize::new(0),
        });
        let mut extensions = ExtensionRegistryBuilder::new();
        let mut providers = PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))));
        if cloud_enabled {
            providers = providers.with_cloud_provider(cloud.clone());
        }
        install_plugin_providers(&mut extensions, providers);
        let mut builder = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model_info_override("gpt-5.5", |model| model.supports_search_tool = false)
            .with_config(move |config| {
                assert!(config.features.enable(Feature::Plugins).is_ok());
                assert!(config.features.enable(Feature::Apps).is_ok());
                assert!(
                    config
                        .features
                        .enable(Feature::ExecutorCapabilityDiscovery)
                        .is_ok()
                );
                config.chatgpt_base_url = apps.chatgpt_base_url;
            });
        let test = builder.build(&server).await?;
        if installed {
            let selection = test
                .codex
                .environment_selections()
                .await
                .into_iter()
                .next()
                .context("selected environment missing")?;
            let root = selection.cwd.join("notes")?;
            let root_path = root.to_abs_path()?;
            fs::create_dir_all(root_path.join(".codex-plugin"))?;
            fs::write(root_path.join(".codex-plugin/plugin.json"), json!({
                "name": "notes", "version": "1", "interface": {"displayName": "Installed Notes"},
                "mcpServers": "./.mcp.json"
            }).to_string())?;
            fs::write(
                root_path.join(".mcp.json"),
                json!({"mcpServers": {"notes": declaration}}).to_string(),
            )?;
            test.codex
                .environment_ready(
                    &selection,
                    EnvironmentConfig {
                        allow_login_shell: false,
                        workspace_roots: selection.workspace_roots.clone(),
                        permission_profile: PermissionProfileSnapshot::legacy(
                            test.config.permissions.permission_profile().clone(),
                        ),
                        shell_environment_policy: Default::default(),
                        windows_sandbox_level: WindowsSandboxLevel::from_config(&test.config),
                        windows_sandbox_type: test.config.permissions.windows_sandbox_type,
                        use_legacy_landlock: test.config.features.use_legacy_landlock(),
                        exec_policy: None,
                        mcp_policy: None,
                        network_policy: None,
                        selected_capability_roots: vec![SelectedCapabilityRoot {
                            id: "notes@marketplace".into(),
                            location: CapabilityRootLocation::Environment {
                                environment_id: selection.environment_id.clone(),
                                path: root,
                            },
                        }],
                    },
                )
                .await?;
        }
        let response = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("call"),
                    ev_function_call_with_namespace(
                        "notes-call",
                        "mcp__notes",
                        "echo",
                        r#"{"title":"release check"}"#,
                    ),
                    ev_completed("call"),
                ]),
                sse(vec![
                    ev_assistant_message("done", "Finished checking Notes."),
                    ev_completed("done"),
                ]),
            ],
        )
        .await;
        test.submit_turn("Use the Notes MCP to check the release.")
            .await?;
        let requests = response.requests();
        assert_eq!(requests.len(), 2, "{case}");
        let tool = requests[0].tool_by_name("mcp__notes", "echo");
        assert_eq!(tool.is_some(), installed, "{case}");
        let output = requests[1].function_call_output("notes-call");
        if installed {
            assert!(
                tool.context("Notes tool missing")?["description"]
                    .as_str()
                    .context("tool description missing")?
                    .contains("Installed Notes")
            );
            assert!(
                output.to_string().contains("release check"),
                "{case}: {output}"
            );
        }
        let calls = recorded_apps_tool_calls(&plugin_server).await;
        assert_eq!(calls.len(), usize::from(installed), "{case}");
        if installed {
            assert_eq!(calls[0]["params"]["name"], "echo");
            assert_eq!(
                calls[0]["params"]["arguments"],
                json!({"title": "release check"})
            );
        }
        let initialized = plugin_server
            .received_requests()
            .await
            .context("MCP requests missing")?
            .iter()
            .any(|request| {
                serde_json::from_slice::<serde_json::Value>(&request.body)
                    .is_ok_and(|body| body["method"] == "initialize")
            });
        assert_eq!(
            initialized, installed,
            "{case}: cloud-only declarations must not start MCPs"
        );
        assert_eq!(
            cloud.calls.load(Ordering::SeqCst),
            usize::from(cloud_enabled),
            "{case}"
        );
        let state = test
            .codex
            .thread_extension_data()
            .get::<PluginsThreadState>()
            .context("plugin state missing")?;
        assert_eq!(
            state.cloud_catalog().is_some(),
            cloud_enabled && cloud_available,
            "{case}"
        );
        let calendar = requests[0]
            .tool_by_name("mcp__codex_apps__calendar", "_list_events")
            .context("Calendar tool missing")?;
        assert_eq!(
            calendar["description"]
                .as_str()
                .context("Apps description missing")?
                .contains("Cloud Notes"),
            cloud_enabled && cloud_available && !cloud_empty,
            "{case}"
        );
        test.codex.shutdown_and_wait().await?;
    }
    Ok(())
}
