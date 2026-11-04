//! Capability discovery stays within the environments captured by the current turn.

use super::*;
use codex_history::InitialHistory;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnEnvironmentRequests;
use codex_protocol::protocol::TurnEnvironmentSelection;
use pretty_assertions::assert_eq;
use test_case::test_case;

/// A Linux/macOS executor can exercise the old Windows enum boundary without launching MXC.
#[cfg(not(windows))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_skill_discovery_reports_unsupported_remote_sandbox() -> Result<()> {
    let executor = core_test_support::exec_server::ExecServerProcess::start().await?;
    let environment =
        codex_exec_server::Environment::create_for_tests(Some(executor.websocket_url.clone()))?;
    assert!(!environment.info().await?.capabilities.windows_mxc);
    let workspace = tempfile::tempdir()?;
    std::fs::create_dir(workspace.path().join(".git"))?;
    let root = workspace.path().join(".agents/skills");
    std::fs::create_dir_all(root.join("existing"))?;
    std::fs::write(
        root.join("existing/SKILL.md"),
        "---\nname: existing\ndescription: Existing repository skill.\n---\nSECRET_SKILL_BODY\n",
    )?;
    let cwd = AbsolutePathBuf::from_absolute_path(workspace.path())?;
    let input = codex_skills_extension::HostSkillsLoadInput::new(
        cwd.clone(),
        Vec::new(),
        Default::default(),
    );
    let service = codex_skills_extension::HostSkillsService::new_with_restriction_product(
        cwd.clone(),
        /*bundled_skills_enabled*/ false,
        /*restriction_product*/ None,
    );
    let fs = environment.get_filesystem();
    let unrestricted = codex_exec_server::FileSystemEnvironmentAccessor::unrestricted(&fs);
    let snapshot = service
        .snapshot_for_config_on_executor(&input, Some((&fs, &unrestricted)))
        .await;
    assert_eq!(snapshot.outcome().skills.len(), 1);
    assert_eq!(snapshot.outcome().errors, Vec::new());

    let mut sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                PathUri::from_host_native_path(&root)?.into(),
                FileSystemAccessMode::Read,
            )]),
            NetworkSandboxPolicy::Restricted,
        ),
        PathUri::from_abs_path(&cwd),
    );
    sandbox.windows_sandbox_selection = codex_exec_server::WindowsSandboxSelection::Mxc;
    let restricted = codex_exec_server::FileSystemEnvironmentAccessor::new(&fs, sandbox);
    let snapshot = service
        .snapshot_for_config_on_executor(&input, Some((&fs, &restricted)))
        .await;
    assert!(snapshot.outcome().skills.is_empty());
    assert_eq!(snapshot.outcome().errors, vec![codex_skills::SkillError {
        path: AbsolutePathBuf::from_absolute_path(root)?,
        message: "exec-server does not support the requested MXC Windows sandbox; update the executor".to_string(),
    }]);
    Ok(())
}

/// Roots supplied at startup or loaded from history work again when their environment is reselected.
#[test_case(false; "api roots")]
#[test_case(true; "saved roots")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capability_roots_survive_environment_reselection(
    from_history: bool,
) -> Result<()> {
    let server = start_mock_server().await;
    let observed_roots = Arc::new(Mutex::new(Vec::new()));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(Arc::new(ReadyCapabilityRootsTestExtension {
        observed_roots: Some(Arc::clone(&observed_roots)),
    }));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .expect("enable discovery");
        })
        .build_with_auto_env(&server)
        .await?;
    let selection = test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .context("selected environment")?;
    let roots = vec![SelectedCapabilityRoot {
        id: "selected-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    }];
    let mut thread_extension_init = ExtensionDataInit::new();
    let initial_history = if from_history {
        InitialHistory::Forked(vec![RolloutItem::SessionMeta(Box::new(SessionMetaLine {
            meta: SessionMeta {
                selected_capability_roots: roots.clone(),
                ..SessionMeta::default()
            },
            git: None,
        }))])
    } else {
        thread_extension_init.insert(roots.clone());
        InitialHistory::New
    };
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![selection.clone().into_request()]),
            initial_history,
            thread_extension_init,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("selected")]),
            sse(vec![ev_completed("removed")]),
            sse(vec![ev_completed("reselected")]),
        ],
    )
    .await;
    // Observe the startup selection before a settings update can rebuild it.
    for environment_requests in [None, Some(Vec::new()), Some(vec![selection.into_request()])] {
        thread
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "continue".to_string(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    environments: environment_requests.map(|requests| {
                        TurnEnvironmentRequests::new(test.config.cwd.clone(), requests)
                    }),
                    ..Default::default()
                }),
            )
            .await?;
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    }
    let mut actual_roots = observed_roots.lock().expect("observed roots").clone();
    // Prompt contributors can run more than once per turn.
    actual_roots.dedup();
    assert_eq!(actual_roots, vec![roots.clone(), Vec::new(), roots]);
    assert_eq!(response_mock.requests().len(), 3);
    Ok(())
}

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
