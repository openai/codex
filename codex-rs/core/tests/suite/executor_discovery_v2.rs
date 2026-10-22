//! Verifies V2 discovery, plugin acceptance across auth changes, and cache behavior.

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginCatalogEntry;
use codex_core_plugins::PluginIdentity;
use codex_core_plugins::PluginListQuery;
use codex_core_plugins::PluginProvider;
use codex_core_plugins::PluginProviderFuture;
use codex_core_plugins::PluginSourceLocation;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Cache;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::HttpRequestBodyDeltaNotification;
use codex_exec_server::HttpRequestParams;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_mcp_extension::PluginProviders;
use codex_mcp_extension::install_plugin_providers;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::ExecutorSkillProvider;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::install_with_providers;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

struct DiscoveryRecorder {
    snapshots: Arc<Mutex<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>>,
    selected_plugins: Arc<Mutex<Option<SelectedPluginSnapshot>>>,
    mcp_snapshots_per_step: Arc<Mutex<Vec<usize>>>,
}

impl McpServerContributor<Config> for DiscoveryRecorder {
    fn id(&self) -> &'static str {
        "discovery-recorder"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            if let Some(snapshots) = context.executor_capability_discovery_v2() {
                self.snapshots.lock().await.extend_from_slice(snapshots);
            }
            vec![McpServerContribution::HostedPluginConnectors {
                plugin_id: "plugin_sentinel".to_string(),
                plugin_display_name: "Inert sentinel".to_string(),
                connector_ids: Vec::new(),
            }]
        })
    }

    fn hosted_plugin_ids(&self, _context: McpServerContributionContext<'_, Config>) -> Vec<String> {
        vec!["plugin_sentinel".to_string()]
    }

    fn selected_plugins<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
        _plugins_config: &'a codex_config::types::PluginsConfigToml,
    ) -> ExtensionFuture<'a, Vec<codex_extension_api::SelectedPlugin<'a>>> {
        Box::pin(async move {
            let mut package_roots = vec!["unavailable-root".to_string()];
            let selected_roots = context
                .ready_selected_capability_roots()
                .unwrap_or_default();
            // Core must reject configured legacy roots when V2 supplies the inventory.
            package_roots.extend(selected_roots.iter().map(|root| root.id.clone()));
            if !selected_roots.is_empty() && context.executor_capability_discovery_v2().is_none() {
                let legacy = context.executor_capability_discovery();
                assert_eq!(
                    legacy.map(|snapshot| snapshot.roots().len()),
                    Some(selected_roots.len())
                );
                assert!(legacy.is_some_and(|snapshot| {
                    snapshot.roots().iter().all(|root| root.result.is_ok())
                }));
            }
            if let Some(snapshots) = context.executor_capability_discovery_v2() {
                for snapshot in snapshots {
                    if let Ok(inventory) = &snapshot.result {
                        package_roots.extend(
                            inventory
                                .plugins
                                .iter()
                                .map(|plugin| format!("{}:{}", snapshot.environment_id, plugin.id)),
                        );
                    }
                }
            }
            package_roots
                .into_iter()
                .map(|selected_root_id| codex_extension_api::SelectedPlugin {
                    selected_root_id,
                    identity: PluginIdentity::Local {
                        plugin_id: "inert-sentinel@market".to_string(),
                    },
                    plugin_id: "inert-sentinel@market".to_string(),
                    enabled: true,
                    mcp: Box::pin(async {
                        codex_extension_api::SelectedPluginContribution {
                            plugin_display_name: "Inert sentinel".to_string(),
                            source_environment_id: codex_exec_server::REMOTE_ENVIRONMENT_ID
                                .to_string(),
                            servers: Vec::new(),
                            connector_ids: Vec::new(),
                        }
                    }),
                })
                .collect()
        })
    }
}

impl codex_extension_api::ToolContributor for DiscoveryRecorder {
    fn prepare_for_step<'a>(
        &'a self,
        input: codex_extension_api::StepPreparationInput<'a>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            *self.selected_plugins.lock().await = Some(input.selected_plugins.clone());
        })
    }
}

impl ContextContributor for DiscoveryRecorder {
    fn contribute_world_state<'a>(
        &'a self,
        _input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async move {
            let snapshot_count = self.snapshots.lock().await.len();
            self.mcp_snapshots_per_step
                .lock()
                .await
                .push(snapshot_count);
            Vec::new()
        })
    }
}

struct Executor {
    url: String,
    inventory: Arc<Mutex<Option<Value>>>,
    requests: Arc<Mutex<Vec<Value>>>,
    _task: AbortOnDropHandle<Result<()>>,
}

#[derive(Clone, Copy)]
enum DiscoveryFailure {
    None,
    Server,
    Disconnect,
}

