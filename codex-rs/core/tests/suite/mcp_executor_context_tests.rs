//! Hosted MCP contributors observe executor selection without filtering unavailable entries.

use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPlugin;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentSelection;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::environment_config_for_selection;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct ExecutorSelectionRecorder {
    observed: Mutex<Vec<Vec<TurnEnvironmentSelection>>>,
    plugin_contexts: Mutex<Vec<Option<Vec<TurnEnvironmentSelection>>>>,
}

impl McpServerContributor<Config> for ExecutorSelectionRecorder {
    fn id(&self) -> &'static str {
        "executor_selection_recorder"
    }

    fn selected_plugins<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
        _plugins_config: &'a codex_config::types::PluginsConfigToml,
    ) -> ExtensionFuture<'a, Vec<SelectedPlugin<'a>>> {
        Box::pin(async move {
            self.plugin_contexts
                .lock()
                .expect("plugin context recorder lock")
                .push(context.selected_environments().map(<[_]>::to_vec));
            Vec::new()
        })
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            if let Some(environments) = context.selected_environments() {
                self.observed
                    .lock()
                    .expect("executor selection recorder lock")
                    .push(environments.to_vec());
            }
            Vec::new()
        })
    }
}

/// A pending or failed primary must reach hosted contributors ahead of a ready secondary.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_projection_preserves_executor_order_and_unavailable_selections() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let recorder = Arc::new(ExecutorSelectionRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .enable(Feature::DeferredExecutor)
                .expect("enable selected executors");
        })
        .build_with_auto_env(&server)
        .await?;
    let ready = test.executor_environment().request();
    test.thread_manager
        .environment_manager()
        .upsert_environment(
            "unavailable-executor".to_owned(),
            "ws://127.0.0.1:1".to_owned(),
            /*connect_timeout*/ None,
        )?;
    for state in [
        EnvironmentConfigState::Pending,
        EnvironmentConfigState::Failed("configuration unavailable".to_owned()),
    ] {
        let unavailable = TurnEnvironmentRequest {
            environment_id: "unavailable-executor".to_owned(),
            config: state,
            ..ready.clone()
        };
        for requests in [
            vec![unavailable.clone(), ready.clone()],
            vec![ready.clone(), unavailable.clone()],
        ] {
            // These synthetic executor selections have no capability roots.
            let selections = requests
                .iter()
                .cloned()
                .map(|request| TurnEnvironmentSelection::new(request, &[]))
                .collect::<Vec<_>>();
            recorder
                .observed
                .lock()
                .expect("executor selection recorder lock")
                .clear();
            let thread = test
                .thread_manager
                .start_thread(StartThreadOptions {
                    environments: Some(requests),
                    ..StartThreadOptions::new(test.config.clone())
                })
                .await?;
            thread.thread.current_mcp_config_and_runtime_context().await;
            assert_eq!(thread.thread.environment_selections().await, selections);
            {
                let observed = recorder
                    .observed
                    .lock()
                    .expect("executor selection recorder lock");
                assert!(
                    observed.contains(&selections),
                    "MCP projection must receive the complete ordered selection: {observed:?}"
                );
            }
            thread.thread.shutdown_and_wait().await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn model_step_plugin_projection_preserves_selected_environments() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let recorder = Arc::new(ExecutorSelectionRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let selection = test.codex.environment_selections().await.remove(0);
    let mut environment_config = environment_config_for_selection(&test.config, &selection);
    // Step-local plugin projection returns early without a ready selected root.
    environment_config.selected_capability_roots = vec![SelectedCapabilityRoot {
        id: "selected-plugin-root".to_owned(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    }];
    test.codex
        .environment_ready(&selection, environment_config)
        .await?;
    let selections = test.codex.environment_selections().await;
    recorder
        .plugin_contexts
        .lock()
        .expect("plugin context recorder lock")
        .clear();
    core_test_support::responses::mount_sse_once(
        &server,
        core_test_support::responses::sse(vec![core_test_support::responses::ev_completed("done")]),
    )
    .await;

    test.submit_turn("Respond without using tools.").await?;

    let contexts = recorder
        .plugin_contexts
        .lock()
        .expect("plugin context recorder lock");
    assert!(
        !contexts.is_empty(),
        "the model step must project selected plugins"
    );
    assert!(
        contexts
            .iter()
            .all(|context| context.as_ref() == Some(&selections)),
        "every step-local plugin projection must receive its selected environments: {contexts:?}"
    );
    Ok(())
}
