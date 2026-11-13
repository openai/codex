#![cfg(test)]
//! Exercises skill and plugin discovery before approval, after approval, and next turn.

use super::*;
use codex_mcp::McpServerKey;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use core_test_support::exec_server::ExecServerProcess;
use core_test_support::test_codex::TestEnv;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy)]
enum Discovery {
    ExecutorV1,
    DisabledExecutorV1Plugin,
    DisabledExecutorV1UnreadablePlugin,
    DisabledExecutorV1UnreadablePluginDirectory,
    DisabledExecutorV1UnreadableAncestorPlugin,
    DisabledExecutorV1UnreadableAncestorPluginDirectory,
    StandaloneExecutorV1WithDisabledId,
    ExecutorV2,
}

#[test_case(Discovery::ExecutorV1, PermissionGrantScope::Session; "executor v1 session grant")]
#[test_case(Discovery::ExecutorV1, PermissionGrantScope::Turn; "executor v1 turn grant")]
#[test_case(Discovery::ExecutorV2, PermissionGrantScope::Session; "executor v2 session grant")]
#[test_case(Discovery::ExecutorV2, PermissionGrantScope::Turn; "executor v2 turn grant")]
#[test_case(Discovery::DisabledExecutorV1Plugin, PermissionGrantScope::Turn; "turn grant respects plugin disablement")]
#[test_case(Discovery::DisabledExecutorV1UnreadablePlugin, PermissionGrantScope::Turn; "turn grant cannot bypass unreadable disabled plugin manifest")]
#[test_case(Discovery::DisabledExecutorV1UnreadablePluginDirectory, PermissionGrantScope::Turn; "turn grant cannot bypass unreadable disabled plugin directory")]
#[test_case(Discovery::DisabledExecutorV1UnreadableAncestorPlugin, PermissionGrantScope::Turn; "turn grant cannot bypass unreadable disabled ancestor plugin manifest")]
#[test_case(Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory, PermissionGrantScope::Turn; "turn grant cannot bypass unreadable disabled ancestor plugin directory")]
#[test_case(Discovery::StandaloneExecutorV1WithDisabledId, PermissionGrantScope::Turn; "explicit plugin deny preserves a standalone root")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_discovery_uses_current_environment_grants(
    discovery: Discovery,
    scope: PermissionGrantScope,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(
        Ok(()),
        "restricted reads require the elevated Windows sandbox backend, unavailable in this fixture"
    );
    let v2 = matches!(discovery, Discovery::ExecutorV2);
    let plugin_enabled = !matches!(
        discovery,
        Discovery::DisabledExecutorV1Plugin
            | Discovery::DisabledExecutorV1UnreadablePlugin
            | Discovery::DisabledExecutorV1UnreadablePluginDirectory
            | Discovery::DisabledExecutorV1UnreadableAncestorPlugin
            | Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory
    );
    let plugin_mcp_server = "approved_plugin_mcp";
    let environment = if v2 {
        None
    } else {
        Some(core_test_support::test_codex::test_env().await?)
    };
    let mut extensions = ExtensionRegistryBuilder::new();
    if let Some(environment) = &environment {
        #[cfg(target_os = "linux")]
        let sandbox_exe = Some(core_test_support::find_codex_linux_sandbox_exe()?);
        #[cfg(not(target_os = "linux"))]
        let sandbox_exe = None;
        let runtime_paths = codex_exec_server::ExecServerRuntimeOptions::new(
            std::env::current_exe()?,
            sandbox_exe,
        )?;
        codex_mcp_extension::install_plugins(
            &mut extensions,
            Arc::new(
                codex_exec_server::EnvironmentManager::create_for_tests(
                    environment.exec_server_url().map(str::to_owned),
                    Some(runtime_paths),
                )
                .await,
            ),
        );
    }
    let providers = SkillProviders::new().with_executor_provider(Arc::new(
        ExecutorSkillProvider::new_with_restriction_product(/*restriction_product*/ None),
    ));
    install_with_providers(&mut extensions, providers, |config: &Config| {
        SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            catalog_deltas_enabled: config.features.enabled(Feature::SkillCatalogDeltas),
            omit_descriptions: false,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        }
    });
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            configure_catalog_test(config);
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("permission tool");
            config
                .features
                .set_enabled(Feature::ExecutorCapabilityDiscoveryV2, v2)
                .expect("executor discovery version");
            config
                .features
                .set_enabled(Feature::Plugins, !v2)
                .expect("selected plugins in V1");
            config
                .features
                .enable(Feature::SkipHostSkillDiscovery)
                .expect("executor-only discovery");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::from_runtime_permissions(
                    &FileSystemSandboxPolicy::restricted(Vec::new()),
                    NetworkSandboxPolicy::Restricted,
                ))
                .expect("no initial filesystem access");
        });
    let server = responses::start_mock_server().await;
    // V2 requires a remote executor with the final protocol version.
    let executor;
    let (test, root) = if v2 {
        skip_if_remote!(Ok(()), "this case starts its own native exec-server");
        let home = tempfile::tempdir()?;
        let root = PathUri::from_host_native_path(home.path().canonicalize()?.join("skills"))?;
        // V2 scans installed roots; create this root before the executor prewarms its locations.
        std::fs::create_dir(home.path().join("skills"))?;
        executor = ExecServerProcess::start_with_version_and_home("0.164.0", home).await?;
        let environment =
            TestEnv::local_with_exec_server_url(Some(executor.websocket_url.clone())).await?;
        (
            builder.build_with_environment(&server, environment).await?,
            root,
        )
    } else {
        let test = builder
            .build_with_environment(&server, environment.expect("V1 test environment"))
            .await?;
        let root = test
            .executor_environment()
            .selection()
            .cwd
            .join(".agents/skills")?;
        (test, root)
    };
    let selection = test.executor_environment().selection().clone();
    let plugin_root = if matches!(
        discovery,
        Discovery::DisabledExecutorV1UnreadableAncestorPlugin
            | Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory
    ) {
        root.parent()
            .expect("plugin owns selected skill subdirectory")
    } else {
        root.clone()
    };
    let skill = root.join("approved-skill")?;
    test.fs()
        .create_directory(
            &skill,
            CreateDirectoryOptions {
                recursive: true,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    test.fs().write_file(
        &skill.join("SKILL.md")?,
        b"---\nname: approved-skill\ndescription: Approved repository instructions.\n---\n\nApproved skill body.".to_vec(),
        Default::default(),
        /*sandbox*/ None,
    ).await?;
    if !v2 && !matches!(discovery, Discovery::StandaloneExecutorV1WithDisabledId) {
        let directory = if matches!(
            discovery,
            Discovery::DisabledExecutorV1UnreadablePluginDirectory
                | Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory
        ) {
            plugin_root
                .parent()
                .expect("root parent")
                .join("denied-plugin")?
        } else {
            plugin_root.join(".codex-plugin")?
        };
        test.fs()
            .create_directory(
                &directory,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
        let manifest = plugin_root.join(".codex-plugin/plugin.json")?;
        let target = if matches!(
            discovery,
            Discovery::DisabledExecutorV1UnreadablePlugin
                | Discovery::DisabledExecutorV1UnreadableAncestorPlugin
        ) {
            plugin_root
                .parent()
                .expect("root parent")
                .join("denied-plugin.json")?
        } else {
            directory.join("plugin.json")?
        };
        test.fs()
            .write_file(
                &target,
                br#"{"name":"approved-plugin"}"#.to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
        let (target, link) = if matches!(
            discovery,
            Discovery::DisabledExecutorV1UnreadablePluginDirectory
                | Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory
        ) {
            (directory, plugin_root.join(".codex-plugin")?)
        } else {
            (target, manifest)
        };
        if target != link {
            let process = test
                .executor_environment()
                .environment()
                .get_exec_backend()
                .start(ExecParams {
                    process_id: ProcessId::from("link-denied-plugin-manifest"),
                    metadata: None,
                    argv: vec![
                        "/bin/ln".to_string(),
                        "-s".to_string(),
                        target.inferred_native_path_string(),
                        link.inferred_native_path_string(),
                    ],
                    cwd: selection.cwd.clone(),
                    env_policy: None,
                    shell_snapshot: None,
                    env: Default::default(),
                    tty: false,
                    pipe_stdin: false,
                    arg0: None,
                    sandbox: None,
                    enforce_managed_network: false,
                    managed_network: None,
                    network_proxy: None,
                })
                .await?
                .process;
            let mut events = process.subscribe_events();
            loop {
                match events.recv().await? {
                    ExecProcessEvent::Exited { exit_code, .. } => {
                        assert_eq!(exit_code, 0);
                        break;
                    }
                    ExecProcessEvent::Output(_) => {}
                    event @ (ExecProcessEvent::Closed { .. } | ExecProcessEvent::Failed(_)) => {
                        panic!("manifest symlink process did not exit: {event:?}")
                    }
                }
            }
        }
        let command = crate::suite::rmcp_client::remote_aware_stdio_server_bin()?;
        test.fs()
            .write_file(
                &root.join(".mcp.json")?,
                serde_json::to_vec(&json!({
                    "mcpServers": {(plugin_mcp_server): {"command": command}},
                }))?,
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
    }
    let package = format!(
        "skill://{}/{}",
        match discovery {
            Discovery::ExecutorV1
            | Discovery::DisabledExecutorV1Plugin
            | Discovery::DisabledExecutorV1UnreadablePlugin
            | Discovery::DisabledExecutorV1UnreadablePluginDirectory
            | Discovery::DisabledExecutorV1UnreadableAncestorPlugin
            | Discovery::DisabledExecutorV1UnreadableAncestorPluginDirectory
            | Discovery::StandaloneExecutorV1WithDisabledId => "approved-root",
            Discovery::ExecutorV2 => "codex-home-skills",
        },
        skill
            .inferred_native_path_string()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let read_skill = |call_id| {
        responses::ev_function_call_with_namespace(
            call_id,
            "skills",
            "read",
            &json!({"package": package}).to_string(),
        )
    };
    let mut thread_extension_init = ExtensionDataInit::new();
    if !v2 {
        thread_extension_init.insert(vec![SelectedCapabilityRoot {
            id: "approved-root".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: selection.environment_id.clone(),
                path: root.clone(),
            },
        }]);
    }
    let server_key = McpServerKey::in_environment(&selection.environment_id, plugin_mcp_server);
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![selection.into_request()]),
            disabled_plugin_ids: (!plugin_enabled
                || matches!(discovery, Discovery::StandaloneExecutorV1WithDisabledId))
            .then(|| vec!["approved-root".to_string()]),
            thread_extension_init,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    let plugin_is_published = async || -> Result<bool> {
        let (config, status) = thread
            .mcp_server_status_snapshot(
                server_key.name(),
                server_key.scoped_environment_id(),
                codex_mcp::McpSnapshotDetail::ToolsAndAuthOnly,
                /*expected_registration*/ None,
            )
            .await?;
        let published = config.mcp_server_catalog.server(&server_key).is_some();
        let has_echo = status
            .tools_by_server
            .get(&server_key)
            .is_some_and(|tools| tools.contains_key("echo"));
        assert_eq!(has_echo, published);
        Ok(published)
    };
    let permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_path_uris(
            Some(vec![
                test.fs()
                    .canonicalize(&plugin_root, /*sandbox*/ None)
                    .await?,
            ]),
            /*write*/ None,
        )),
        ..Default::default()
    };
    let mut sequence = vec![
        sse(vec![
            ev_response_created("before-approval"),
            read_skill("read-before-approval"),
            ev_completed("before-approval"),
        ]),
        sse(vec![
            ev_response_created("grant"),
            responses::ev_function_call(
                "grant-skills",
                "request_permissions",
                &json!({
                    "reason": "Read repository skills", "permissions": permissions,
                })
                .to_string(),
            ),
            ev_completed("grant"),
        ]),
    ];
    for (response_id, call_id) in [
        ("granted", "read-after-approval"),
        ("next-turn", "read-next-turn"),
    ] {
        let mut events = vec![ev_response_created(response_id), read_skill(call_id)];
        if response_id == "granted" && !v2 {
            events.push(responses::ev_function_call(
                "request-sibling",
                "request_permissions",
                &json!({
                    "reason": "Read the sibling file",
                    "permissions": {"file_system": FileSystemPermissions::from_read_write_path_uris(
                        Some(vec![root.parent().expect("skills parent").join("private")?]), /*write*/ None,
                    )},
                })
                .to_string(),
            ));
        }
        events.push(ev_completed(response_id));
        sequence.push(sse(events));
        sequence.push(sse(vec![ev_response_created("done"), ev_completed("done")]));
    }
    let responses = responses::mount_sse_sequence(&server, sequence).await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Read the selected skill, requesting permission if needed.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let event = wait_for_event(&thread, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissions(request) = event else {
        panic!("expected permission request: {event:?}");
    };
    assert_eq!(request.permissions, permissions);
    if !v2 {
        assert!(!plugin_is_published().await?);
    }
    thread
        .submit(Op::RequestPermissionsResponse {
            id: request.call_id,
            response: RequestPermissionsResponse {
                permissions,
                scope,
                strict_auto_review: false,
            },
        })
        .await?;
    if !v2 {
        // The second permission request keeps the turn open while we inspect the live grant.
        let event = wait_for_event(&thread, |event| {
            matches!(
                event,
                EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::RequestPermissions(request) = event else {
            panic!("expected the sibling permission request: {event:?}");
        };
        assert_eq!(request.call_id, "request-sibling");
        assert_eq!(
            plugin_is_published().await?,
            scope == PermissionGrantScope::Session
        );
        thread
            .submit(Op::RequestPermissionsResponse {
                id: request.call_id,
                response: RequestPermissionsResponse {
                    permissions: RequestPermissionProfile::default(),
                    scope: PermissionGrantScope::Turn,
                    strict_auto_review: false,
                },
            })
            .await?;
    }
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    if !v2 {
        assert_eq!(
            plugin_is_published().await?,
            scope == PermissionGrantScope::Session
        );
    }
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Read the selected skill again.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 6);
    let visible = [0, 2, 4]
        .into_iter()
        .map(|index| {
            requests[index]
                .message_input_texts("developer")
                .iter()
                .rev()
                .find(|text| text.contains(SKILLS_INSTRUCTIONS_OPEN_TAG))
                .is_some_and(|text| text.contains("approved-skill"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        vec![
            false,
            plugin_enabled,
            plugin_enabled && scope == PermissionGrantScope::Session
        ]
    );
    let readable = [
        (1, "read-before-approval"),
        (3, "read-after-approval"),
        (5, "read-next-turn"),
    ]
    .into_iter()
    .map(|(index, call)| {
        requests[index]
            .function_call_output_text(call)
            .expect("skill read result")
            .contains("Approved skill body.")
    })
    .collect::<Vec<_>>();
    assert_eq!(
        readable,
        vec![
            false,
            plugin_enabled,
            plugin_enabled && scope == PermissionGrantScope::Session
        ]
    );
    Ok(())
}