impl Executor {
    async fn start(supports_v2: bool, failure: DiscoveryFailure) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("ws://{}", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let inventory = Arc::new(Mutex::new(None::<Value>));
        let served_inventory = Arc::clone(&inventory);
        let recorded = Arc::clone(&requests);
        let discovery_attempts = Arc::new(AtomicUsize::new(0));
        let http = codex_exec_server::Environment::create_for_tests(/*exec_server_url*/ None)?
            .get_http_client();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                let recorded = Arc::clone(&recorded);
                let inventory = Arc::clone(&served_inventory);
                let discovery_attempts = Arc::clone(&discovery_attempts);
                let http = Arc::clone(&http);
                connections.spawn(async move {
                    let mut socket = accept_async(stream).await?;
                    while let Some(Ok(message)) = socket.next().await {
                        let request: Value = match message {
                            Message::Text(text) => serde_json::from_str(&text)?,
                            Message::Binary(bytes) => serde_json::from_slice(&bytes)?,
                            Message::Close(_) => break,
                            _ => continue,
                        };
                        if request.get("id").is_none() { continue; }
                        recorded.lock().await.push(request.clone());
                        if request["method"] == "http/request" {
                            let params: HttpRequestParams = serde_json::from_value(request["params"].clone())?;
                            let request_id = params.request_id.clone();
                            let streaming = params.stream_response;
                            let mut response = http.http_request(params).await?;
                            let delta = streaming.then(|| HttpRequestBodyDeltaNotification {
                                request_id, seq: 1,
                                delta: std::mem::replace(&mut response.body, Vec::new().into()),
                                done: true, error: None,
                            });
                            socket.send(Message::Text(json!({"id":request["id"],"result":response}).to_string().into())).await?;
                            if let Some(delta) = delta {
                                socket.send(Message::Text(json!({"method":"http/request/bodyDelta","params":delta}).to_string().into())).await?;
                            }
                            continue;
                        }
                        if request["method"] == "capabilities/discoverV2" {
                            let attempt = discovery_attempts.fetch_add(1, Ordering::Relaxed);
                            match failure {
                                // Exhaust the environment's one transport replay so the cache sees Err.
                                DiscoveryFailure::Disconnect if attempt < 2 => break,
                                DiscoveryFailure::Server if attempt == 0 => {
                                    socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32004,"message":"fixture discovery failure"}}).to_string().into())).await?;
                                    continue;
                                }
                                DiscoveryFailure::None | DiscoveryFailure::Server | DiscoveryFailure::Disconnect => {}
                            }
                        }
                        let result = match request["method"].as_str() {
                            Some("initialize") => Some(json!({"sessionId":"acquisition-only", "environmentInfo": {
                                "shell":{"name":"bash","path":"/bin/bash"},
                                "capabilities":{"sandboxedFileStreaming":true,"capabilityDiscoveryV2":supports_v2}
                            }})),
                            Some("environment/info") => Some(json!({
                                "shell":{"name":"bash","path":"/bin/bash"},
                                "capabilities":{"sandboxedFileStreaming":true,"capabilityDiscoveryV2":supports_v2}
                            })),
                            Some("capabilityRoots/discoverV1") => Some(json!({
                                "roots": request["params"]["roots"].as_array().context("V1 roots")?
                                    .iter().map(|root| json!({"id":root["id"],"path":root["path"]})).collect::<Vec<_>>()
                            })),
                            Some("capabilities/discoverV2") => Some(inventory.lock().await.clone().unwrap_or_else(|| json!({
                                "plugins":[{"id":"inert-sentinel@market","displayName":"inert-sentinel","remotePluginId":null,
                                    "version":"1","root":"file:///inert-sentinel",
                                    "files":{"manifest":{"path":"file:///inert-sentinel/.codex-plugin/plugin.json",
                                        "contents":"{\"name\":\"inert-sentinel\",\"mcpServers\":{\"inert-sentinel\":{\"command\":\"never-run\"}}}"},
                                        "mcpConfig":{"path":"file:///inert-sentinel/.codex-plugin/plugin.json",
                                            "contents":"{\"inert-sentinel\":{\"command\":\"never-run\"}}"},
                                        "appsConfig":null},"skills":[]}],
                                "skills":[{"name":"inert-sentinel","description":"Executor skill used by the acquisition test",
                                    "shortDescription":null,"path":"file:///inert-sentinel/SKILL.md",
                                    "discoveryRoot":{"id":"codex-home-skills","path":"file:///"},"metadata":null}],
                                "warnings":[]
                            }))),
                            Some("fs/open") if request["params"]["path"] == "file:///inert-sentinel/SKILL.md" => Some(json!({"handleId": request["params"]["handleId"]})),
                            Some("fs/readBlock") => Some(json!(codex_exec_server::FsReadBlockResponse {
                                chunk: b"# Sentinel instructions from the captured executor".to_vec().into(),
                                eof: true,
                            })),
                            Some("fs/close") => Some(json!({})),
                            Some("fs/canonicalize") => Some(json!({"path":request["params"]["path"]})),
                            Some("fs/walk") => Some(json!({"entries":[],"errors":[],"truncated":false})),
                            _ => None,
                        };
                        let response = match result {
                            Some(result) => json!({"id":request["id"],"result":result}),
                            None => json!({"id":request["id"],"error":{"code":-32004,"message":"fixture discovery failure"}}),
                        };
                        if socket.send(Message::Text(response.to_string().into())).await.is_err() { break; }
                    }
                    anyhow::Ok(())
                });
            }
            anyhow::Ok(())
        });
        Ok(Self {
            url,
            inventory,
            requests,
            _task: AbortOnDropHandle::new(task),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_disabled_skips_inventory_discovery() -> Result<()> {
    check_acquisition(/*enabled*/ false, /*supported*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_shares_inventory_with_mcp_and_skills_until_next_turn() -> Result<()> {
    check_acquisition(/*enabled*/ true, /*supported*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_changed_inventory_refreshes_mcp_without_selection_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    #[derive(Default)]
    struct InventoryRecorder {
        projections: AtomicUsize,
        projected: Mutex<Option<Arc<DiscoverV2CapabilitiesResponse>>>,
        steps: Mutex<
            Vec<(
                Arc<DiscoverV2CapabilitiesResponse>,
                Arc<DiscoverV2CapabilitiesResponse>,
            )>,
        >,
    }

    impl McpServerContributor<Config> for InventoryRecorder {
        fn id(&self) -> &'static str {
            "inventory-refresh-recorder"
        }

        fn contribute<'a>(
            &'a self,
            context: McpServerContributionContext<'a, Config>,
        ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
            Box::pin(async move {
                self.projections.fetch_add(1, Ordering::SeqCst);
                *self.projected.lock().await = context
                    .executor_capability_discovery_v2()
                    .and_then(|snapshots| snapshots.first())
                    .and_then(|snapshot| snapshot.result.as_ref().ok())
                    .cloned();
                Vec::new()
            })
        }
    }

    impl codex_extension_api::ToolContributor for InventoryRecorder {
        fn prepare_for_step<'a>(
            &'a self,
            input: codex_extension_api::StepPreparationInput<'a>,
        ) -> ExtensionFuture<'a, ()> {
            Box::pin(async move {
                let snapshots = input
                    .executor_capability_discovery_v2
                    .expect("captured inventories");
                let inventory = snapshots
                    .first()
                    .and_then(|snapshot| snapshot.result.as_ref().ok())
                    .expect("step V2 inventory")
                    .clone();
                let projected = self.projected.lock().await.clone().expect("MCP inventory");
                let mut steps = self.steps.lock().await;
                steps.push((inventory, projected));
                // Model a fresh inventory after each step, without changing selected roots.
                input
                    .thread_store
                    .insert(ExecutorCapabilityDiscoveryV2Cache::default());
            })
        }
    }

    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let model = responses::start_mock_server().await;
    let recorder = Arc::new(InventoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_exec_server_url(executor.url.clone())
        .with_config(|config| {
            config.update_plan_enabled = true;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .expect("enable discovery V2");
        })
        .build_with_auto_env(&model)
        .await?;
    let first = responses::mount_sse_once(
        &model,
        responses::sse(vec![
            responses::ev_response_created("first"),
            responses::ev_function_call(
                "plan-call",
                "update_plan",
                r#"{"plan":[{"step":"Say hello","status":"completed"}]}"#,
            ),
            responses::ev_completed("first"),
        ]),
    )
    .await;
    let second = responses::mount_sse_once(
        &model,
        responses::sse(vec![
            responses::ev_response_created("second"),
            responses::ev_completed("second"),
        ]),
    )
    .await;
    test.submit_turn("Say hello.").await?;
    first.single_request();
    second.single_request();
    {
        let steps = recorder.steps.lock().await;
        let (initial, _) = steps.first().context("first captured inventory")?;
        assert!(
            steps
                .iter()
                .any(|(inventory, _)| !Arc::ptr_eq(initial, inventory)),
            "the next step must acquire a fresh inventory"
        );
        for (inventory, projected) in steps.iter() {
            assert!(
                Arc::ptr_eq(inventory, projected),
                "MCP must project the step's changed inventory even when selections are unchanged"
            );
        }
    }
    let projections = recorder.projections.load(Ordering::SeqCst);
    // Refresh before another step can discover the new inventory. This fixture has no Apps
    // server, but the refresh must still publish exactly once before reporting that absence.
    let error = test
        .codex
        .refresh_codex_apps_tools()
        .await
        .err()
        .expect("no Apps server");
    assert!(error.to_string().contains("unknown MCP server"));
    assert_eq!(recorder.projections.load(Ordering::SeqCst), projections + 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_discovery_failures_retry_or_remain_cached_by_error_kind() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for failure in [DiscoveryFailure::Server, DiscoveryFailure::Disconnect] {
        let executor = Executor::start(/*supports_v2*/ true, failure).await?;
        let manager = codex_exec_server::EnvironmentManager::create_for_tests(
            Some(executor.url.clone()),
            /*local_runtime_paths*/ None,
        )
        .await;
        let environment = manager
            .default_environment()
            .context("remote environment")?;
        let cache = codex_exec_server::ExecutorCapabilityDiscoveryV2Cache::default();
        let request = codex_exec_server::DiscoverV2CapabilitiesRequest {
            cwd: codex_utils_path_uri::PathUri::parse("file:///repo")?,
            sandbox: None,
        };
        assert!(
            cache
                .discover("executor", &environment, request.clone())
                .await
                .is_err()
        );
        assert!(!cache.take_recovered_discovery());
        let (first, concurrent) = tokio::join!(
            cache.discover("executor", &environment, request.clone()),
            cache.discover("executor", &environment, request.clone()),
        );
        let expected_calls = match failure {
            DiscoveryFailure::Server => {
                assert!(first.is_err());
                assert!(concurrent.is_err());
                let next_turn = codex_exec_server::ExecutorCapabilityDiscoveryV2Cache::default();
                next_turn
                    .discover("executor", &environment, request.clone())
                    .await
                    .map_err(anyhow::Error::msg)?;
                assert!(!next_turn.take_recovered_discovery());
                assert!(!cache.take_recovered_discovery());
                2
            }
            DiscoveryFailure::Disconnect => {
                let first = first.map_err(anyhow::Error::msg)?;
                let concurrent = concurrent.map_err(anyhow::Error::msg)?;
                assert!(Arc::ptr_eq(&first, &concurrent));
                assert!(cache.take_recovered_discovery());
                assert!(!cache.take_recovered_discovery());
                let reused = cache
                    .discover("executor", &environment, request.clone())
                    .await
                    .map_err(anyhow::Error::msg)?;
                assert!(Arc::ptr_eq(&first, &reused));
                assert!(!cache.take_recovered_discovery());
                // Two disconnected attempts, then one shared successful retry in the same cache.
                3
            }
            DiscoveryFailure::None => unreachable!("the test only exercises failures"),
        };
        assert_eq!(
            executor
                .requests
                .lock()
                .await
                .iter()
                .filter(|request| request["method"] == "capabilities/discoverV2")
                .count(),
            expected_calls,
        );
    }
    Ok(())
}

async fn check_acquisition(enabled: bool, supported: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let executor = Executor::start(supported, DiscoveryFailure::None).await?;
    let model = responses::start_mock_server().await;
    let observed = Arc::new(Mutex::new(Vec::new()));
    let selected_plugins = Arc::new(Mutex::new(None));
    let mcp_snapshots_per_step = Arc::new(Mutex::new(Vec::new()));
    let mut extensions = ExtensionRegistryBuilder::new();
    let recorder = Arc::new(DiscoveryRecorder {
        snapshots: Arc::clone(&observed),
        selected_plugins: Arc::clone(&selected_plugins),
        mcp_snapshots_per_step: Arc::clone(&mcp_snapshots_per_step),
    });
    extensions.prompt_contributor(recorder.clone());
    extensions.tool_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder);
    codex_skills_extension::install_with_providers(
        &mut extensions,
        codex_skills_extension::SkillProviders::new().with_executor_provider(Arc::new(
            codex_skills_extension::ExecutorSkillProvider::new_with_restriction_product(
                Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
                /*restriction_product*/ None,
            ),
        )),
        |config: &Config| codex_skills_extension::SkillsExtensionConfig {
            include_instructions: true,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_exec_server_url(executor.url.clone())
        .with_config(move |config| {
            config.update_plan_enabled = true;
            assert!(config.features.enable(Feature::Plugins).is_ok());
            assert!(
                config
                    .features
                    .enable(Feature::ExecutorCapabilityDiscovery)
                    .is_ok()
            );
            if enabled {
                assert!(
                    config
                        .features
                        .enable(Feature::ExecutorCapabilityDiscoveryV2)
                        .is_ok()
                );
            }
        })
        .build_with_auto_env(&model)
        .await?;
    let calls = || async {
        executor
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request["method"] == "capabilities/discoverV2")
            .count()
    };
    let acquires_v2 = enabled && supported;
    if acquires_v2 {
        observed.lock().await.clear();
        mcp_snapshots_per_step.lock().await.clear();
        let _ = test.codex.current_mcp_config_and_runtime_context().await;
        let initial_calls = calls().await;
        let _ = test.codex.current_mcp_config_and_runtime_context().await;
        assert!(initial_calls > 0);
        assert_eq!(
            calls().await,
            initial_calls,
            "idle acquisitions reuse the cache"
        );
        let observed = observed.lock().await;
        assert!(!observed.is_empty());
        assert!(observed.iter().all(|snapshot| snapshot.result.is_ok()));
    }
    let initial_calls = calls().await;
    let mut selection = test.executor_environment().selection().clone();
    selection.environment_id = codex_exec_server::REMOTE_ENVIRONMENT_ID.to_string();
    let cwd = selection.cwd.clone();
    selection.workspace_roots = vec![cwd.clone()];
    selection.config =
        codex_protocol::protocol::EnvironmentConfigState::Ready(codex_core::EnvironmentConfig {
            environment_hooks: Default::default(),
            allow_login_shell: false,
            workspace_roots: selection.workspace_roots.clone(),
            permission_profile: codex_protocol::models::PermissionProfileSnapshot::legacy(
                test.config.permissions.permission_profile().clone(),
            ),
            shell_environment_policy: Default::default(),
            windows_sandbox_level: WindowsSandboxLevel::from_config(&test.config),
            windows_sandbox_type: test.config.permissions.windows_sandbox_type,
            use_legacy_landlock: test.config.features.use_legacy_landlock(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: if acquires_v2 {
                Vec::new()
            } else {
                vec![codex_protocol::capabilities::SelectedCapabilityRoot {
                    id: "legacy-root".to_string(),
                    location: codex_protocol::capabilities::CapabilityRootLocation::Environment {
                        environment_id: selection.environment_id.clone(),
                        path: cwd.clone(),
                    },
                }]
            },
        });
    // Only the successful lifecycle case needs two turns and multiple model steps.
    let turns = if acquires_v2 { 2 } else { 1 };
    let mut previous_inventory = None;
    for turn in 0..turns {
        observed.lock().await.clear();
        mcp_snapshots_per_step.lock().await.clear();
        *selected_plugins.lock().await = None;
        let before_turn = calls().await;
        let mut bodies = Vec::new();
        if acquires_v2 {
            bodies.push(responses::sse(vec![
                responses::ev_response_created("plan"),
                responses::ev_function_call(
                    "plan-call",
                    "update_plan",
                    r#"{"plan":[{"step":"Say hello","status":"completed"}]}"#,
                ),
                responses::ev_completed("plan"),
            ]));
        }
        bodies.push(responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_completed("response"),
        ]));
        let mut responses = Vec::new();
        for body in bodies {
            responses.push(responses::mount_sse_once(&model, body).await);
        }
        let prompt = if acquires_v2 {
            "Use $inert-sentinel to say hello."
        } else {
            "Say hello."
        };
        test.submit_turn_with_environments(prompt, Some(vec![selection.clone()]))
            .await?;
        if acquires_v2 {
            let counts = mcp_snapshots_per_step.lock().await;
            let first = counts
                .first()
                .context("MCP snapshot count for the first step")?;
            assert!(
                counts.len() >= 2,
                "the turn must prepare multiple model steps"
            );
            assert!(
                counts.iter().all(|count| count == first),
                "unchanged V2 read routes must not refresh MCP between steps: {counts:?}"
            );
        }
        let executor_root = if acquires_v2 {
            format!("{}:inert-sentinel@market", selection.environment_id)
        } else {
            "legacy-root".to_string()
        };
        let local_identity = PluginIdentity::Local {
            plugin_id: "inert-sentinel@market".to_string(),
        };
        // Both paths publish accepted Cloud packages and available installations only.
        let mut expected_plugins = vec![
            (Some(executor_root), local_identity),
            (
                None,
                PluginIdentity::Remote {
                    remote_plugin_id: "plugin_sentinel".to_string(),
                },
            ),
        ];
        if enabled && !supported {
            expected_plugins.retain(|(root, _)| root.is_none());
        }
        {
            let selected_plugins = selected_plugins.lock().await;
            let snapshot = selected_plugins.as_ref().context("step plugin snapshot")?;
            assert_eq!(
                snapshot
                    .plugins
                    .iter()
                    .map(|plugin| (plugin.selected_root_id.clone(), plugin.plugin_id.clone()))
                    .collect::<Vec<_>>(),
                expected_plugins,
            );
            assert_eq!(snapshot.disabled_plugin_ids, Vec::<String>::new());
        }
        let model_requests = responses
            .iter()
            .flat_map(core_test_support::responses::ResponseMock::requests)
            .collect::<Vec<_>>();
        assert_eq!(model_requests.len(), if acquires_v2 { 2 } else { 1 });
        for request in model_requests {
            if acquires_v2 {
                assert!(
                    request
                        .body_json()
                        .to_string()
                        .contains("Sentinel instructions from the captured executor")
                );
            }
            let developer_text = request.message_input_texts("developer").join("\n");
            assert_eq!(
                developer_text
                    .contains("- inert-sentinel: Executor skill used by the acquisition test"),
                acquires_v2,
                "only a successful V2 inventory renders the skill catalog",
            );
        }
        let turn_calls = calls().await;
        if acquires_v2 {
            assert!(
                turn_calls > before_turn,
                "a new turn must acquire fresh inventory"
            );
            if turn > 0 {
                assert_eq!(
                    turn_calls,
                    before_turn + 1,
                    "steps share the new turn's cache"
                );
            }
            let observed = observed.lock().await;
            let inventory = observed
                .last()
                .context("ordinary turn must project its V2 inventory into MCP")?
                .result
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            assert_eq!(
                inventory.plugins.first().context("inert plugin")?.id,
                "inert-sentinel@market"
            );
            if let Some(previous) = &previous_inventory {
                assert!(
                    !Arc::ptr_eq(previous, inventory),
                    "MCP contributors must receive the new turn's inventory"
                );
            }
            previous_inventory = Some(Arc::clone(inventory));
        } else {
            assert_eq!(turn_calls, 0);
            assert!(observed.lock().await.is_empty());
        }
    }
    let requests = executor.requests.lock().await.clone();
    assert_eq!(
        requests
            .iter()
            .any(|request| request["method"] == "capabilityRoots/discoverV1"),
        !enabled,
        "the V2 flag selects one protocol, including when no executor supplies a V2 snapshot",
    );
    if acquires_v2 {
        let reads = requests
            .iter()
            .filter(|request| request["method"] == "fs/open")
            .collect::<Vec<_>>();
        assert!(
            !reads.is_empty(),
            "explicit mentions must read through the captured executor"
        );
        for read in reads {
            assert!(read["params"]["sandbox"].is_object());
            assert_eq!(
                read["params"]["sandbox"]["policyContext"]["cwd"],
                json!(cwd)
            );
        }
    }
    let discovery = requests
        .iter()
        .filter(|request| request["method"] == "capabilities/discoverV2")
        .collect::<Vec<_>>();
    for request in discovery.iter().skip(initial_calls) {
        assert_eq!(request["params"]["cwd"], json!(cwd));
    }
    for request in discovery {
        assert!(request["params"]["sandbox"].is_object());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_discovery_cache_reuses_matching_requests_and_isolates_execution_contexts() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    let root = tempfile::tempdir()?;
    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let manager = codex_exec_server::EnvironmentManager::create_for_tests(
        Some(executor.url.clone()),
        /*local_runtime_paths*/ None,
    )
    .await;
    let environment = manager
        .default_environment()
        .context("remote environment")?;
    let cache = codex_exec_server::ExecutorCapabilityDiscoveryV2Cache::default();
    let request = codex_exec_server::DiscoverV2CapabilitiesRequest {
        cwd: codex_utils_path_uri::PathUri::from_host_native_path(root.path())?,
        sandbox: None,
    };
    let calls = || async {
        executor
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request["method"] == "capabilities/discoverV2")
            .count()
    };
    assert_eq!(
        calls().await,
        0,
        "creating a cache must not request discovery"
    );
    let (first, concurrent) = tokio::join!(
        cache.discover("executor", &environment, request.clone()),
        cache.discover("executor", &environment, request.clone()),
    );
    let first = first.map_err(anyhow::Error::msg)?;
    assert!(Arc::ptr_eq(
        &first,
        &concurrent.map_err(anyhow::Error::msg)?
    ));
    assert_eq!(calls().await, 1);
    let restricted = codex_exec_server::DiscoverV2CapabilitiesRequest {
        sandbox: Some(
            codex_exec_server::FileSystemSandboxContext::from_legacy_sandbox_policy(
                codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
                request.cwd.clone(),
            )?,
        ),
        ..request.clone()
    };
    cache
        .discover("executor", &environment, restricted)
        .await
        .map_err(anyhow::Error::msg)?;
    let original = cache
        .discover("executor", &environment, request.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    assert!(Arc::ptr_eq(&first, &original));
    assert_eq!(calls().await, 2);
    cache
        .discover(
            "executor",
            &environment,
            codex_exec_server::DiscoverV2CapabilitiesRequest {
                cwd: request.cwd.join("other")?,
                ..request.clone()
            },
        )
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        calls().await,
        3,
        "another cwd must not reuse the same inventory"
    );
    let other = cache
        .discover("other-executor", &environment, request.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    assert!(Arc::ptr_eq(&first, &other));
    assert_eq!(calls().await, 3);
    let replacement_manager = codex_exec_server::EnvironmentManager::create_for_tests(
        Some(executor.url.clone()),
        /*local_runtime_paths*/ None,
    )
    .await;
    let replacement = replacement_manager
        .default_environment()
        .context("replacement environment")?;
    cache
        .discover("executor", &replacement, request.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(calls().await, 4);
    let other_reused = cache
        .discover("other-executor", &environment, request.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    assert!(Arc::ptr_eq(&other, &other_reused));
    assert_eq!(
        calls().await,
        4,
        "a new environment instance must not invalidate the original instance"
    );

    // Four entries remain; reserve enough capacity for a selection larger than 16.
    cache.ensure_capacity(/*minimum*/ 17);
    for index in 0..13 {
        cache
            .discover(
                "executor",
                &environment,
                codex_exec_server::DiscoverV2CapabilitiesRequest {
                    cwd: request.cwd.join(&format!("context-{index}"))?,
                    ..request.clone()
                },
            )
            .await
            .map_err(anyhow::Error::msg)?;
    }
    let oldest = cache
        .discover("other-executor", &environment, request.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    assert!(Arc::ptr_eq(&other, &oldest));
    assert_eq!(calls().await, 17);

    cache
        .discover(
            "executor",
            &environment,
            codex_exec_server::DiscoverV2CapabilitiesRequest {
                cwd: request.cwd.join("overflow")?,
                ..request.clone()
            },
        )
        .await
        .map_err(anyhow::Error::msg)?;
    // Reading the oldest entry did not promote it: eviction is FIFO, not LRU.
    let reloaded = cache
        .discover("other-executor", &environment, request)
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(calls().await, 19);
    assert!(!Arc::ptr_eq(&other, &reloaded));
    assert_eq!(other.as_ref(), reloaded.as_ref());

    Ok(())
}

struct Cloud {
    available: AtomicBool,
    ready: mpsc::UnboundedSender<(Arc<McpResourceClient>, oneshot::Sender<()>)>,
}

impl PluginProvider for Cloud {
    fn list(&self, _query: PluginListQuery) -> PluginProviderFuture<'_, PluginCatalog> {
        Box::pin(async move {
            Ok(PluginCatalog {
                entries: if !self.available.load(Ordering::SeqCst) {
                    Vec::new()
                } else {
                    vec![PluginCatalogEntry {
                        id: PluginIdentity::Remote {
                            remote_plugin_id: "plugin_demo".into(),
                        },
                        enabled: true,
                        display_name: "Demo".into(),
                        version: "1".into(),
                        mcp_servers: Default::default(),
                        connector_ids: Vec::new(),
                        locations: vec![PluginSourceLocation::Cloud {
                            resource_uri: "plugin://plugin_demo".into(),
                            bundle_uri: None,
                        }],
                    }]
                },
                ..Default::default()
            })
        })
    }
}

impl ToolLifecycleContributor for Cloud {
    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let (resume, wait) = oneshot::channel();
            let client = input
                .session_store
                .get::<McpResourceClient>()
                .expect("MCP resource client should be installed");
            self.ready.send((client, resume)).expect("step observer");
            wait.await.expect("resume after auth rotation");
        })
    }
}

