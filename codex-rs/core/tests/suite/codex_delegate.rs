use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_ignores_legacy_deltas() {
    skip_if_no_network!();

    // Single response with reasoning summary deltas.
    let sse_stream = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item_added("reason-1", &["initial"]),
        ev_reasoning_summary_text_delta("think-1"),
        ev_completed("resp-1"),
    ]);

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![sse_stream]).await;

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("build test codex");

    // Kick off review (delegated).
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let mut reasoning_delta_count = 0;

    loop {
        let ev = wait_for_event(&test.codex, |_| true).await;
        match ev {
            EventMsg::ReasoningContentDelta(_) => reasoning_delta_count += 1,
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(reasoning_delta_count, 1, "expected one new reasoning delta");
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_rejects_escalation_requests_when_parent_can_prompt() {
    skip_if_no_network!();

    let call_id = "review-escalation-call";
    let command = serde_json::json!({
        "cmd": "echo review",
        "sandbox_permissions": "require_escalated"
    });
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &command.to_string()),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        })
        .build_with_auto_env(&server)
        .await
        .expect("build review delegate with escalation support");

    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review without requesting approval".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "review delegate should reject escalation requests without prompting: {event:?}"
    );

    let requests = response_mock.requests();
    let [_, completion_request] = requests.as_slice() else {
        panic!("expected the model request and escalation-denial continuation");
    };
    let output = completion_request.function_call_output(call_id);
    let response = output["output"]
        .as_str()
        .expect("command tool output should be a string");
    assert!(
        response.contains("approval policy is Never"),
        "escalation should be rejected by the delegate's approval policy: {response}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_rejects_legacy_mcp_approvals_without_prompting() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server)
        .await
        .expect("mount mock app server");
    let call_id = "review-calendar-call";
    let arguments = serde_json::json!({
        "title": "Review meeting",
        "starts_at": "2026-08-12T12:00:00Z"
    });
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &arguments.to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let test = search_capable_apps_builder(apps_server.chatgpt_base_url)
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("review delegate should inherit a restricted permission profile");
            let config_path = config.codex_home.join("config.toml").abs();
            let app_config = toml::from_str(
                r#"
[apps.calendar]
default_tools_approval_mode = "prompt"
"#,
            )
            .expect("app approval configuration should parse");
            config.config_layer_stack = config
                .config_layer_stack
                .with_user_config(&config_path, app_config)
                .expect("app approval configuration should be valid");
            config
                .features
                .disable(Feature::ToolCallMcpElicitation)
                .expect("legacy MCP approvals should be available");
        })
        .build_with_auto_env(&server)
        .await
        .expect("build review delegate with legacy MCP approvals");

    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review the [$calendar](app://calendar) integration".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestUserInput(_)
                | EventMsg::ElicitationRequest(_)
                | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "review delegate should reject legacy MCP approvals without prompting: {event:?}"
    );

    let requests = response_mock.requests();
    let [_, completion_request] = requests.as_slice() else {
        panic!("expected the model request and MCP-denial continuation");
    };
    let output = completion_request.function_call_output(call_id);
    let response = output["output"]
        .as_str()
        .expect("MCP tool output should be a string");
    assert!(
        response.contains("approval policy is never"),
        "MCP tool should be rejected by the delegate's approval policy: {response}"
    );
    assert!(
        recorded_apps_tool_calls(&server).await.is_empty(),
        "MCP tool requiring approval should never execute"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_delegate_rejects_skill_mcp_dependency_installation_without_prompting() {
    skip_if_wine_exec!("skill paths require matching host and executor path conventions");
    skip_if_no_network!();

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("review delegate should inherit a restricted permission profile");
            config
                .features
                .enable(Feature::SkillMcpDependencyInstall)
                .expect("skill MCP dependency installation should be available");
        })
        .with_workspace_setup(|cwd, fs| async move {
            let skill_dir = cwd.join(".agents/skills/dependency-skill");
            let agents_dir = skill_dir.join("agents");
            fs.create_directory(
                &PathUri::from_host_native_path(&agents_dir)?,
                CreateDirectoryOptions { recursive: true, follow_symlinks: true },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                b"---\nname: dependency-skill\ndescription: Requires an MCP server.\n---\n\nReview dependency instructions.\n"
                    .to_vec(),
                Default::default(), /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(agents_dir.join("openai.yaml"))?,
                b"dependencies:\n  tools:\n    - type: mcp\n      value: missing-review-server\n      transport: streamable_http\n      url: http://127.0.0.1:1/mcp\n"
                    .to_vec(),
                Default::default(), /*sandbox*/ None,
            )
            .await?;
            Ok(())
        })
        .build_with_auto_env(&server)
        .await
        .expect("build review delegate with a missing skill MCP dependency");

    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review with $dependency-skill".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestUserInput(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "review delegate should reject skill MCP dependency installation without prompting: {event:?}"
    );

    let request = response_mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>dependency-skill</name>")
                && text.contains("Review dependency instructions.")
        }),
        "review should continue with the selected skill after rejecting its MCP dependency: {user_texts:?}"
    );
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_delegate_rejects_escalation_requests_without_prompting() {
    skip_if_wine_exec!("Guardian approval actions require host-native paths");
    skip_if_no_network!();

    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("guardian delegate should inherit a restricted permission profile");
        })
        .build_with_auto_env(&server)
        .await
        .expect("build guardian delegate with escalation support");

    let parent_call_id = "parent-escalation-call";
    let guardian_call_id = "guardian-escalation-call";
    let guardian_output_file = test.cwd.path().join("guardian-escalation-marker.txt");
    let parent_command = serde_json::json!({
        "cmd": "echo parent command",
        "sandbox_permissions": "require_escalated",
        "justification": "Trigger Guardian approval review."
    });
    let guardian_command = serde_json::json!({
        "cmd": format!("echo guardian-ran > \"{}\"", guardian_output_file.display()),
        "sandbox_permissions": "require_escalated",
        "justification": "Guardian must not escalate its own commands."
    });
    let assessment = serde_json::json!({
        "risk_level": "high",
        "user_authorization": "low",
        "outcome": "deny",
        "rationale": "Guardian could not execute an escalated command."
    });
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-command"),
                ev_function_call(parent_call_id, "exec_command", &parent_command.to_string()),
                ev_completed("resp-parent-command"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-command"),
                ev_function_call(
                    guardian_call_id,
                    "exec_command",
                    &guardian_command.to_string(),
                ),
                ev_completed("resp-guardian-command"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-assessment"),
                ev_assistant_message("msg-guardian-assessment", &assessment.to_string()),
                ev_completed("resp-guardian-assessment"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-denied"),
                ev_assistant_message("msg-parent-denied", "denied"),
                ev_completed("resp-parent-denied"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Trigger Guardian review of an escalated command".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                ..Default::default()
            }),
        )
        .await
        .expect("submit guardian-reviewed command");

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "guardian delegate should reject escalation requests without prompting: {event:?}"
    );

    let requests = response_mock.requests();
    let guardian_requests = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 2);
    let guardian_output = guardian_requests
        .iter()
        .find_map(|request| request.function_call_output_text(guardian_call_id))
        .expect("guardian continuation should include the rejected command output");
    assert!(
        guardian_output.contains("approval policy is Never"),
        "guardian escalation should be rejected by its never approval policy: {guardian_output}"
    );
    assert!(
        !guardian_output_file.exists(),
        "guardian command requiring approval should never execute"
    );

    let parent_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text(parent_call_id))
        .expect("parent continuation should include the guardian denial");
    assert!(
        parent_output.contains("Guardian could not execute an escalated command."),
        "guardian denial rationale should reach the parent: {parent_output}"
    );
}
