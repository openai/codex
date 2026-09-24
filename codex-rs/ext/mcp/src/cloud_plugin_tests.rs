//! Tests cloud plugin lifecycle, tool calls, scheduling, and plugin policy.

use super::*;
use crate::PluginProvider;
use crate::PluginProviderError;
use crate::PluginProviderFuture;
use crate::PluginProviders;
use crate::PluginsThreadState;
use crate::install_plugin_providers;
use anyhow::Context;
use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginCatalogEntry;
use codex_core_plugins::PluginIdentity;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::run_test_with_large_stack;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;

#[tokio::test]
async fn cloud_discovery_selects_regular_task_phase() {
    struct ExistingContributor;
    impl TurnLifecycleContributor for ExistingContributor {}

    let mut builder = ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(Arc::new(ExistingContributor));
    let executor = Arc::new(ExecutorPluginProvider::new(Arc::new(
        EnvironmentManager::default_for_tests(),
    )));
    install_plugin_providers(&mut builder, PluginProviders::new(executor.clone()));
    install_plugin_providers(
        &mut builder,
        PluginProviders::new(executor).with_cloud_provider(Arc::new(CloudProvider::default())),
    );
    let registry = builder.build();
    let thread = ExtensionData::new("thread");
    for plugins_enabled in [false, true] {
        thread
            .get_or_init(PluginsThreadState::default)
            .contributor_state()
            .plugins_enabled = plugins_enabled;
        let dependencies = registry
            .turn_lifecycle_contributors()
            .iter()
            .map(|contributor| contributor.requires_mcp_runtime(&thread))
            .collect::<Vec<_>>();
        assert_eq!(dependencies, vec![false, false, plugins_enabled]);
        let phases = registry
            .turn_lifecycle_contributors()
            .iter()
            .map(|contributor| contributor.turn_start_phase(&thread))
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            vec![
                TurnStartPhase::BeforeTaskRegistration,
                TurnStartPhase::BeforeTaskRegistration,
                if plugins_enabled {
                    TurnStartPhase::RegularTaskStart
                } else {
                    TurnStartPhase::BeforeTaskRegistration
                },
            ],
        );
    }
}

#[tokio::test]
async fn disabling_plugins_clears_cloud_catalog_and_skips_discovery() -> anyhow::Result<()> {
    let config_home = tempfile::tempdir()?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(config_home.path().to_path_buf())
        .fallback_cwd(Some(config_home.path().to_path_buf()))
        .build()
        .await?;
    let mode = CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "test".to_string(),
            reasoning_effort: None,
            developer_instructions: None,
        },
    };
    let provider = Arc::new(CloudProvider::default());
    let mut builder = ExtensionRegistryBuilder::new();
    let providers = PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
        EnvironmentManager::default_for_tests(),
    ))))
    .with_cloud_provider(provider.clone());
    install_plugin_providers(&mut builder, providers);
    let registry = builder.build();
    let session = ExtensionData::new("session");
    let thread = ExtensionData::new("thread");
    let thread_init = codex_extension_api::ExtensionDataInit::default();
    for (turn_id, plugins_enabled, reply) in [
        ("off", false, Ok(cloud_catalog())),
        ("on", true, Ok(cloud_catalog())),
        ("plugins-off", false, Ok(cloud_catalog())),
        (
            "re-enabled-failed",
            true,
            Err("discovery unavailable".to_string()),
        ),
        ("re-enabled", true, Ok(cloud_catalog())),
    ] {
        let expected_catalog = if plugins_enabled {
            reply.clone().ok()
        } else {
            None
        };
        let previous_discoveries = provider.calls.load(Ordering::SeqCst);
        provider.set_reply(reply);
        let previous = config.clone();
        config
            .features
            .set_enabled(Feature::Plugins, plugins_enabled)?;
        for contributor in registry.config_contributors() {
            contributor.on_config_changed(&session, &thread, &previous, &config);
        }
        if !plugins_enabled && let Some(state) = thread.get::<PluginsThreadState>() {
            assert_eq!(state.cloud_catalog(), None);
        }
        for contributor in registry.turn_lifecycle_contributors() {
            contributor
                .on_turn_start(TurnStartInput {
                    turn_id,
                    collaboration_mode: &mode,
                    token_usage_at_turn_start: None,
                    session_store: &session,
                    thread_store: &thread,
                    turn_store: &ExtensionData::new(turn_id),
                })
                .await;
        }
        registry.mcp_server_contributors()[0]
            .contribute(codex_extension_api::McpServerContributionContext::for_step(
                &config,
                &thread_init,
                &thread,
                "test",
                &[],
                /*executor_capability_discovery*/ None,
            ))
            .await;
        if let Some(state) = thread.get::<PluginsThreadState>() {
            assert_eq!(state.cloud_catalog(), expected_catalog, "turn {turn_id}");
        }
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            previous_discoveries + usize::from(plugins_enabled),
            "turn {turn_id}",
        );
    }
    Ok(())
}

