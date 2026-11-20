//! Verifies executor discovery, merged plugin routing, acceptance across auth changes, and caching.

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
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::HttpRequestBodyDeltaNotification;
use codex_exec_server::HttpRequestParams;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
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
use core_test_support::test_codex::environment_config_for_selection;
use core_test_support::test_codex::test_codex;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::Weak;
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
    mcp_projections: AtomicUsize,
    snapshots: Arc<Mutex<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>>,
    selected_plugins: Arc<Mutex<Option<SelectedPluginSnapshot>>>,
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
            self.mcp_projections.fetch_add(1, Ordering::Relaxed);
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

pub(super) struct Executor {
    pub(super) url: String,
    pub(super) inventory: Arc<Mutex<Option<Value>>>,
    pub(super) requests: Arc<Mutex<Vec<Value>>>,
    discovery_pause: Arc<Mutex<Option<DiscoveryPause>>>,
    _task: AbortOnDropHandle<Result<()>>,
}

struct DiscoveryPause {
    started: oneshot::Sender<()>,
    resume: oneshot::Receiver<()>,
}

#[derive(Clone, Copy)]
pub(super) enum DiscoveryFailure {
    None,
    Server,
    Disconnect,
    #[cfg_attr(windows, expect(dead_code, reason = "executor hook tests run on Unix"))]
    TurnGrant,
}

impl Executor {
    pub(super) async fn start(supports_v2: bool, failure: DiscoveryFailure) -> Result<Self> {
        Self::start_with_version(supports_v2, failure, "0.164.0").await
    }

