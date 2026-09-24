//! Exercises retained terminal grants and strict stdin review through the agent API.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_protocol::approvals::ExecApprovalKind;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;

fn tool_response(id: &str, tool: &str, args: Value) -> String {
    sse(vec![
        ev_function_call(id, tool, &args.to_string()),
        ev_completed(id),
    ])
}

async fn harness(builder: TestCodexBuilder) -> Result<TestCodexHarness> {
    TestCodexHarness::with_auto_env_builder(builder.with_config(|config| {
        for feature in [
            Feature::ExecPermissionApprovals,
            Feature::RequestPermissionsTool,
        ] {
            config
                .features
                .enable(feature)
                .expect("enable stdin review");
        }
        config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())
            .expect("set read-only permissions");
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    }))
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdin_reviews_retained_grants_after_turn_permissions_expire() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses a POSIX interactive shell");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    let harness = harness(test_codex()).await?;
    let test = harness.test();
    harness.create_dir_all("allowed").await?;
    let directory = test
        .fs()
        .canonicalize(&test.workspace_path_uri("allowed")?, /*sandbox*/ None)
        .await?;
    let permissions = json!({"file_system": {"write": [directory.to_abs_path()?]}});
    let grant = mount_sse_once(
        harness.server(),
        tool_response(
            "grant",
            "request_permissions",
            json!({"reason":"allow terminal writes", "permissions":permissions}),
        ),
    )
    .await;
    let opened = mount_function_call_agent_response(
        harness.server(),
        "open",
        &json!({"cmd":"/bin/bash --noprofile --norc", "tty":true, "yield_time_ms":200}).to_string(),
        "exec_command",
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "open terminal".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestPermissions(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: request.call_id,
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(grant.requests().len(), 1);
    let output = opened
        .completion
        .single_request()
        .function_call_output("open");
    assert!(
        output
            .to_string()
            .contains("Process running with session ID 1000"),
        "{output}"
    );

    let writes = mount_sse_sequence(harness.server(), vec![
        tool_response("poll", "write_stdin", json!({"session_id":1000, "chars":"", "yield_time_ms":1000})),
        tool_response("denied", "write_stdin", json!({"session_id":1000, "chars":"REJECTED=1\n"})),
        tool_response("allowed", "write_stdin", json!({"session_id":1000, "chars":"test -z \"${REJECTED-}\" && printf allowed > allowed/result; exit\n"})),
        sse(vec![ev_completed("done")]),
    ]).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "send input".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    for (id, decision) in [
        ("denied", ReviewDecision::denied("blocked input")),
        ("allowed", ReviewDecision::Approved),
    ] {
        let request = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::ExecApprovalRequest(request) => Some(request.clone()),
            _ => None,
        })
        .await;
        assert_eq!(
            (
                request.kind,
                request.call_id.as_str(),
                request.effective_approval_id().as_str()
            ),
            (ExecApprovalKind::WriteStdin, "open", id)
        );
        assert_eq!(
            request.additional_permissions,
            Some(serde_json::from_value::<AdditionalPermissionProfile>(
                permissions.clone()
            )?)
        );
        test.codex
            .submit(Op::ExecApproval {
                id: id.into(),
                turn_id: Some(request.turn_id),
                decision,
            })
            .await?;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let output = writes
        .requests()
        .last()
        .unwrap()
        .function_call_output("allowed");
    assert!(
        output.to_string().contains("Process exited with code 0"),
        "{output}"
    );
    assert_eq!(harness.read_file_text("allowed/result").await?, "allowed");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_stdin_review_reaches_guardian_with_sandbox_prompts_disabled() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses a POSIX interactive shell");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    #[derive(Default)]
    struct ReviewedCalls(Mutex<Vec<String>>);

    impl codex_extension_api::ApprovalReviewContributor for ReviewedCalls {
        fn decide<'a>(
            &'a self,
            input: &'a codex_extension_api::ApprovalDecisionInput<'_>,
        ) -> codex_extension_api::ExtensionFuture<'a, Option<codex_extension_api::ApprovalDecision>>
        {
            self.0
                .lock()
                .unwrap()
                .push(input.tool_call_id.expect("reviewed call id").to_owned());
            Box::pin(async { None })
        }
    }

    let calls = Arc::new(ReviewedCalls::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.approval_review_contributor(calls.clone());
    let harness = harness(test_codex().with_extensions(Arc::new(extensions.build()))).await?;
    let test = harness.test();
    let opened = mount_sse_sequence(
        harness.server(),
        vec![
            tool_response(
                "open",
                "exec_command",
                json!({"cmd":"/bin/bash --noprofile --norc", "tty":true, "yield_time_ms":200}),
            ),
            tool_response(
                "baseline",
                "write_stdin",
                json!({"session_id":1000, "chars":"INITIAL=1\n"}),
            ),
            sse(vec![ev_completed("opened")]),
        ],
    )
    .await;
    harness
        .submit_with_permission_profile("open terminal", PermissionProfile::read_only())
        .await?;
    assert_eq!(opened.requests().len(), 3);
    let decision = |outcome: &str| {
        sse(vec![
        ev_assistant_message(outcome, &json!({"risk_level":"low", "user_authorization":"high", "outcome":outcome, "rationale":"stdin test"}).to_string()),
        ev_completed(outcome),
    ])
    };
    let writes = mount_sse_sequence(harness.server(), vec![
        tool_response("strict", "request_permissions", json!({"reason":"review later input", "permissions":{"network":{"enabled":true}}})),
        tool_response("poll", "write_stdin", json!({"session_id":1000, "chars":"", "yield_time_ms":1000})),
        tool_response("denied", "write_stdin", json!({"session_id":1000, "chars":"REJECTED=1\n"})),
        decision("deny"),
        tool_response("allowed", "write_stdin", json!({"session_id":1000, "chars":"test \"${INITIAL-}\" = 1 && test -z \"${REJECTED-}\" && printf 'STDIN_%s_OK\\n' REVIEW; exit\n"})),
        decision("allow"),
        sse(vec![ev_completed("done")]),
    ]).await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "send input".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval: false,
                    rules: false,
                    skill_approval: true,
                    request_permissions: true,
                    mcp_elicitations: false,
                })),
                ..Default::default()
            }),
        )
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestPermissions(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: request.call_id,
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = writes.requests();
    assert_eq!(
        *calls.0.lock().unwrap(),
        vec!["strict", "denied", "allowed"]
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.body_contains_text("Planned action JSON:"))
            .count(),
        2
    );
    let output = requests.last().unwrap().function_call_output("allowed");
    assert!(output.to_string().contains("STDIN_REVIEW_OK"), "{output}");
    Ok(())
}
