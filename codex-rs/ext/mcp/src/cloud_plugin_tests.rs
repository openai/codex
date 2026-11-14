//! Tests cloud plugin cache reuse, auth recovery, tool calls, and plugin policy.

use super::*;
use crate::PluginProvider;
use crate::PluginProviderError;
use crate::PluginProviderFuture;
use crate::PluginProviders;
use crate::PluginsThreadState;
use crate::install_plugin_providers;
use anyhow::Context;
use codex_config::test_support::CloudConfigBundleFixture;
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
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::Op;
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

/// The host can suppress the first cloud refresh even when plugins are enabled.
#[tokio::test]
async fn cloud_source_policy_skips_startup_discovery() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await?;
    config.features.enable(Feature::Plugins)?;
    config.features.disable(Feature::Apps)?;
    let provider = Arc::new(CloudProvider::default());
    let contributor = PluginContributor {
        providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))))
        .with_cloud_provider(provider.clone())
        .with_cloud_enabled(|config| config.features.enabled(Feature::Apps)),
    };
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    contributor
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &codex_protocol::protocol::SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    contributor
        .refresh_cloud_catalog(CloudCatalogRefreshInput {
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await
        .expect("disabled discovery succeeds");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn disabling_plugins_clears_cloud_catalog_and_skips_discovery() -> anyhow::Result<()> {
    let config_home = tempfile::tempdir()?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(config_home.path().to_path_buf())
        .fallback_cwd(Some(config_home.path().to_path_buf()))
        .build()
        .await?;
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
        ("failed-plugins-off", false, Ok(cloud_catalog())),
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
                .refresh_catalog_for_step(StepCatalogRefreshInput {
                    turn_id,
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
                /*executor_capability_discovery_v2*/ None,
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
    assert!(state.contributor_state().publish_cloud_catalog(
        old_auth.as_ref(),
        /*resource_cache_key*/ None,
        cloud_catalog()
    ));
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
        /*executor_capability_discovery_v2*/ None,
    )
    .with_auth_changed(/*auth_changed*/ true);
    let mut loading = std::pin::pin!(contributor.selected_plugins(context, &config.plugins));
    assert!(futures::poll!(&mut loading).is_pending());
    assert_eq!(state.cloud_catalog(), None);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn partial_or_warning_cloud_catalogs_retry_with_backoff_and_recover() {
    for (is_partial, warnings) in [
        (true, Vec::new()),
        (false, vec!["catalog diagnostic".to_string()]),
        (true, vec!["catalog diagnostic".to_string()]),
    ] {
        for initial_client in [
            None,
            Some(McpResourceClient::new(Arc::new(
                codex_mcp::McpRuntime::empty(/*prefix_mcp_tool_names*/ false),
            ))),
        ] {
            let provider = Arc::new(CloudProvider::default());
            let retryable = PluginCatalog {
                is_partial,
                warnings: warnings.clone(),
                ..cloud_catalog()
            };
            provider.set_reply(Ok(retryable.clone()));
            let contributor = PluginContributor {
                providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
                    EnvironmentManager::default_for_tests(),
                ))))
                .with_cloud_provider(provider.clone()),
            };
            let session = ExtensionData::new("session");
            if let Some(client) = initial_client {
                session.insert(client);
            }
            let thread = ExtensionData::new("thread");
            let turn = ExtensionData::new("turn");
            let input = StepCatalogRefreshInput {
                turn_id: "turn",
                session_store: &session,
                thread_store: &thread,
                turn_store: &turn,
            };
            let next_turn = ExtensionData::new("next turn");
            let next_input = StepCatalogRefreshInput {
                turn_id: "next turn",
                turn_store: &next_turn,
                ..input
            };
            let state = thread.get_or_init(PluginsThreadState::default);
            state.contributor_state().plugins_enabled = true;
            for (index, delay) in [5, 10, 20, 40, 80, 160, 300, 300].into_iter().enumerate() {
                assert!(contributor.refresh_catalog_for_step(input).await);
                assert_eq!(state.cloud_catalog(), Some(retryable.clone()));
                assert_eq!(provider.calls.load(Ordering::SeqCst), index + 1);
                tokio::time::advance(Duration::from_secs(delay - 1)).await;
                assert!(!contributor.refresh_catalog_for_step(next_input).await);
                assert_eq!(provider.calls.load(Ordering::SeqCst), index + 1);
                tokio::time::advance(Duration::from_secs(1)).await;
            }
            let complete = PluginCatalog {
                warnings: Vec::new(),
                ..cloud_catalog()
            };
            provider.set_reply(Ok(complete.clone()));
            assert!(contributor.refresh_catalog_for_step(input).await);
            assert_eq!(state.cloud_catalog(), Some(complete.clone()));
            assert!(state.contributor_state().cloud_retry.is_none());
            tokio::time::advance(Duration::from_secs(600)).await;
            assert!(!contributor.refresh_catalog_for_step(input).await);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 9);

            // A new auth scope bypasses a retry cooldown immediately.
            provider.set_reply(Ok(retryable));
            session.insert(McpResourceClient::new(Arc::new(
                codex_mcp::McpRuntime::empty(/*prefix_mcp_tool_names*/ false),
            )));
            assert!(contributor.refresh_catalog_for_step(input).await);
            assert!(!contributor.refresh_catalog_for_step(input).await);
            session.insert(McpResourceClient::new(Arc::new(
                codex_mcp::McpRuntime::empty(/*prefix_mcp_tool_names*/ false),
            )));
            provider.set_reply(Ok(complete.clone()));
            assert!(contributor.refresh_catalog_for_step(input).await);
            assert_eq!(state.cloud_catalog(), Some(complete));
            assert_eq!(provider.calls.load(Ordering::SeqCst), 11);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn cloud_refresh_reuses_catalogs_and_retries() {
    let new_catalog = PluginCatalog::default();
    for (reply, cancelled) in [
        (Ok(cloud_catalog()), false),
        (Err("discovery failed".to_string()), false),
        (Ok(cloud_catalog()), true),
    ] {
        let succeeded = reply.is_ok() && !cancelled;
        let expected_catalog = if succeeded {
            cloud_catalog()
        } else {
            new_catalog.clone()
        };
        let provider = Arc::new(CloudProvider::default());
        provider.set_reply(reply);
        let contributor = PluginContributor {
            providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
                EnvironmentManager::default_for_tests(),
            ))))
            .with_cloud_provider(provider.clone()),
        };
        let session = ExtensionData::new("session");
        let thread = ExtensionData::new("thread");
        let turn = ExtensionData::new("turn");
        let next_turn = ExtensionData::new("next turn");
        let input = StepCatalogRefreshInput {
            turn_id: "turn",
            session_store: &session,
            thread_store: &thread,
            turn_store: &turn,
        };
        let state = thread.get_or_init(PluginsThreadState::default);
        state.contributor_state().plugins_enabled = true;

        // Core serializes callers; exercise the next preparation after completion or cancellation.
        provider.pause_discovery.store(true, Ordering::SeqCst);
        let mut turn_start = contributor.refresh_catalog_for_step(input);
        assert!(futures::poll!(&mut turn_start).is_pending());
        let next_input = StepCatalogRefreshInput {
            turn_id: "next turn",
            turn_store: &next_turn,
            ..input
        };
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        provider.set_reply(Ok(new_catalog.clone()));
        if cancelled {
            drop(turn_start);
        } else {
            provider.resume_discovery.notify_one();
            assert_eq!(turn_start.await, succeeded);
        }
        // Later turns share failure cooldowns; cancellation still allows immediate retry.
        assert_eq!(
            contributor.refresh_catalog_for_step(next_input).await,
            cancelled
        );
        if !succeeded && !cancelled {
            assert_eq!(state.cloud_catalog(), None);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
            tokio::time::advance(Duration::from_secs(5)).await;
            assert!(contributor.refresh_catalog_for_step(next_input).await);
        }
        assert_eq!(state.cloud_catalog(), Some(expected_catalog.clone()));
        assert!(!contributor.refresh_catalog_for_step(next_input).await);
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            1 + usize::from(!succeeded),
        );

        provider.set_reply(Err("discovery failed".to_string()));

        // Clearing an old auth scope still requires rebuilding, even on failure.
        session.insert(McpResourceClient::new(Arc::new(
            codex_mcp::McpRuntime::empty(/*prefix_mcp_tool_names*/ false),
        )));
        assert!(contributor.refresh_catalog_for_step(next_input).await);
        assert_eq!(state.cloud_catalog(), None);
        assert!(!contributor.refresh_catalog_for_step(next_input).await);
        provider.set_reply(Ok(new_catalog.clone()));
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(contributor.refresh_catalog_for_step(next_input).await);
    }
}

