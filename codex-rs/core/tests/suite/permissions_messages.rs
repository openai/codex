use anyhow::Result;
use codex_config::ConfigLayerStack;
use codex_core::ForkSnapshot;
use codex_core::config::Constrained;
use codex_core::context::ContextualUserFragment;
use codex_core::context::PermissionsInstructions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

fn permissions_texts(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.contains("<permissions instructions>"))
        .collect()
}

fn environment_context_texts(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.contains("<environment_context>"))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_sent_once_on_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(permissions_texts(&req.single_request()).len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_command_prefixes_are_sent_in_initial_environment_context_only() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        let rules_dir = config.codex_home.join("rules");
        fs::create_dir_all(&rules_dir).expect("create rules dir");
        fs::write(
            rules_dir.join("default.rules"),
            r#"prefix_rule(pattern=["git", "pull"], decision="allow")"#,
        )
        .expect("write policy");
    });
    let test = builder.build(&server).await?;
    let rollout_path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let first_environment_contexts = environment_context_texts(&req1.single_request());
    assert_eq!(first_environment_contexts.len(), 1);
    assert!(
        first_environment_contexts[0].contains("<approved_command_prefixes>"),
        "expected approved prefixes in environment context: {first_environment_contexts:?}"
    );
    assert!(first_environment_contexts[0].contains(r#"["git", "pull"]"#));

    let first_permissions = permissions_texts(&req1.single_request());
    assert_eq!(first_permissions.len(), 1);
    assert!(
        !first_permissions[0].contains("Approved command prefixes"),
        "did not expect approved prefixes in permissions message: {first_permissions:?}"
    );

    let second_environment_contexts = environment_context_texts(&req2.single_request());
    assert_eq!(second_environment_contexts, first_environment_contexts);

    let rollout_text = fs::read_to_string(rollout_path)?;
    for line in rollout_text.lines() {
        let item: Value = serde_json::from_str(line)?;
        if item.get("type").and_then(Value::as_str) == Some("turn_context") {
            let text = item.to_string();
            assert!(
                !text.contains("approved_command_prefixes"),
                "turn_context rollout item should not include approved prefixes: {text}"
            );
            assert!(
                !text.contains(r#"["git","pull"]"#),
                "turn_context rollout item should not include approved prefix list: {text}"
            );
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_command_prefixes_render_with_multiple_environments() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        let rules_dir = config.codex_home.join("rules");
        fs::create_dir_all(&rules_dir).expect("create rules dir");
        fs::write(
            rules_dir.join("default.rules"),
            r#"prefix_rule(pattern=["cargo", "test"], decision="allow")"#,
        )
        .expect("write policy");
    });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = AbsolutePathBuf::from_absolute_path(test.cwd.path().to_path_buf())
        .expect("test cwd is absolute");
    let remote_cwd = AbsolutePathBuf::from_absolute_path(test.cwd.path().join("remote"))
        .expect("remote cwd is absolute");

    test.submit_turn_with_environments(
        "hello multiple environments",
        Some(vec![
            TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: local_cwd.clone(),
            },
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: remote_cwd.clone(),
            },
        ]),
    )
    .await?;

    let environment_contexts = environment_context_texts(&req.single_request());
    assert_eq!(environment_contexts.len(), 1);
    let environment_context = &environment_contexts[0];
    assert!(environment_context.contains("<environments>"));
    assert!(environment_context.contains(&format!(r#"<environment id="{LOCAL_ENVIRONMENT_ID}">"#)));
    assert!(environment_context.contains(&format!("<cwd>{}</cwd>", local_cwd.to_string_lossy())));
    assert!(
        environment_context.contains(&format!(r#"<environment id="{REMOTE_ENVIRONMENT_ID}">"#))
    );
    assert!(environment_context.contains(&format!("<cwd>{}</cwd>", remote_cwd.to_string_lossy())));
    assert!(environment_context.contains("<approved_command_prefixes>"));
    assert!(environment_context.contains(r#"["cargo", "test"]"#));
    assert!(
        !environment_context.contains("\n  <cwd>"),
        "multiple environment context should not render a legacy top-level cwd: {environment_context}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_added_on_override_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions_1 = permissions_texts(&req1.single_request());
    let permissions_2 = permissions_texts(&req2.single_request());

    assert_eq!(permissions_1.len(), 1);
    assert_eq!(permissions_2.len(), 2);
    let unique = permissions_2.into_iter().collect::<HashSet<String>>();
    assert_eq!(unique.len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_not_added_when_no_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions_1 = permissions_texts(&req1.single_request());
    let permissions_2 = permissions_texts(&req2.single_request());

    assert_eq!(permissions_1.len(), 1);
    assert_eq!(permissions_2.len(), 1);
    assert_eq!(permissions_1, permissions_2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_omitted_when_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.include_permissions_instructions = false;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(
        permissions_texts(&req1.single_request()),
        Vec::<String>::new()
    );
    assert_eq!(
        permissions_texts(&req2.single_request()),
        Vec::<String>::new()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_replays_permissions_messages() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let _req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let req3 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let initial = builder.build(&server).await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let home = initial.home.clone();

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &initial.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            ..Default::default()
        },
    )
    .await?;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let resumed = builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions = permissions_texts(&req3.single_request());
    assert_eq!(permissions.len(), 3);
    let unique = permissions.into_iter().collect::<HashSet<String>>();
    assert_eq!(unique.len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_and_fork_append_permissions_messages() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let req3 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
    )
    .await;
    let req4 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-4"), ev_completed("resp-4")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let initial = builder.build(&server).await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let home = initial.home.clone();

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &initial.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            ..Default::default()
        },
    )
    .await?;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions_base = permissions_texts(&req2.single_request());
    assert_eq!(permissions_base.len(), 2);

    builder = builder.with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
    });
    let resumed = builder.resume(&server, home, rollout_path.clone()).await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions_resume = permissions_texts(&req3.single_request());
    assert_eq!(permissions_resume.len(), permissions_base.len() + 1);
    assert_eq!(
        &permissions_resume[..permissions_base.len()],
        permissions_base.as_slice()
    );
    assert!(!permissions_base.contains(permissions_resume.last().expect("new permissions")));

    let mut fork_config = initial.config.clone();
    fork_config.permissions.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
    let forked = initial
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            fork_config.clone(),
            rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await?;
    forked
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after fork".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&forked.thread, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions_fork = permissions_texts(&req4.single_request());
    assert_eq!(permissions_fork.len(), permissions_base.len() + 1);
    assert_eq!(
        &permissions_fork[..permissions_base.len()],
        permissions_base.as_slice()
    );
    let new_permissions = &permissions_fork[permissions_base.len()..];
    assert_eq!(new_permissions.len(), 1);
    assert_eq!(permissions_fork, permissions_resume);
    assert!(!permissions_base.contains(&new_permissions[0]));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_includes_writable_roots() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let writable = TempDir::new()?;
    let writable_root = AbsolutePathBuf::try_from(writable.path())?;
    let writable_root_for_config = writable_root.clone();
    let permission_profile = PermissionProfile::workspace_write_with(
        std::slice::from_ref(&writable_root),
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config
            .permissions
            .set_permission_profile(permission_profile)
            .expect("test permission profile should be allowed");
        let workspace_roots = vec![config.cwd.clone(), writable_root_for_config];
        config.workspace_roots = workspace_roots.clone();
        config.permissions.set_workspace_roots(workspace_roots);
        config.config_layer_stack = ConfigLayerStack::default();
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let permissions = permissions_texts(&req.single_request());
    let normalize_line_endings = |s: &str| s.replace("\r\n", "\n");
    let permission_profile = test.config.permissions.effective_permission_profile();
    let expected = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        AskForApproval::OnRequest,
        test.config.approvals_reviewer,
        test.config.cwd.as_path(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    )
    .render();
    let expected_normalized = normalize_line_endings(&expected);
    let actual_normalized: Vec<String> = permissions
        .iter()
        .map(|s| normalize_line_endings(s))
        .collect();
    assert_eq!(actual_normalized, vec![expected_normalized]);

    Ok(())
}