/// Auth refresh updates plugin acceptance before merging skills and capturing the next step.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_rotation_refreshes_plugin_acceptance_before_merged_skills() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    *executor.inventory.lock().await = Some(json!({
        "plugins":[{"id":"inert-sentinel@market","displayName":"inert-sentinel","remotePluginId":"plugin_demo",
            "version":"1","root":"file:///inert-sentinel",
            "files":{"manifest":{"path":"file:///inert-sentinel/.codex-plugin/plugin.json",
                "contents":"{\"name\":\"inert-sentinel\"}"},"mcpConfig":null,"appsConfig":null},
            "skills":[{"name":"inert-sentinel","description":"Executor skill used by the acquisition test",
                "shortDescription":null,"path":"file:///inert-sentinel/SKILL.md",
                "discoveryRoot":{"id":"codex-home-skills","path":"file:///"},"metadata":null}]}],
        "skills":[],"warnings":[]
    }));
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let (ready, mut steps) = mpsc::unbounded_channel();
    let cloud = Arc::new(Cloud {
        available: AtomicBool::new(true),
        ready,
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_lifecycle_contributor(cloud.clone());
    let manager = Arc::new(codex_exec_server::EnvironmentManager::default_for_tests());
    install_plugin_providers(
        &mut extensions,
        PluginProviders::new(Arc::new(ExecutorPluginProvider::new(manager.clone())))
            .with_cloud_provider(cloud.clone()),
    );
    install_with_providers(
        &mut extensions,
        SkillProviders::new()
            // Keep listing the skill after its owning plugin disappears.
            .with_cloud_provider(Arc::new(super::skills_extension::CatalogSkillProvider {
                catalog: SkillCatalog {
                    entries: vec![
                        SkillCatalogEntry::new(
                            SkillPackageId("skill://plugin_demo/demo".into()),
                            SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                            "inert-sentinel",
                            "Accepted plugin skill",
                            SkillResourceId::new("skill://plugin_demo/demo/SKILL.md"),
                        )
                        .with_remote_plugin_id(Some("plugin_demo".into())),
                    ],
                    ..Default::default()
                },
            }))
            .with_executor_provider(Arc::new(
                ExecutorSkillProvider::new_with_restriction_product(
                    manager, /*restriction_product*/ None,
                ),
            )),
        |_config: &Config| SkillsExtensionConfig {
            include_instructions: true,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    // Rotate auth, revoke the owning plugin, then reuse the revoked state.
    let cases = [
        ("initial", true),
        ("rotated", true),
        ("removed", false),
        ("removed", false),
    ];
    let model_responses = (0..cases.len())
        .map(|step| {
            let mut events = vec![responses::ev_response_created(&format!("response-{step}"))];
            if step + 1 < cases.len() {
                events.push(responses::ev_function_call_with_namespace(
                    &format!("read-{step}"),
                    "skills",
                    "read",
                    r#"{"package":"skill://inert-sentinel@market/inert-sentinel"}"#,
                ));
            }
            events.push(responses::ev_completed(&format!("response-{step}")));
            responses::sse(events)
        })
        .collect::<Vec<_>>();
    let model = responses::mount_sse_sequence(&server, model_responses).await;
    let test = apps_enabled_builder(apps.chatgpt_base_url)
        .with_exec_server_url(executor.url.clone())
        .with_auth(CodexAuth::from_external_chatgpt_tokens(
            "header.e30.initial",
            "account",
            /*chatgpt_plan_type*/ None,
        )?)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.cloud_skill_enabled = true;
            config.features.enable(Feature::Plugins).unwrap();
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let rotate = async {
        for (index, &(revision, available)) in cases.iter().enumerate().skip(1) {
            let (client, resume) = tokio::time::timeout(Duration::from_secs(15), steps.recv())
                .await?
                .expect("tool completion");
            cloud.available.store(available, Ordering::SeqCst);
            let previous = client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME);
            if revision != cases[index - 1].0 {
                codex_login::auth::login_with_chatgpt_auth_tokens(
                    test.codex_home_path(),
                    &format!("header.e30.{revision}"),
                    "account",
                    /*chatgpt_plan_type*/ None,
                )?;
                test.thread_manager.auth_manager().reload().await;
                tokio::time::timeout(Duration::from_secs(15), async {
                    while client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME) == previous {
                        tokio::task::yield_now().await;
                    }
                })
                .await?;
            }
            resume.send(()).expect("resume next model step");
        }
        anyhow::Ok(())
    };
    tokio::try_join!(
        test.submit_turn("Inspect the accepted plugin skill."),
        rotate
    )?;
    let requests = model.requests();
    assert_eq!(requests.len(), cases.len());
    for (request, (_, available)) in requests.iter().zip(cases) {
        let instructions = request.message_input_texts("developer");
        let skills = instructions
            .iter()
            .rfind(|text| text.starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG))
            .expect("latest skill instructions");
        assert_eq!(skills.contains("Accepted plugin skill"), available);
    }
    let mut bodies = requests
        .iter()
        .map(responses::ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    // Normalize JSON ordering for the request-history snapshot.
    for body in &mut bodies {
        for item in body["input"].as_array_mut().unwrap() {
            if item["type"] == "function_call_output"
                && let Ok(mut output) =
                    serde_json::from_str::<Value>(item["output"].as_str().unwrap())
            {
                output.sort_all_objects();
                item["output"] = Value::String(output.to_string());
            }
        }
    }
    insta::assert_snapshot!(
        "plugin_acceptance_auth_refresh",
        context_snapshot::format_context_snapshot(
            "Mid-turn auth refresh preserves skill context and executor access. Revoking the plugin removes cloud skill instructions and denies executor reads.",
            &bodies
                .iter()
                .map(context_snapshot::SnapshotEntry::body)
                .collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[test_case::test_case("Stop"; "stop")]
#[test_case::test_case("Interrupt"; "interrupt")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_cleanup_hooks_follow_plugin_selection_without_legacy_roots(
    hook_event: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let hook_calls = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&hook_calls);
    wiremock::Mock::given(wiremock::matchers::path("/hooks"))
        .respond_with(move |request: &wiremock::Request| {
            let request: Value =
                serde_json::from_slice(&request.body).expect("MCP request should be valid JSON");
            let result = match request["method"].as_str() {
                Some("initialize") => json!({
                    "protocolVersion": request["params"]["protocolVersion"],
                    "capabilities": {"tools": {}}, "serverInfo": {"name": "hooks", "version": "1"}
                }),
                Some("notifications/initialized") => return wiremock::ResponseTemplate::new(202),
                Some("tools/list") => json!({"tools": [{
                    "name": "turn_ended", "inputSchema": {"type": "object"}
                }]}),
                Some("tools/call") => {
                    assert_eq!(request["params"]["name"], "turn_ended");
                    assert_eq!(
                        request["params"]["arguments"]["hook_event_name"],
                        hook_event
                    );
                    recorded.fetch_add(1, Ordering::SeqCst);
                    json!({"content": []})
                }
                other => panic!("unexpected MCP request: {other:?}"),
            };
            wiremock::ResponseTemplate::new(200)
                .set_body_json(json!({"jsonrpc": "2.0", "id": request["id"], "result": result}))
        })
        .mount(&model)
        .await;
    let manifest = json!({"name": "computer-use", "hooks": {"hooks": {(hook_event): [{"hooks": [{
        "type": "mcp_tool", "server": "node_repl", "tool": "turn_ended",
        "input": {"hook_event_name": "${hook_event_name}", "session_id": "${session_id}", "turn_id": "${turn_id}"}
    }]}]}}});
    let inventory = json!({"plugins": [{
        "id": "computer-use@openai-bundled", "displayName": "Computer use",
        "remotePluginId": null, "version": "1", "root": "file:///plugins/computer-use",
        "files": {"manifest": {"path": "file:///plugins/computer-use/.codex-plugin/plugin.json",
            "contents": manifest.to_string()}, "mcpConfig": null, "appsConfig": null}, "skills": []
    }], "skills": [], "warnings": []});
    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    *executor.inventory.lock().await = Some(inventory);
    let hook_url = format!("{}/hooks", model.uri());
    let hook_server = serde_json::from_value(json!({
        "url": hook_url, "environment_id": codex_exec_server::REMOTE_ENVIRONMENT_ID
    }))?;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_mcp_extension::install_plugins(
        &mut extensions,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    let home = Arc::new(tempfile::TempDir::new()?);
    std::fs::write(
        home.path().join("config.toml"),
        "[plugins.\"computer-use@openai-bundled\"]\nenabled = true\n",
    )?;
    let test = test_codex()
        .with_home(home)
        .with_extensions(Arc::new(extensions.build()))
        .with_exec_server_url(executor.url.clone())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Plugins)
                .expect("enable plugins");
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .expect("enable V2 discovery");
            config
                .features
                .disable(Feature::ExecutorCapabilityDiscovery)
                .expect("disable V1 discovery");
            config
                .mcp_servers
                .set(
                    [(String::from("node_repl"), hook_server)]
                        .into_iter()
                        .collect(),
                )
                .expect("configure the executor hook server");
        })
        .build_with_auto_env(&model)
        .await?;
    core_test_support::wait_for_mcp_server(&test.codex, "node_repl").await?;
    let mut selection = test.executor_environment().selection().clone();
    selection.environment_id = codex_exec_server::REMOTE_ENVIRONMENT_ID.to_string();
    selection.selected_capability_roots = Default::default();
    for disabled in [false, true] {
        core_test_support::submit_thread_settings(
            &test.codex,
            codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(
                    TurnEnvironmentSelections::new(
                        test.config.cwd.clone(),
                        vec![selection.clone()],
                    )
                    .into_requests(),
                ),
                disabled_plugin_ids: Some(if disabled {
                    vec!["computer-use@openai-bundled".to_string()]
                } else {
                    Vec::new()
                }),
                ..Default::default()
            },
        )
        .await?;
        let mut response = responses::sse_response(responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_assistant_message("message", "Done."),
            responses::ev_completed("response"),
        ]));
        if hook_event == "Interrupt" {
            response = response.set_delay(std::time::Duration::from_secs(60));
        }
        let requests = responses::mount_response_once(&model, response).await;
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Finish this turn.".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        if hook_event == "Interrupt" {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while requests.requests().is_empty() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .context("interrupted turn should reach inference with its prepared step")?;
            test.codex.submit(Op::Interrupt).await?;
        }
        core_test_support::wait_for_event(&test.codex, |event| {
            if hook_event == "Interrupt" {
                matches!(event, EventMsg::TurnAborted(_))
            } else {
                matches!(event, EventMsg::TurnComplete(_))
            }
        })
        .await;
        if !disabled {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while hook_calls.load(Ordering::SeqCst) == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .context("V2 cleanup hook should run")?;
        } else {
            // Cleanup is asynchronous; turn completion alone cannot prove disablement.
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(1), async {
                    while hook_calls.load(Ordering::SeqCst) == 1 {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                })
                .await
                .is_err(),
                "disabled cleanup hook must remain silent"
            );
        }
        assert_eq!(hook_calls.load(Ordering::SeqCst), 1);
    }
    let requests = executor.requests.lock().await;
    assert!(
        requests
            .iter()
            .any(|request| request["method"] == "capabilities/discoverV2")
    );
    assert!(
        !requests
            .iter()
            .any(|request| request["method"] == "capabilityRoots/discoverV1")
    );
    Ok(())
}