#[tokio::test]
async fn account_change_clears_hosted_plugins_before_waiting_for_executor() -> anyhow::Result<()> {
    struct UnreadyExecutor;
    impl codex_exec_server::NoiseRendezvousConnectProvider for UnreadyExecutor {
        fn connect_bundle(
            &self,
            _: codex_exec_server::NoiseChannelPublicKey,
        ) -> futures::future::BoxFuture<
            '_,
            Result<
                codex_exec_server::NoiseRendezvousConnectBundle,
                codex_exec_server::ExecServerError,
            >,
        > {
            Box::pin(futures::future::pending())
        }
    }

    let home = tempfile::tempdir()?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await?;
    config.features.enable(Feature::Plugins)?;
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    manager
        .materialize_pending_noise_environment("unready".to_string(), Arc::new(UnreadyExecutor))?;
    let contributor = PluginContributor {
        providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(manager)))
            .with_cloud_provider(Arc::new(CloudProvider::default())),
    };
    let thread = ExtensionData::new("thread");
    let state = thread.get_or_init(PluginsThreadState::default);
    let old_auth = state
        .contributor_state()
        .prepare_cloud_generation(/*mcp_resources*/ None);
    assert!(
        state
            .contributor_state()
            .publish_cloud_catalog(old_auth.as_ref(), cloud_catalog())
    );
    let roots = [SelectedCapabilityRoot {
        id: "plugin".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "unready".to_string(),
            path: PathUri::parse("file:///plugins/demo")?,
        },
    }];
    let init = codex_extension_api::ExtensionDataInit::default();
    let context = McpServerContributionContext::for_step(
        &config, &init, &thread, "test", &roots, /*executor_capability_discovery*/ None,
    )
    .with_auth_changed(/*auth_changed*/ true);
    let mut loading = std::pin::pin!(contributor.selected_plugins(context));
    assert!(futures::poll!(&mut loading).is_pending());
    assert_eq!(state.cloud_catalog(), None);
    Ok(())
}

struct CloudProvider {
    reply: Mutex<Result<PluginCatalog, String>>,
    calls: AtomicUsize,
    resources: Mutex<Option<Arc<McpResourceClient>>>,
    pause_discovery: AtomicBool,
    discovery_started: Notify,
    resume_discovery: Notify,
}

type ObservedPackages = Vec<(String, Option<String>)>;

#[derive(Default)]
struct StepPackages(Mutex<Option<ObservedPackages>>);

impl ContextContributor for StepPackages {
    fn contribute_world_state<'a>(
        &'a self,
        input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async move {
            *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = input
                .thread_store
                .get::<SelectedPluginSnapshot>()
                .map(|snapshot| {
                    snapshot
                        .plugins
                        .iter()
                        .map(|plugin| (plugin.plugin_id.clone(), plugin.selected_root_id.clone()))
                        .collect()
                });
            Vec::new()
        })
    }
}

impl Default for CloudProvider {
    fn default() -> Self {
        Self {
            reply: Mutex::new(Ok(PluginCatalog::default())),
            calls: AtomicUsize::new(0),
            resources: Mutex::new(None),
            pause_discovery: AtomicBool::new(false),
            discovery_started: Notify::new(),
            resume_discovery: Notify::new(),
        }
    }
}

impl CloudProvider {
    fn set_reply(&self, reply: Result<PluginCatalog, String>) {
        *self
            .reply
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = reply;
    }
}

impl PluginProvider for CloudProvider {
    fn list(&self, query: PluginListQuery) -> PluginProviderFuture<'_, PluginCatalog> {
        *self
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = query.mcp_resources;
        self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = self
            .reply
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let pause = self.pause_discovery.swap(false, Ordering::SeqCst);
        Box::pin(async move {
            if pause {
                self.discovery_started.notify_one();
                self.resume_discovery.notified().await;
            }
            reply.map_err(PluginProviderError::Message)
        })
    }
}

