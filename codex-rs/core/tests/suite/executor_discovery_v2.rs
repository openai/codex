//! Verifies gated, inert V2 acquisition and cache reuse across Core turns and idle callers.

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_core_plugins::PluginIdentity;
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
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_features::Feature;
use codex_protocol::config_types::WindowsSandboxLevel;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

struct DiscoveryRecorder {
    snapshots: Arc<Mutex<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>>,
    selected_plugins: Arc<Mutex<Option<SelectedPluginSnapshot>>>,
}

impl McpServerContributor<Config> for DiscoveryRecorder {
    fn id(&self) -> &'static str {
        "discovery-recorder"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async {
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
            package_roots.extend(selected_roots.iter().map(|root| root.id.clone()));
            if !selected_roots.is_empty() {
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
                self.snapshots.lock().await.extend_from_slice(snapshots);
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

impl ContextContributor for DiscoveryRecorder {
    fn contribute_world_state<'a>(
        &'a self,
        input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async move {
            if let Some(snapshot) = input.step_store.get::<SelectedPluginSnapshot>() {
                *self.selected_plugins.lock().await = Some(snapshot.as_ref().clone());
            }
            Vec::new()
        })
    }
}

struct Executor {
    url: String,
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
        let recorded = Arc::clone(&requests);
        let discovery_attempts = Arc::new(AtomicUsize::new(0));
        let http = codex_exec_server::Environment::create_for_tests(/*exec_server_url*/ None)?
            .get_http_client();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                let recorded = Arc::clone(&recorded);
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
                            Some("capabilities/discoverV2") => Some(json!({
                                "plugins":[{"id":"inert-sentinel@market","displayName":"inert-sentinel","remotePluginId":null,
                                    "version":"1","root":"file:///inert-sentinel",
                                    "files":{"manifest":{"path":"file:///inert-sentinel/.codex-plugin/plugin.json",
                                        "contents":"{\"name\":\"inert-sentinel\",\"mcpServers\":{\"inert-sentinel\":{\"command\":\"never-run\"}}}"},
                                        "mcpConfig":{"path":"file:///inert-sentinel/.codex-plugin/plugin.json",
                                            "contents":"{\"inert-sentinel\":{\"command\":\"never-run\"}}"},
                                        "appsConfig":null},"skills":[]}],
                                "skills":[{"name":"inert-sentinel","description":"must not enter the prompt",
                                    "shortDescription":null,"path":"file:///inert-sentinel/SKILL.md","metadata":null}],
                                "warnings":[]
                            })),
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
            requests,
            _task: AbortOnDropHandle::new(task),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flag_off_never_requests_v2() -> Result<()> {
    check_acquisition(/*enabled*/ false, /*supported*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_inventory_reaches_mcp_projection_without_loading_skills() -> Result<()> {
    check_acquisition(/*enabled*/ true, /*supported*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_executor_keeps_existing_behavior() -> Result<()> {
    check_acquisition(/*enabled*/ true, /*supported*/ false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_failures_retry_or_remain_cached_by_error_kind() -> Result<()> {
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
    let mut extensions = ExtensionRegistryBuilder::new();
    let recorder = Arc::new(DiscoveryRecorder {
        snapshots: Arc::clone(&observed),
        selected_plugins: Arc::clone(&selected_plugins),
    });
    extensions.prompt_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder);
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
        test.submit_turn_with_environments("Say hello.", Some(vec![selection.clone()]))
            .await?;
        let executor_root = if acquires_v2 {
            format!("{}:inert-sentinel@market", selection.environment_id)
        } else {
            "legacy-root".to_string()
        };
        let local_identity = PluginIdentity::Local {
            plugin_id: "inert-sentinel@market".to_string(),
        };
        // Both paths publish accepted Cloud packages and available installations only.
        let expected_plugins = vec![
            (Some(executor_root), local_identity),
            (
                None,
                PluginIdentity::Remote {
                    remote_plugin_id: "plugin_sentinel".to_string(),
                },
            ),
        ];
        {
            let selected_plugins = selected_plugins.lock().await;
            let snapshot = selected_plugins.as_ref().context("turn plugin snapshot")?;
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
            assert!(!request.body_json().to_string().contains("inert-sentinel"));
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
    if !acquires_v2 {
        assert!(
            requests
                .iter()
                .any(|request| request["method"] == "capabilityRoots/discoverV1")
        );
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
async fn discovery_cache_coalesces_requests_and_separates_cwds_sandboxes_and_executors()
-> Result<()> {
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