    async fn start_with_version(
        supports_v2: bool,
        failure: DiscoveryFailure,
        version: &'static str,
    ) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("ws://{}", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let inventory = Arc::new(Mutex::new(None::<Value>));
        let served_inventory = Arc::clone(&inventory);
        let recorded = Arc::clone(&requests);
        let discovery_pause = Arc::new(Mutex::new(None::<DiscoveryPause>));
        let pause = Arc::clone(&discovery_pause);
        let discovery_attempts = Arc::new(AtomicUsize::new(0));
        let http = codex_exec_server::Environment::create_for_tests(/*exec_server_url*/ None)?
            .get_http_client();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                let recorded = Arc::clone(&recorded);
                let pause = Arc::clone(&pause);
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
                                DiscoveryFailure::TurnGrant if request["params"]["sandbox"].to_string().contains("turn-only-skills") => {
                                    socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32004,"message":"turn discovery failed"}}).to_string().into())).await?;
                                    continue;
                                }
                                DiscoveryFailure::Server if attempt == 0 => {
                                    socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32603,"message":"fixture discovery failure"}}).to_string().into())).await?;
                                    continue;
                                }
                                DiscoveryFailure::None | DiscoveryFailure::Server | DiscoveryFailure::Disconnect | DiscoveryFailure::TurnGrant => {}
                            }
                        }
                        let result = match request["method"].as_str() {
                            Some("initialize") => Some(json!({"sessionId":"acquisition-only", "environmentInfo": {
                                "shell":{"name":"bash","path":"/bin/bash"},
                                "executorVersion":version,
                                "capabilities":{"sandboxedFileStreaming":true,"capabilityDiscoverySandbox":true,"capabilityDiscoveryV2":supports_v2}
                            }})),
                            Some("environment/info") => Some(json!({
                                "shell":{"name":"bash","path":"/bin/bash"},
                                "executorVersion":version,
                                "capabilities":{"sandboxedFileStreaming":true,"capabilityDiscoverySandbox":true,"capabilityDiscoveryV2":supports_v2}
                            })),
                            Some("capabilityRoots/discoverV1") => Some(json!({
                                "roots": request["params"]["roots"].as_array().context("V1 roots")?
                                    .iter().map(|root| json!({
                                        "id":root["id"],"path":root["path"],
                                        "skills":[{"instructions":{
                                            "path":format!("{}/inert-sentinel/SKILL.md", root["path"].as_str().expect("root URI").trim_end_matches('/')),
                                            "contents":"---\nname: inert-sentinel\ndescription: Executor skill used by the acquisition test\n---\n# Sentinel instructions from the captured executor"
                                        }}]
                                    })).collect::<Vec<_>>()
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
                        if request["method"] == "capabilities/discoverV2" {
                            let pause = pause.lock().await.take();
                            if let Some(pause) = pause {
                                let _ = pause.started.send(());
                                pause.resume.await?;
                            }
                        }
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
            discovery_pause,
            _task: AbortOnDropHandle::new(task),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_disabled_skips_inventory_discovery() -> Result<()> {
    check_acquisition(/*enabled*/ false, "0.164.0").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_captured_inventory_survives_refresh_without_restoring_revoked_mcp() -> Result<()> {
    skip_if_no_network!(Ok(()));

    #[derive(Default)]
    struct InventoryRecorder {
        thread: OnceLock<Weak<codex_core::CodexThread>>,
        invalidated_during_capture: AtomicBool,
        projected: Mutex<Option<Arc<DiscoverV2CapabilitiesResponse>>>,
        captured_before_invalidation: Mutex<Option<Arc<DiscoverV2CapabilitiesResponse>>>,
        steps: Mutex<
            Vec<(
                Arc<DiscoverV2CapabilitiesResponse>,
                Arc<DiscoverV2CapabilitiesResponse>,
            )>,
        >,
    }

    impl codex_extension_api::ThreadLifecycleContributor<Config> for InventoryRecorder {
        fn on_thread_start<'a>(
            &'a self,
            input: codex_extension_api::ThreadStartInput<'a, Config>,
        ) -> ExtensionFuture<'a, ()> {
            Box::pin(async move {
                input
                    .session_store
                    .insert_arc(input.mcp_resource_client.expect("MCP resource client"));
            })
        }
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
                *self.projected.lock().await = context
                    .executor_capability_discovery_v2()
                    .and_then(|snapshots| snapshots.first())
                    .and_then(|snapshot| snapshot.result.as_ref().ok())
                    .cloned();
                Vec::new()
            })
        }
    }

    impl codex_extension_api::TurnLifecycleContributor for InventoryRecorder {
        fn refresh_catalog_for_step<'a>(
            &'a self,
            input: codex_extension_api::CatalogRefreshInput<'a>,
        ) -> ExtensionFuture<'a, bool> {
            Box::pin(async move {
                if !self
                    .invalidated_during_capture
                    .swap(true, Ordering::Relaxed)
                {
                    let projected = self.projected.lock().await.clone();
                    *self.captured_before_invalidation.lock().await = projected;
                    let client = input
                        .session_store
                        .get::<McpResourceClient>()
                        .expect("MCP resource client");
                    assert!(client.has_server("revoked").await);
                    let thread = self
                        .thread
                        .get()
                        .and_then(Weak::upgrade)
                        .expect("live test thread");
                    let selection = thread
                        .environment_selections()
                        .await
                        .into_iter()
                        .find(|selection| {
                            selection.environment_id == codex_exec_server::REMOTE_ENVIRONMENT_ID
                        })
                        .expect("selected executor");
                    let mut config = environment_config_for_selection(
                        thread.config().await.as_ref(),
                        &selection,
                    );
                    config.mcp_policy = Some(codex_protocol::mcp_policy::EnvironmentMcpPolicy {
                        servers: Some(Default::default()),
                        ..Default::default()
                    });
                    thread
                        .environment_ready(&selection, config)
                        .await
                        .expect("revoke MCP server");
                    thread.invalidate_executor_discovery();
                    // A cloud catalog change needs another MCP projection, using the
                    // executor inventory this step already captured before invalidation.
                    return true;
                }
                false
            })
        }
    }

    impl codex_extension_api::ToolContributor for InventoryRecorder {
        fn prepare_for_step<'a>(
            &'a self,
            input: codex_extension_api::StepPreparationInput<'a>,
        ) -> ExtensionFuture<'a, ()> {
            Box::pin(async move {
                let client = input
                    .session_store
                    .get::<McpResourceClient>()
                    .expect("MCP resource client");
                assert!(
                    !client.has_server("revoked").await,
                    "the published runtime must enforce revocation even with the step's old inventory"
                );
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
            })
        }
    }

    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let model = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&model).await?;
    let recorder = Arc::new(InventoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(recorder.clone());
    extensions.tool_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder.clone());
    extensions.turn_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_exec_server_url(executor.url.clone())
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "revoked".to_string(),
                serde_json::from_value(json!({
                    "url": format!("{}/api/codex/ps/mcp", apps.chatgpt_base_url),
                    "environment_id": codex_exec_server::REMOTE_ENVIRONMENT_ID,
                }))
                .expect("test MCP config"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("unconstrained MCP config");
            config.update_plan_enabled = true;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .expect("enable discovery V2");
        })
        .build_with_auto_env(&model)
        .await?;
    assert!(recorder.thread.set(Arc::downgrade(&test.codex)).is_ok());
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
        let captured = recorder
            .captured_before_invalidation
            .lock()
            .await
            .clone()
            .context("inventory captured before invalidation")?;
        let steps = recorder.steps.lock().await;
        let (initial, _) = steps.first().context("first captured inventory")?;
        assert!(
            Arc::ptr_eq(initial, &captured),
            "catalog refresh must not reacquire the invalidated executor inventory within this step"
        );
        assert_eq!(steps.len(), 2);
        assert!(
            !Arc::ptr_eq(initial, &steps[1].0),
            "the next step must acquire a fresh inventory"
        );
        for (inventory, projected) in steps.iter() {
            assert!(
                Arc::ptr_eq(inventory, projected),
                "MCP must project the step's changed inventory even when selections are unchanged"
            );
        }
    }
    assert!(recorder.invalidated_during_capture.load(Ordering::Relaxed));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_refresh_during_discovery_applies_to_the_next_step() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let second_executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let model = responses::start_mock_server().await;
    let test = test_codex()
        .with_exec_server_url(executor.url.clone())
        .with_config(|config| {
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .expect("enable discovery V2");
        })
        .build_with_auto_env(&model)
        .await?;
    let original_selection = test.codex.environment_selections().await.remove(0);
    test.thread_manager
        .environment_manager()
        .upsert_environment(
            "second".into(),
            second_executor.url.clone(),
            /*connect_timeout*/ None,
        )?;
    let mut second_selection = original_selection.clone();
    second_selection.environment_id = "second".into();
    second_selection.config = codex_protocol::protocol::EnvironmentConfigState::Ready(
        environment_config_for_selection(&test.config, &second_selection),
    );
    test.codex
        .update_thread_settings(codex_protocol::protocol::ThreadSettingsOverrides {
            environments: Some(codex_protocol::protocol::TurnEnvironmentRequests::new(
                test.config.cwd.clone(),
                vec![
                    original_selection.clone().into_request(),
                    second_selection.into_request(),
                ],
            )),
            ..Default::default()
        })
        .await?;
    responses::mount_sse_once(
        &model,
        responses::sse(vec![responses::ev_completed("ready")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Initialize both environments.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let _ = test.codex.current_mcp_config_and_runtime_context().await;
    let second_calls = || async {
        second_executor
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request["method"] == "capabilities/discoverV2")
            .count()
    };
    let second_before = second_calls().await;
    let calls = || async {
        executor
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request["method"] == "capabilities/discoverV2")
            .count()
    };
    let before = calls().await;
    let (started, captured) = oneshot::channel();
    let (resume, released) = oneshot::channel();
    *second_executor.discovery_pause.lock().await = Some(DiscoveryPause {
        started,
        resume: released,
    });
    test.codex.invalidate_executor_discovery();
    let (_, refreshed) = tokio::join!(test.codex.current_mcp_config_and_runtime_context(), async {
        tokio::time::timeout(Duration::from_secs(/*secs*/ 10), captured).await??;
        // A has finished and B is still loading. Both readers may finish with their
        // captured inventories; repeated refreshes must survive until the next lookup.
        assert_eq!(calls().await, before + 1);
        *executor.inventory.lock().await = Some(json!({"plugins":[],"skills":[],"warnings":[]}));
        *second_executor.inventory.lock().await =
            Some(json!({"plugins":[],"skills":[],"warnings":[]}));
        for _ in 0..3 {
            test.codex.invalidate_executor_discovery();
        }
        resume
            .send(())
            .map_err(|_| anyhow::anyhow!("discovery stopped before release"))?;
        anyhow::Ok(())
    },);
    refreshed?;
    let captured_snapshots = test
        .codex
        .thread_extension_data()
        .get::<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>()
        .context("published inventory")?;
    assert_eq!(captured_snapshots.len(), 2);
    for snapshot in captured_snapshots.iter() {
        assert!(
            !snapshot
                .result
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error}"))?
                .plugins
                .is_empty(),
            "an in-flight reader may finish with its captured inventory"
        );
    }
    assert_eq!(
        calls().await,
        before + 1,
        "refresh does not restart the current discovery"
    );
    assert_eq!(second_calls().await, second_before + 1);

    let response = responses::mount_sse_once(
        &model,
        responses::sse(vec![responses::ev_completed("after-refresh")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Use the refreshed inventories.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    response.single_request();
    let snapshots = test
        .codex
        .thread_extension_data()
        .get::<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>()
        .context("refreshed inventory")?;
    let inventory = snapshots
        .iter()
        .find(|snapshot| snapshot.environment_id == original_selection.environment_id)
        .context("executor inventory")?
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    assert!(inventory.plugins.is_empty(), "the next step must refresh");
    let _ = test.codex.current_mcp_config_and_runtime_context().await;
    assert_eq!(calls().await, before + 2);
    assert_eq!(second_calls().await, second_before + 2);
    assert_eq!(snapshots.len(), 2);
    let second = snapshots
        .iter()
        .find(|snapshot| snapshot.environment_id == "second")
        .context("second inventory")?;
    assert!(
        second
            .result
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .plugins
            .is_empty()
    );
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
                let refreshed = codex_exec_server::ExecutorCapabilityDiscoveryV2Cache::default();
                refreshed
                    .discover("executor", &environment, request.clone())
                    .await
                    .map_err(anyhow::Error::msg)?;
                assert!(!refreshed.take_recovered_discovery());
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
            DiscoveryFailure::None | DiscoveryFailure::TurnGrant => {
                unreachable!("the test only exercises startup failures")
            }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_old_executor_retains_v1_selected_roots() -> Result<()> {
    check_acquisition(/*enabled*/ true, "0.163.0").await
}

pub(super) async fn check_acquisition(enabled: bool, version: &'static str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let executor =
        Executor::start_with_version(/*supports_v2*/ true, DiscoveryFailure::None, version).await?;
    let acquires_v2 = enabled && version == "0.164.0";
    if acquires_v2 {
        *executor.inventory.lock().await = Some(json!({"plugins":[],"skills":[],"warnings":[]}));
    }
    let model = responses::start_mock_server().await;
    let observed = Arc::new(Mutex::new(Vec::new()));
    let selected_plugins = Arc::new(Mutex::new(None));
    let mut extensions = ExtensionRegistryBuilder::new();
    let recorder = Arc::new(DiscoveryRecorder {
        mcp_projections: AtomicUsize::new(0),
        snapshots: Arc::clone(&observed),
        selected_plugins: Arc::clone(&selected_plugins),
    });
    extensions.tool_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder.clone());
    codex_skills_extension::install_with_providers(
        &mut extensions,
        codex_skills_extension::SkillProviders::new().with_executor_provider(Arc::new(
            codex_skills_extension::ExecutorSkillProvider::new_with_restriction_product(
                /*restriction_product*/ None,
            ),
        )),
        |config: &Config| codex_skills_extension::SkillsExtensionConfig {
            include_instructions: true,
            catalog_deltas_enabled: config.features.enabled(Feature::SkillCatalogDeltas),
            omit_descriptions: false,
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
    if acquires_v2 {
        let _ = test.codex.current_mcp_config_and_runtime_context().await;
        let before = calls().await;
        assert!(before > 0);
        {
            let observed = observed.lock().await;
            let inventory = observed
                .last()
                .context("initial inventory")?
                .result
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            assert!(inventory.plugins.is_empty());
            assert!(inventory.skills.is_empty());
        }

        *executor.inventory.lock().await = None; // Serve the sentinel plugin and skill.
        test.codex.invalidate_executor_discovery();
        assert_eq!(calls().await, before, "invalidation is lazy");
        let _ = test.codex.current_mcp_config_and_runtime_context().await;
        assert_eq!(calls().await, before + 1);
        let observed = observed.lock().await;
        let inventory = observed
            .last()
            .context("refreshed inventory")?
            .result
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        assert_eq!(
            inventory.plugins.first().context("refreshed plugin")?.id,
            "inert-sentinel@market"
        );
        assert_eq!(
            inventory.skills.first().context("refreshed skill")?.name,
            "inert-sentinel"
        );
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
    let response = responses::mount_sse_once(
        &model,
        responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    test.submit_turn_with_environments(
        "Use $inert-sentinel to say hello.",
        Some(vec![selection.clone()]),
    )
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
    let request = response.single_request();
    assert!(
        request
            .body_json()
            .to_string()
            .contains("Sentinel instructions from the captured executor")
    );
    let developer_text = request.message_input_texts("developer").join("\n");
    assert!(
        developer_text.contains("- inert-sentinel: Executor skill used by the acquisition test"),
        "both discovery modes must render the skill catalog",
    );
    if acquires_v2 {
        let discovery_calls = calls().await;
        let mcp_projections = recorder.mcp_projections.load(Ordering::Relaxed);
        let second_response = responses::mount_sse_once(
            &model,
            responses::sse(vec![
                responses::ev_response_created("second-response"),
                responses::ev_completed("second-response"),
            ]),
        )
        .await;
        test.submit_turn_with_environments(
            "Use $inert-sentinel to say hello.",
            Some(vec![selection.clone()]),
        )
        .await?;
        second_response.single_request();
        assert_eq!(
            calls().await,
            discovery_calls,
            "an unchanged turn must reuse the discovered inventory"
        );
        assert_eq!(
            recorder.mcp_projections.load(Ordering::Relaxed),
            mcp_projections,
            "an unchanged turn must not reproject MCP"
        );
        let mut history = response.requests();
        history.extend(second_response.requests());
        insta::assert_snapshot!(
            "v2_discovery_invalidation",
            context_snapshot::format_request_history_snapshot(
                "Invalidation makes the new skill available; an unchanged turn reuses discovery and MCP projection.",
                &history,
                &ContextSnapshotOptions::default(),
            )
        );
    } else {
        assert_eq!(calls().await, 0);
        assert!(observed.lock().await.is_empty());
    }
    let requests = executor.requests.lock().await.clone();
    assert_eq!(
        requests
            .iter()
            .any(|request| request["method"] == "capabilityRoots/discoverV1"),
        !acquires_v2,
        "incompatible executors retain V1 discovery even with the V2 flag enabled",
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
            assert!(read["params"]["sandbox"].is_null());
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
async fn v1_reuses_session_and_turn_discovery_across_model_steps() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let executor = Executor::start(/*supports_v2*/ false, DiscoveryFailure::None).await?;
    let model = responses::start_mock_server().await;
    let test = test_codex()
        .with_exec_server_url(executor.url.clone())
        .with_config(|config| {
            config.update_plan_enabled = true;
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .unwrap();
            config
                .features
                .disable(Feature::ExecutorCapabilityDiscoveryV2)
                .unwrap();
            config.permissions.approval_policy = codex_core::config::Constrained::allow_any(
                codex_protocol::protocol::AskForApproval::OnRequest,
            );
            config
                .permissions
                .set_permission_profile(
                    codex_protocol::models::PermissionProfile::from_runtime_permissions(
                        &codex_protocol::permissions::FileSystemSandboxPolicy::restricted(
                            Vec::new(),
                        ),
                        codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
                    ),
                )
                .unwrap();
        })
        .build_with_auto_env(&model)
        .await?;
    let selection = test.codex.environment_selections().await.remove(0);
    let mut config =
        core_test_support::test_codex::environment_config_for_selection(&test.config, &selection);
    config.selected_capability_roots = vec![codex_protocol::capabilities::SelectedCapabilityRoot {
        id: "legacy-root".to_string(),
        location: codex_protocol::capabilities::CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    }];
    test.codex.environment_ready(&selection, config).await?;
    let permissions = codex_protocol::request_permissions::RequestPermissionProfile {
        file_system: Some(
            codex_protocol::models::FileSystemPermissions::from_read_write_path_uris(
                Some(vec![selection.cwd.join("approved")?]),
                /*write*/ None,
            ),
        ),
        ..Default::default()
    };
    let mut sequence = vec![responses::sse(vec![
        responses::ev_function_call(
            "grant",
            "request_permissions",
            &json!({
                "permissions": permissions,
            })
            .to_string(),
        ),
        responses::ev_completed("grant"),
    ])];
    for id in ["first-step", "second-step"] {
        sequence.push(responses::sse(vec![
            responses::ev_function_call(
                id,
                "update_plan",
                r#"{"plan":[{"step":"Inspect skills","status":"completed"}]}"#,
            ),
            responses::ev_completed(id),
        ]));
    }
    sequence.push(responses::sse(vec![responses::ev_completed("done")]));
    let responses = responses::mount_sse_sequence(&model, sequence).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the approved skills across two steps.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = core_test_support::wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestPermissions(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: request.call_id,
            response: codex_protocol::request_permissions::RequestPermissionsResponse {
                permissions,
                scope: codex_protocol::request_permissions::PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(responses.requests().len(), 4);
    let requests = executor.requests.lock().await;
    let discovery = requests
        .iter()
        .filter(|request| request["method"] == "capabilityRoots/discoverV1")
        .collect::<Vec<_>>();
    assert_eq!(
        discovery.len(),
        2,
        "unchanged steps reuse both discovery scopes"
    );
    assert_ne!(
        discovery[0]["params"]["roots"][0]["sandbox"],
        discovery[1]["params"]["roots"][0]["sandbox"]
    );
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
            codex_exec_server::FileSystemSandboxContext::from_permission_profile(
                codex_protocol::models::PermissionProfile::read_only(),
                request.cwd.clone(),
            ),
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
    plugins: AtomicUsize,
    available: AtomicBool,
    mcp_url: String,
    ready: mpsc::UnboundedSender<(Arc<McpResourceClient>, oneshot::Sender<()>)>,
}

impl PluginProvider for Cloud {
    fn list(&self, _query: PluginListQuery) -> PluginProviderFuture<'_, PluginCatalog> {
        Box::pin(async move {
            self.plugins.fetch_add(1, Ordering::SeqCst);
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
                        mcp_servers: [(
                            "notes".into(),
                            json!({
                                "url": self.mcp_url,
                                "default_tools_approval_mode": "approve",
                            }),
                        )]
                        .into(),
                        connector_ids: Vec::new(),
                        locations: vec![PluginSourceLocation::Cloud {
                            resource_uri: "plugin://plugin_demo".into(),
                            bundle_uri: None,
                            release_id: None,
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

/// Merged installations route separately and share plugin acceptance across auth refreshes.
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
    let second_executor = Executor::start(/*supports_v2*/ true, DiscoveryFailure::None).await?;
    let inventory = executor.inventory.lock().await.clone();
    *second_executor.inventory.lock().await = inventory;
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount_with_tools(&server, Arc::new(std::sync::Mutex::new(vec![json!({
        "name": "echo", "description": "Read a note.", "annotations": {"readOnlyHint": true},
        "inputSchema": {"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]}
    })]))).await?;
    let (ready, mut steps) = mpsc::unbounded_channel();
    let cloud = Arc::new(Cloud {
        plugins: AtomicUsize::new(0),
        available: AtomicBool::new(true),
        mcp_url: format!("{}/api/codex/ps/mcp", apps.chatgpt_base_url),
        ready,
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_lifecycle_contributor(cloud.clone());
    let manager = Arc::new(codex_exec_server::EnvironmentManager::default_for_tests());
    install_plugin_providers(
        &mut extensions,
        PluginProviders::new(Arc::new(ExecutorPluginProvider::new(manager)))
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
                    /*restriction_product*/ None,
                ),
            )),
        |_config: &Config| SkillsExtensionConfig {
            include_instructions: true,
            catalog_deltas_enabled: false,
            omit_descriptions: false,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    // Call both installations, then preserve the skill-read/auth-rotation/revocation flow.
    let installations = [
        ("env-a", "mcp__74afdff5f493__notes", &executor),
        ("env-b", "mcp__14afef09931d__notes", &second_executor),
    ];
    let cases = [
        ("initial", true),
        ("initial", true),
        ("initial", true),
        ("rotated", true),
        ("removed", false),
        ("removed", false),
    ];
    let mut model_responses = (0..cases.len())
        .map(|step| {
            let mut events = vec![responses::ev_response_created(&format!("response-{step}"))];
            if let Some((environment, namespace, _)) = installations.get(step) {
                events.push(responses::ev_function_call_with_namespace(
                    &format!("call-{environment}"),
                    namespace,
                    "echo",
                    &json!({"title": environment}).to_string(),
                ));
            } else if step + 1 < cases.len() {
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
    model_responses.push(responses::sse(vec![
        responses::ev_response_created("cached-turn"),
        responses::ev_completed("cached-turn"),
    ]));
    let model = responses::mount_sse_sequence(&server, model_responses).await;
    let test = apps_enabled_builder(apps.chatgpt_base_url)
        .with_exec_server_url(executor.url.clone())
        .with_model_info_override("gpt-5.5", |model| model.supports_search_tool = false)
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
                .enable(Feature::MultiEnvironmentMcp)
                .unwrap();
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let mut selections = Vec::new();
    for (environment, _, executor) in installations {
        test.thread_manager
            .environment_manager()
            .upsert_environment(
                environment.into(),
                executor.url.clone(),
                /*connect_timeout*/ None,
            )?;
        let mut selection = test.executor_environment().selection().clone();
        selection.environment_id = environment.into();
        selections.push(selection);
    }
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
        test.submit_turn_with_environments(
            "Read a note on each computer, then inspect the accepted plugin skill.",
            Some(selections),
        ),
        rotate
    )?;
    // The next turn reuses the empty plugin catalog.
    test.submit_turn("Check the same skills again.").await?;
    assert_eq!(cloud.plugins.load(Ordering::SeqCst), 3);
    let requests = model.requests();
    assert_eq!(requests.len(), cases.len() + 1);
    for (request, (_, available)) in requests.iter().zip(cases) {
        let instructions = request.message_input_texts("developer");
        let skills = instructions
            .iter()
            .rfind(|text| text.starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG))
            .expect("latest skill instructions");
        assert_eq!(skills.contains("Accepted plugin skill"), available);
        for (_, namespace, _) in installations {
            assert_eq!(request.tool_by_name(namespace, "echo").is_some(), available);
        }
    }
    for (environment, _, executor) in installations {
        let mut calls = Vec::new();
        for request in executor.requests.lock().await.iter() {
            if request["method"] == "http/request" {
                let params: HttpRequestParams = serde_json::from_value(request["params"].clone())?;
                let Some(body) = params.body else { continue };
                let body: Value = serde_json::from_slice(body.as_ref())?;
                if body["method"] == "tools/call" {
                    calls.push((
                        body["params"]["name"].clone(),
                        body["params"]["arguments"].clone(),
                    ));
                }
            }
        }
        assert_eq!(calls, vec![(json!("echo"), json!({"title": environment}))]);
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
            "Two installations of one cloud plugin route MCP calls through their own executors. Mid-turn auth refresh preserves skill context and executor access; revocation removes both MCPs and cloud skill instructions and denies executor reads. The next turn reuses the cached catalog.",
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