fn cloud_catalog() -> PluginCatalog {
    PluginCatalog {
        entries: vec![PluginCatalogEntry {
            id: PluginIdentity::Remote {
                remote_plugin_id: "plugin_remote".to_string(),
            },
            display_name: "Cloud Demo".to_string(),
            version: Some("1.0.0".to_string()),
            mcp_servers: Default::default(),
            connector_ids: vec!["calendar".to_string()],
            locations: vec![codex_core_plugins::PluginSourceLocation::Cloud {
                resource_uri: "plugin://plugin_remote".to_string(),
                bundle_uri: None,
            }],
        }],
        warnings: Vec::new(),
    }
}

fn app_tool(connector: &str, action: &str) -> serde_json::Value {
    json!({
        "name": format!("{connector}_{action}"),
        "description": format!("Run {action} in {connector}."),
        "annotations": {"readOnlyHint": true},
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
        "_meta": {
            "connector_id": connector,
            "connector_name": connector,
            "_codex_apps": {
                "resource_uri": format!("connector://{connector}/tools/{connector}_{action}"),
                "contains_mcp_source": true,
                "connector_id": connector,
            },
        },
    })
}

#[test]
fn cloud_catalog_refreshes_per_turn_and_invalidates_on_auth_change() -> anyhow::Result<()> {
    run_test_with_large_stack(
        "cloud-plugin-lifecycle",
        run_cloud_catalog_refresh_lifecycle,
    )
}