#[tokio::test(start_paused = true)]
async fn explicit_cloud_refresh_replaces_cached_catalog() {
    let provider = Arc::new(CloudProvider::default());
    provider.set_reply(Ok(cloud_catalog()));
    let contributor = PluginContributor {
        providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))))
        .with_cloud_provider(provider.clone()),
    };
    let session = ExtensionData::new("session");
    let thread = ExtensionData::new("thread");
    let turn = ExtensionData::new("turn");
    let state = thread.get_or_init(PluginsThreadState::default);
    state.contributor_state().plugins_enabled = true;
    let input = StepCatalogRefreshInput {
        turn_id: "turn",
        session_store: &session,
        thread_store: &thread,
        turn_store: &turn,
    };
    assert!(contributor.refresh_catalog_for_step(input).await);
    let refreshed_catalog = PluginCatalog {
        is_partial: true,
        ..Default::default()
    };
    provider.set_reply(Ok(refreshed_catalog.clone()));
    assert!(!contributor.refresh_catalog_for_step(input).await);
    contributor
        .refresh_cloud_catalog(input.into())
        .await
        .unwrap();
    assert_eq!(state.cloud_catalog(), Some(refreshed_catalog));
    assert!(!contributor.refresh_catalog_for_step(input).await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
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
                        .map(|plugin| {
                            (
                                plugin.plugin_id.as_str().to_string(),
                                plugin.selected_root_id.clone(),
                            )
                        })
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
            enabled: true,
            id: PluginIdentity::Remote {
                remote_plugin_id: "plugin_remote".to_string(),
            },
            display_name: "Cloud Demo".to_string(),
            version: "1.0.0".to_string(),
            mcp_servers: Default::default(),
            connector_ids: vec!["calendar".to_string()],
            locations: vec![codex_core_plugins::PluginSourceLocation::Cloud {
                resource_uri: "plugin://plugin_remote".to_string(),
                bundle_uri: None,
                release_id: None,
            }],
        }],
        ..Default::default()
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
fn cloud_catalog_reuses_cache_and_invalidates_on_resource_or_auth_change() -> anyhow::Result<()> {
    run_test_with_large_stack(
        "cloud-plugin-lifecycle",
        run_cloud_catalog_refresh_lifecycle,
    )
}

