//! Capability discovery stays within the environments captured by the current turn.

use super::*;
use codex_protocol::protocol::TurnEnvironmentRequests;
use codex_protocol::protocol::TurnEnvironmentSelection;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(PermissionProfile::Disabled; "full access")]
#[test_case(PermissionProfile::workspace_write(); "workspace write")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_roots_do_not_connect_unselected_executors(
    permission_profile: PermissionProfile,
) -> Result<()> {
    let server = start_mock_server().await;
    let observed_roots = Arc::new(Mutex::new(Vec::new()));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(Arc::new(ReadyCapabilityRootsTestExtension {
        observed_roots: Some(Arc::clone(&observed_roots)),
    }));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config
                .permissions
                .set_permission_profile(permission_profile)
                .expect("thread permissions should be configurable");
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .expect("capability discovery should be configurable");
        })
        .build_with_auto_env(&server)
        .await?;
    let mut selection = test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .context("thread should select its executor environment")?;
    let selected_root = SelectedCapabilityRoot {
        id: "shared-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    };
    let stale_root = SelectedCapabilityRoot {
        id: selected_root.id.clone(),
        location: CapabilityRootLocation::Environment {
            environment_id: "previous-executor".to_string(),
            path: selection.cwd.clone(),
        },
    };
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let previous_executor = test
        .thread_manager
        .environment_manager()
        .report_environment_provisioning_status(
            "previous-executor".to_string(),
            Ok(EnvironmentReadyInfo {
                selected_capability_roots: vec![stale_root.clone()],
            }),
            provider.clone(),
        )?
        .context("previous executor should remain registered")?;
    let mut environment_config = environment_config_for_selection(&test.config, &selection);
    environment_config.selected_capability_roots = vec![selected_root.clone()];
    selection.config = EnvironmentConfigState::Ready(environment_config);
    let mut thread_extension_init = ExtensionDataInit::new();
    thread_extension_init.insert(vec![stale_root]);
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![selection.clone().into_request()]),
            thread_extension_init,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("selected-environment")]),
            sse(vec![ev_completed("no-environments")]),
        ],
    )
    .await;
    for (environments, expected_roots) in [
        (vec![selection], vec![selected_root]),
        (Vec::new(), Vec::new()),
    ] {
        thread
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "continue in the selected environments".to_string(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    environments: Some(TurnEnvironmentRequests::new(
                        test.config.cwd.clone(),
                        environments
                            .into_iter()
                            .map(TurnEnvironmentSelection::into_request)
                            .collect(),
                    )),
                    ..Default::default()
                }),
            )
            .await?;
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        assert_eq!(
            observed_roots.lock().expect("observed roots").last(),
            Some(&expected_roots)
        );
    }

    assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
    assert!(!previous_executor.startup_finished());
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .message_input_texts("user")
            .contains(&"<ready_capability_roots>shared-root</ready_capability_roots>".to_string())
    );
    Ok(())
}