async fn run_cloud_catalog_refresh_lifecycle() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    // Each account has a fixed inventory; only the auth switch changes the server's tools.
    let apps_tools = Arc::new(Mutex::new(vec![app_tool("calendar", "list_events")]));
    let apps_server = AppsTestServer::mount_with_tools(&server, apps_tools.clone()).await?;
    let mut gmail_catalog = cloud_catalog();
    gmail_catalog.entries[0].id = PluginIdentity::Remote {
        remote_plugin_id: "plugin_gmail".to_string(),
    };
    gmail_catalog.entries[0].display_name = "Gmail Demo".to_string();
    gmail_catalog.entries[0].connector_ids = vec!["gmail".to_string()];
    let provider = Arc::new(CloudProvider::default());
    let step_packages = Arc::new(StepPackages::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(step_packages.clone());
    install_plugin_providers(
        &mut extensions,
        PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))))
        .with_cloud_provider(provider.clone()),
    );
    let mut builder = test_codex()
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_auth(codex_login::CodexAuth::from_external_chatgpt_tokens(
            "header.e30.initial",
            "account-a",
            /*chatgpt_plan_type*/ None,
        )?)
        .with_model_info_override("gpt-5.5", |model| {
            model.supports_search_tool = false;
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Plugins)
                .expect("enable plugins in fixture");
            config
                .features
                .enable(Feature::Apps)
                .expect("enable apps in fixture");
            config.chatgpt_base_url = apps_server.chatgpt_base_url;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let calendar = ("calendar", "list_events", "Cloud Demo");
    let gmail = ("gmail", "search_email", "Gmail Demo");
    for (turn, reply, expected_plugin, new_account, tool_call) in [
        (
            "empty",
            Ok(PluginCatalog::default()),
            None,
            None,
            Some(calendar),
        ),
        (
            "discovered",
            Ok(cloud_catalog()),
            Some("plugin_remote"),
            None,
            Some(calendar),
        ),
        (
            "same-account-failure",
            Err("temporarily unavailable".to_string()),
            Some("plugin_remote"),
            None,
            None,
        ),
        ("removed", Ok(PluginCatalog::default()), None, None, None),
        (
            "restored",
            Ok(cloud_catalog()),
            Some("plugin_remote"),
            None,
            None,
        ),
        (
            "auth-changed-failure",
            Err("unavailable after auth changed".to_string()),
            None,
            Some("account-b"),
            None,
        ),
        (
            "new-account-discovered",
            Ok(gmail_catalog),
            Some("plugin_gmail"),
            None,
            Some(gmail),
        ),
        (
            "in-flight-auth-change",
            Ok(cloud_catalog()),
            None,
            None,
            None,
        ),
    ] {
        let previous_discoveries = provider.calls.load(Ordering::SeqCst);
        if let Some(account) = new_account {
            // Do not manually refresh MCP: the auth transition must pick up this new inventory.
            *apps_tools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                vec![app_tool("gmail", "search_email")];
            let resources = provider
                .resources
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .context("cloud provider resource client missing")?;
            let previous_auth =
                resources.auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME);
            codex_login::auth::login_with_chatgpt_auth_tokens(
                test.codex_home_path(),
                "header.e30.changed",
                account,
                /*chatgpt_plan_type*/ None,
            )?;
            test.thread_manager.auth_manager().reload().await;
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    if resources.auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME)
                        != previous_auth
                        && test
                            .codex
                            .thread_extension_data()
                            .get::<SelectedPluginSnapshot>()
                            .is_some_and(|snapshot| snapshot.plugins.is_empty())
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            assert_eq!(provider.calls.load(Ordering::SeqCst), previous_discoveries);
        }
        provider.set_reply(reply);
        let previous_calls = recorded_apps_tool_calls(&server).await.len();
        let call_id = format!("{turn}-call");
        let mut events = vec![ev_response_created("resp")];
        if let Some((connector, action, _)) = tool_call {
            events.push(ev_function_call_with_namespace(
                &call_id,
                &format!("mcp__codex_apps__{connector}"),
                &format!("_{action}"),
                "{}",
            ));
        }
        events.push(ev_completed("resp"));
        let mut responses = vec![sse(events)];
        if tool_call.is_some() {
            responses.push(sse(vec![
                ev_response_created("after-tool"),
                ev_completed("after-tool"),
            ]));
        }
        let response = mount_sse_sequence(&server, responses).await;
        *step_packages
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        if turn == "in-flight-auth-change" {
            provider.pause_discovery.store(true, Ordering::SeqCst);
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
                tokio::try_join!(
                    test.submit_turn("What capabilities are available?"),
                    async {
                        provider.discovery_started.notified().await;
                        let resources = provider
                            .resources
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone()
                            .context("cloud provider resource client missing")?;
                        let previous_auth = resources
                            .auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME);
                        // Rotate credentials without replacing the captured runtime/client.
                        codex_login::auth::login_with_chatgpt_auth_tokens(
                            test.codex_home_path(),
                            "header.e30.rotated",
                            "account-b",
                            /*chatgpt_plan_type*/ None,
                        )?;
                        test.thread_manager.auth_manager().reload().await;
                        while resources
                            .auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME)
                            == previous_auth
                        {
                            tokio::task::yield_now().await;
                        }
                        provider.resume_discovery.notify_one();
                        anyhow::Ok(())
                    }
                )
            })
            .await??;
            // The old request must not republish its catalog after credentials change.
            let state = test
                .codex
                .thread_extension_data()
                .get::<PluginsThreadState>()
                .context("plugin state missing")?;
            assert_eq!(state.cloud_catalog(), None);
            assert_eq!(
                state
                    .contributor_state()
                    .cloud_generation
                    .as_ref()
                    .and_then(|generation| generation.catalog.clone()),
                None,
            );
        } else {
            test.submit_turn("What capabilities are available?").await?;
        }
        let requests = response.requests();
        let request = &requests[0];
        if let Some((connector, action, plugin_name)) = tool_call {
            assert_eq!(requests.len(), 2);
            let calls = recorded_apps_tool_calls(&server).await;
            let output = requests[1].function_call_output(&call_id);
            assert_eq!(calls.len(), previous_calls + 1, "turn {turn}: {output}");
            let call = calls.last().context("expected an Apps tool call")?;
            assert_eq!(
                call["params"]["name"],
                json!(format!("{connector}_{action}"))
            );
            assert_eq!(call["params"]["arguments"], json!({}));
            let (other_namespace, other_tool) = if connector == "calendar" {
                ("mcp__codex_apps__gmail", "_search_email")
            } else {
                ("mcp__codex_apps__calendar", "_list_events")
            };
            assert!(request.tool_by_name(other_namespace, other_tool).is_none());
            let tool = request
                .tool_by_name(
                    &format!("mcp__codex_apps__{connector}"),
                    &format!("_{action}"),
                )
                .expect("the current account's tool should be advertised");
            assert_eq!(
                tool["description"]
                    .as_str()
                    .context("tool description missing")?
                    .contains(plugin_name),
                expected_plugin.is_some(),
                "cloud discovery should attribute tools to the current account's plugin"
            );
            // The server echoes this URI in its result; it must reach the next model step.
            assert!(
                output["output"]
                    .as_str()
                    .context("tool output missing")?
                    .contains(&format!(
                        "connector://{connector}/tools/{connector}_{action}"
                    )),
                "turn {turn}: {output}"
            );
        }
        assert_eq!(
            *step_packages
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            Some(
                expected_plugin
                    .into_iter()
                    .map(|id| (id.to_owned(), None))
                    .collect()
            ),
            "turn {turn}"
        );
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            previous_discoveries + 1,
            "turn {turn}"
        );
    }
    Ok(())
}