async fn run_cloud_catalog_refresh_lifecycle() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

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
    let mut disabled_catalog = cloud_catalog();
    disabled_catalog.entries[0].enabled = false;
    let mut partial_catalog = gmail_catalog.clone();
    partial_catalog.is_partial = true;
    let partial_empty_catalog = PluginCatalog {
        is_partial: true,
        warnings: vec!["resource listing was truncated".to_string()],
        ..Default::default()
    };
    let provider = Arc::new(CloudProvider::default());
    let step_packages = Arc::new(StepPackages::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    crate::install(&mut extensions);
    extensions.prompt_contributor(step_packages.clone());
    install_plugin_providers(
        &mut extensions,
        PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))))
        .with_cloud_provider(provider.clone()),
    );
    let mut builder = test_codex()
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(
                "[mcp_servers.codex_apps]\nurl = 'https://example.com/custom'",
            )
            .add_enterprise_requirement("[mcp_servers]")
            .into_loader(),
        )
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
    assert!(!test.config.mcp_servers.get()[codex_mcp::CODEX_APPS_MCP_SERVER_NAME].enabled);
    let calendar = ("calendar", "list_events", "Cloud Demo");
    let gmail = ("gmail", "search_email", "Gmail Demo");
    for (turn, reply, expected_plugin, new_account, tool_call) in [
        (
            "initial-partial",
            Ok(partial_catalog.clone()),
            Some("plugin_gmail"),
            None,
            None,
        ),
        (
            "partial-cached",
            Ok(cloud_catalog()),
            Some("plugin_gmail"),
            None,
            None,
        ),
        (
            "partial-replaces-partial",
            Ok(PluginCatalog {
                is_partial: true,
                ..cloud_catalog()
            }),
            Some("plugin_remote"),
            None,
            Some(calendar),
        ),
        (
            "partial-empty-replaces-partial",
            Ok(partial_empty_catalog.clone()),
            None,
            None,
            None,
        ),
        (
            "partial-empty-cached",
            Ok(cloud_catalog()),
            None,
            None,
            None,
        ),
        (
            "empty",
            Ok(PluginCatalog::default()),
            None,
            None,
            Some(calendar),
        ),
        ("empty-cached", Ok(cloud_catalog()), None, None, None),
        (
            "in-flight-resource-change",
            Ok(cloud_catalog()),
            None,
            None,
            None,
        ),
        (
            "partial-after-complete-empty",
            Ok(partial_catalog.clone()),
            Some("plugin_gmail"),
            None,
            None,
        ),
        (
            "discovered",
            Ok(cloud_catalog()),
            Some("plugin_remote"),
            None,
            Some(calendar),
        ),
        (
            "cached",
            Ok(gmail_catalog.clone()),
            Some("plugin_remote"),
            None,
            Some(calendar),
        ),
        (
            "partial-refresh",
            Ok(partial_catalog.clone()),
            Some("plugin_gmail"),
            None,
            None,
        ),
        (
            "partial-empty-refresh",
            Ok(partial_empty_catalog),
            None,
            None,
            None,
        ),
        (
            "same-account-failure",
            Err("temporarily unavailable".to_string()),
            None,
            None,
            None,
        ),
        (
            "complete-with-warning-disables-plugin",
            Ok(PluginCatalog {
                warnings: vec!["optional bundle hint ignored".to_string()],
                ..disabled_catalog
            }),
            None,
            None,
            None,
        ),
        ("warning-cached", Ok(cloud_catalog()), None, None, None),
        (
            "partial-after-disable",
            Ok(partial_catalog.clone()),
            Some("plugin_gmail"),
            None,
            None,
        ),
        (
            "re-enabled",
            Ok(cloud_catalog()),
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
            "new-account-partial",
            Ok(partial_catalog),
            Some("plugin_gmail"),
            None,
            Some(gmail),
        ),
        (
            "new-account-partial-cached",
            Ok(PluginCatalog::default()),
            Some("plugin_gmail"),
            None,
            Some(gmail),
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
        let cached = matches!(
            turn,
            "cached"
                | "empty-cached"
                | "partial-cached"
                | "partial-empty-cached"
                | "warning-cached"
                | "new-account-partial-cached"
        );
        let in_flight_change =
            matches!(turn, "in-flight-auth-change" | "in-flight-resource-change");
        // Invalidation bypasses retry cooldowns; clean catalogs await invalidation.
        if !cached && turn != "initial-partial" && new_account.is_none() {
            test.codex.submit(Op::RefreshMcpServers).await?;
        }
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
        let expected_catalog = if cached {
            None
        } else {
            reply.as_ref().ok().cloned()
        };
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
        if in_flight_change {
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
                        let previous_resource =
                            resources.server_cache_key(codex_mcp::CODEX_APPS_MCP_SERVER_NAME);
                        if turn == "in-flight-resource-change" {
                            test.codex.submit(Op::RefreshMcpServers).await?;
                            while resources.server_cache_key(codex_mcp::CODEX_APPS_MCP_SERVER_NAME)
                                == previous_resource
                            {
                                tokio::task::yield_now().await;
                            }
                            assert!(
                                resources.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME)
                                    == previous_auth
                            );
                        } else {
                            // Rotate credentials without replacing the captured runtime/client.
                            codex_login::auth::login_with_chatgpt_auth_tokens(
                                test.codex_home_path(),
                                "header.e30.rotated",
                                "account-b",
                                /*chatgpt_plan_type*/ None,
                            )?;
                            test.thread_manager.auth_manager().reload().await;
                            // Auth must be published while cloud discovery remains paused.
                            while resources.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME)
                                == previous_auth
                            {
                                tokio::task::yield_now().await;
                            }
                        }
                        provider.set_reply(Err("retry discovery failed".to_string()));
                        provider.resume_discovery.notify_one();
                        anyhow::Ok(())
                    }
                )
            })
            .await??;
            // Superseded discovery retries on the next step, not before this request.
            assert_eq!(
                provider.calls.load(Ordering::SeqCst),
                previous_discoveries + 1
            );
            let retry = responses::mount_sse_once(
                &server,
                sse(vec![ev_response_created("retry"), ev_completed("retry")]),
            )
            .await;
            test.submit_turn("Retry the catalog after the connection change.")
                .await?;
            assert_eq!(retry.requests().len(), 1);
            // Failed retries retain a fallback only for the same auth.
            let state = test
                .codex
                .thread_extension_data()
                .get::<PluginsThreadState>()
                .context("plugin state missing")?;
            let expected = (turn == "in-flight-resource-change").then(PluginCatalog::default);
            assert_eq!(state.cloud_catalog(), expected);
        } else {
            test.submit_turn("What capabilities are available?").await?;
            if let Some(expected_catalog) = expected_catalog {
                assert_eq!(
                    test.codex
                        .thread_extension_data()
                        .get::<PluginsThreadState>()
                        .context("plugin state missing")?
                        .cloud_catalog(),
                    Some(expected_catalog),
                    "turn {turn}"
                );
            }
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
            previous_discoveries + usize::from(!cached) + usize::from(in_flight_change),
            "turn {turn}"
        );
    }
    // Re-enabling a server-filtered plugin can change Apps resources within one turn.
    let resources = provider.resources.lock().unwrap().clone().unwrap();
    let session = ExtensionData::new("same-turn-session");
    session.insert(resources.as_ref().clone());
    let thread = ExtensionData::new("same-turn-thread");
    let turn = ExtensionData::new("same-turn");
    let state = thread.get_or_init(PluginsThreadState::default);
    state.contributor_state().plugins_enabled = true;
    let contributor = PluginContributor {
        providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(Arc::new(
            EnvironmentManager::default_for_tests(),
        ))))
        .with_cloud_provider(provider.clone()),
    };
    let input = StepCatalogRefreshInput {
        turn_id: "same-turn",
        session_store: &session,
        thread_store: &thread,
        turn_store: &turn,
    };
    provider.set_reply(Ok(PluginCatalog::default()));
    assert!(contributor.refresh_catalog_for_step(input).await);
    provider.set_reply(Ok(cloud_catalog()));
    assert!(!contributor.refresh_catalog_for_step(input).await);
    assert_eq!(state.cloud_catalog(), Some(PluginCatalog::default()));
    let key = resources.server_cache_key(CODEX_APPS_MCP_SERVER_NAME);
    let auth = resources.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME);
    test.codex.submit(Op::RefreshMcpServers).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while resources.server_cache_key(CODEX_APPS_MCP_SERVER_NAME) == key {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert!(resources.server_cache_key(CODEX_APPS_MCP_SERVER_NAME) != key);
    assert!(resources.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME) == auth);
    assert!(contributor.refresh_catalog_for_step(input).await);
    assert_eq!(state.cloud_catalog(), Some(cloud_catalog()));
    let calls = provider.calls.load(Ordering::SeqCst);
    assert!(!contributor.refresh_catalog_for_step(input).await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    Ok(())
}
