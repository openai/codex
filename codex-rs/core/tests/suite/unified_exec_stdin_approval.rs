//! Exercises retained terminal grants and strict stdin review through the agent API.

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_protocol::approvals::ExecApprovalKind;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::create_directory_symlink;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
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
async fn stdin_reviews_escalated_terminal_with_denied_reads_before_delivering_input() -> Result<()>
{
    skip_if_target_windows!(Ok(()), "uses a POSIX interactive shell");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    let harness = TestCodexHarness::with_auto_env_builder(test_codex().with_config(|config| {
        config.features.enable(Feature::WriteStdinApproval).unwrap();
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .unwrap();
        config.features.enable(Feature::UnifiedExec).unwrap();
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        let mut file_system = PermissionProfile::read_only().file_system_sandbox_policy();
        file_system.entries.push(FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: "**/secret.env".into(),
            },
            FileSystemAccessMode::Deny,
        ));
        config
            .permissions
            .set_permission_profile(PermissionProfile::from_runtime_permissions(
                &file_system,
                NetworkSandboxPolicy::Restricted,
            ))
            .unwrap();
    }))
    .await?;
    let codex = &harness.test().codex;
    harness.write_file("secret.env", "secret").await?;
    let _open = mount_function_call_agent_response(
        harness.server(),
        "open",
        &json!({"cmd":"/bin/bash --noprofile --norc", "tty":true, "yield_time_ms":200, "sandbox_permissions":"require_escalated", "justification":"open an escalated terminal"}).to_string(),
        "exec_command",
    ).await;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "open terminal".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let launch = wait_for_event_match(codex, |event| match event {
        EventMsg::ExecApprovalRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(launch.kind, ExecApprovalKind::Command);
    codex
        .submit(Op::ExecApproval {
            id: launch.effective_approval_id(),
            turn_id: Some(launch.turn_id),
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let _writes = mount_sse_sequence(harness.server(), vec![
        tool_response("denied", "write_stdin", json!({"session_id":1000,"chars":"printf denied > denied-marker\n"})),
        tool_response("allowed", "write_stdin", json!({"session_id":1000,"chars":"if test -e denied-marker || cat secret.env; then exit 12; fi; printf approved > approved-marker; exit\n"})),
        sse(vec![ev_completed("done")]),
    ]).await;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "send input".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    for (id, decision) in [
        ("denied", ReviewDecision::denied("blocked input")),
        ("allowed", ReviewDecision::Approved),
    ] {
        let request = wait_for_event_match(codex, |event| match event {
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
        assert!(!harness.path_exists("denied-marker").await?);
        assert!(!harness.path_exists("approved-marker").await?);
        codex
            .submit(Op::ExecApproval {
                id: request.effective_approval_id(),
                turn_id: Some(request.turn_id),
                decision,
            })
            .await?;
    }
    let mut process_end = None;
    let mut turn_completed = false;
    while process_end.is_none() || !turn_completed {
        match wait_for_event(codex, |_| true).await {
            EventMsg::ExecCommandEnd(event) if event.call_id == "open" => process_end = Some(event),
            EventMsg::TurnComplete(_) => turn_completed = true,
            EventMsg::ExecApprovalRequest(request) => {
                anyhow::bail!("unexpected additional stdin approval: {request:?}")
            }
            _ => {}
        }
    }
    let process_end = process_end.expect("retained terminal should finish");
    assert_eq!(process_end.exit_code, 0);
    assert!(!harness.path_exists("denied-marker").await?);
    assert_eq!(harness.read_file_text("approved-marker").await?, "approved");
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
    let original_command = format!("/bin/bash --noprofile --norc # {}", "x".repeat(8_000));
    let opened = mount_sse_sequence(
        harness.server(),
        vec![
            tool_response(
                "open",
                "exec_command",
                json!({"cmd":original_command, "tty":true, "yield_time_ms":200}),
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
    let original_commands = requests
        .iter()
        .filter(|request| request.body_contains_text("Planned action JSON:"))
        .map(|request| {
            let text = request.message_input_texts("user").join("\n");
            let (_, action) = text
                .rsplit_once("Planned action JSON:\n")
                .context("planned stdin action")?;
            let (action, _) = action
                .split_once("\n>>> APPROVAL REQUEST END")
                .context("end of planned stdin action")?;
            Ok(serde_json::from_str::<Value>(action)?["original_command"].clone())
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(original_commands, vec![json!(original_command); 2]);
    let output = requests.last().unwrap().function_call_output("allowed");
    assert!(output.to_string().contains("STDIN_REVIEW_OK"), "{output}");
    Ok(())
}

#[derive(Clone, Copy)]
enum StdinReviewer {
    User,
    Guardian,
}

#[derive(Clone, Copy)]
enum GrantChange {
    Unchanged,
    ReadOnlyChildExpires,
    MetadataWriteExpires,
    SymlinkWriteExpires,
    UnchangedOrderedDenies,
}

#[derive(Clone, Copy)]
enum LaterApprovalPolicy {
    Inherit,
    Never,
}

/// A grant active at launch permits later input to reach review without enabling
/// sandbox escalation, including grants with interleaved denials. Expired authority
/// beneath parent grants and Never policy must block input without delivery.
#[test_case::test_case(PermissionGrantScope::Turn, StdinReviewer::User, GrantChange::Unchanged, LaterApprovalPolicy::Inherit; "turn_grant_user_review")]
#[test_case::test_case(PermissionGrantScope::Turn, StdinReviewer::Guardian, GrantChange::Unchanged, LaterApprovalPolicy::Inherit; "turn_grant_strict_review")]
#[test_case::test_case(PermissionGrantScope::Session, StdinReviewer::User, GrantChange::Unchanged, LaterApprovalPolicy::Never; "session_grant_never_policy")]
#[test_case::test_case(PermissionGrantScope::Session, StdinReviewer::User, GrantChange::ReadOnlyChildExpires, LaterApprovalPolicy::Inherit; "read_only_child_grant_expiry")]
#[test_case::test_case(PermissionGrantScope::Session, StdinReviewer::User, GrantChange::MetadataWriteExpires, LaterApprovalPolicy::Inherit; "explicit_metadata_grant_expiry")]
#[test_case::test_case(PermissionGrantScope::Session, StdinReviewer::User, GrantChange::SymlinkWriteExpires, LaterApprovalPolicy::Inherit; "symlink_grant_expiry")]
#[test_case::test_case(PermissionGrantScope::Session, StdinReviewer::User, GrantChange::UnchangedOrderedDenies, LaterApprovalPolicy::Inherit; "unchanged_interleaved_denials")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdin_reviews_current_grants_with_sandbox_prompts_disabled(
    scope: PermissionGrantScope,
    reviewer: StdinReviewer,
    grant_change: GrantChange,
    later_policy: LaterApprovalPolicy,
) -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses a POSIX interactive shell");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    if matches!(grant_change, GrantChange::SymlinkWriteExpires) {
        skip_if_remote!(Ok(()), "creates a symlink on the local executor filesystem");
        if !cfg!(target_os = "linux") {
            eprintln!("Skipping Linux symlink grant regression on another host OS");
            return Ok(());
        }
    }
    let harness = harness(test_codex()).await?;
    let test = harness.test();
    harness.create_dir_all("allowed").await?;
    let allowed = test.workspace_path_uri("allowed")?.to_abs_path()?;
    let mut permission_profile = PermissionProfile::read_only();
    let mut grants = vec![(json!({"network":{"enabled":true}}), scope)];
    match grant_change {
        GrantChange::ReadOnlyChildExpires
        | GrantChange::MetadataWriteExpires
        | GrantChange::SymlinkWriteExpires => {
            let child = if matches!(grant_change, GrantChange::MetadataWriteExpires) {
                "allowed/.git"
            } else if matches!(grant_change, GrantChange::SymlinkWriteExpires) {
                "allowed/link"
            } else {
                "allowed/child"
            };
            if matches!(grant_change, GrantChange::SymlinkWriteExpires) {
                harness.create_dir_all("outside").await?;
                create_directory_symlink(&harness.path("outside"), &harness.path(child));
            } else {
                harness.create_dir_all(child).await?;
            }
            let child = test.workspace_path_uri(child)?.to_abs_path()?;
            if matches!(grant_change, GrantChange::ReadOnlyChildExpires) {
                let mut file_system = permission_profile.file_system_sandbox_policy();
                file_system.entries.push(FileSystemSandboxEntry::new(
                    child.clone().into(),
                    FileSystemAccessMode::Read,
                ));
                permission_profile = PermissionProfile::from_runtime_permissions(
                    &file_system,
                    NetworkSandboxPolicy::Restricted,
                );
            }
            grants = vec![
                (json!({"file_system":{"write":[allowed]}}), scope),
                (
                    json!({"file_system":{"write":[child]}}),
                    PermissionGrantScope::Turn,
                ),
            ];
        }
        GrantChange::UnchangedOrderedDenies => {
            harness
                .write_file("allowed/a/private/secret", "secret")
                .await?;
            harness.create_dir_all("allowed/b").await?;
            let write_a =
                FileSystemSandboxEntry::new(allowed.join("a").into(), FileSystemAccessMode::Write);
            let deny_a = FileSystemSandboxEntry::new(
                allowed.join("a/private").into(),
                FileSystemAccessMode::Deny,
            );
            let write_b =
                FileSystemSandboxEntry::new(allowed.join("b").into(), FileSystemAccessMode::Write);
            let retained = FileSystemPermissions {
                entries: vec![write_a, deny_a],
                glob_scan_max_depth: None,
            };
            let extra = FileSystemPermissions {
                entries: vec![write_b],
                glob_scan_max_depth: None,
            };
            grants = vec![
                (json!({"file_system":retained}), scope),
                (json!({"file_system":extra}), scope),
            ];
        }
        GrantChange::Unchanged => {}
    }
    let turn_grant_expires = matches!(
        grant_change,
        GrantChange::ReadOnlyChildExpires
            | GrantChange::MetadataWriteExpires
            | GrantChange::SymlinkWriteExpires
    );
    let decision = |outcome: &str| {
        sse(vec![
            ev_assistant_message(outcome, &json!({"risk_level":"low", "user_authorization":"high", "outcome":outcome, "rationale":"stdin grant test"}).to_string()),
            ev_completed(outcome),
        ])
    };
    let mut responses = Vec::new();
    for (id, (permissions, _)) in ["grant", "extra_grant"].into_iter().zip(&grants) {
        responses.push(tool_response(
            id,
            "request_permissions",
            json!({"reason":"allow terminal access", "permissions":permissions}),
        ));
    }
    responses.push(tool_response(
        "open",
        "exec_command",
        json!({"cmd":"/bin/bash --noprofile --norc", "tty":true, "yield_time_ms":200}),
    ));
    if matches!(reviewer, StdinReviewer::Guardian) {
        responses.push(decision("allow"));
    }
    let grant_probe = match grant_change {
        GrantChange::ReadOnlyChildExpires => "printf launch > allowed/child/launched && ",
        GrantChange::MetadataWriteExpires => "printf launch > allowed/.git/launched && ",
        GrantChange::SymlinkWriteExpires => "printf launch > allowed/link/launched && ",
        GrantChange::UnchangedOrderedDenies => {
            "printf launch > allowed/a/launched && printf launch > allowed/b/launched && ! cat allowed/a/private/secret && "
        }
        GrantChange::Unchanged => "",
    };
    responses.push(tool_response("allowed", "write_stdin", json!({"session_id":1000, "chars":format!("{grant_probe}printf 'CURRENT_%s_OK\\n' GRANT\n")})));
    if matches!(reviewer, StdinReviewer::Guardian) {
        responses.push(decision("allow"));
    }
    responses.push(sse(vec![ev_completed("done")]));
    let writes = mount_sse_sequence(harness.server(), responses).await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "grant permission, open a terminal, and send reviewed input".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                permission_profile: Some(permission_profile),
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
    let mut grant_scopes = grants.into_iter().map(|(_, scope)| scope);
    let mut approvals = 0;
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::RequestPermissions(request) => {
                test.codex
                    .submit(Op::RequestPermissionsResponse {
                        id: request.call_id,
                        response: RequestPermissionsResponse {
                            permissions: request.permissions,
                            scope: grant_scopes.next().expect("requested grant"),
                            strict_auto_review: matches!(reviewer, StdinReviewer::Guardian),
                        },
                    })
                    .await?;
            }
            EventMsg::ExecApprovalRequest(request) => {
                assert!(matches!(reviewer, StdinReviewer::User));
                assert_eq!(
                    (request.kind, request.effective_approval_id()),
                    (ExecApprovalKind::WriteStdin, "allowed".to_string())
                );
                approvals += 1;
                test.codex
                    .submit(Op::ExecApproval {
                        id: request.effective_approval_id(),
                        turn_id: Some(request.turn_id),
                        decision: ReviewDecision::Approved,
                    })
                    .await?;
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(grant_scopes.next(), None);
    assert_eq!(
        approvals,
        usize::from(matches!(reviewer, StdinReviewer::User))
    );
    let output = writes
        .function_call_output_text("allowed")
        .expect("stdin output");
    assert!(output.contains("CURRENT_GRANT_OK"), "{output}");
    if matches!(reviewer, StdinReviewer::Guardian) {
        assert_eq!(
            writes
                .requests()
                .iter()
                .filter(|request| request.body_contains_text("Planned action JSON:"))
                .count(),
            2
        );
        return Ok(());
    }

    let later = mount_function_call_agent_response(
        harness.server(),
        "later",
        &json!({"session_id":1000, "chars":"LATER=1; printf 'LATER_%s_OK\\n' GRANT\n"}).to_string(),
        "write_stdin",
    )
    .await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "send input in a later turn".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: match later_policy {
                    LaterApprovalPolicy::Inherit => None,
                    LaterApprovalPolicy::Never => Some(AskForApproval::Never),
                },
                ..Default::default()
            }),
        )
        .await?;
    let later_input_allowed = matches!(scope, PermissionGrantScope::Session)
        && !turn_grant_expires
        && matches!(later_policy, LaterApprovalPolicy::Inherit);
    let mut later_approvals = 0;
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::ExecApprovalRequest(request) => {
                assert!(later_input_allowed, "unexpected stdin review: {request:?}");
                assert_eq!(
                    (request.kind, request.effective_approval_id()),
                    (ExecApprovalKind::WriteStdin, "later".to_string())
                );
                later_approvals += 1;
                test.codex
                    .submit(Op::ExecApproval {
                        id: request.effective_approval_id(),
                        turn_id: Some(request.turn_id),
                        decision: ReviewDecision::Approved,
                    })
                    .await?;
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(later_approvals, usize::from(later_input_allowed));
    let output = later
        .completion
        .function_call_output_text("later")
        .expect("later stdin output");
    if matches!(later_policy, LaterApprovalPolicy::Never) {
        assert!(
            output.contains("AskForApproval is set to Never"),
            "{output}"
        );
    } else if later_input_allowed {
        assert!(output.contains("LATER_GRANT_OK"), "{output}");
    } else {
        assert!(
            output.contains("AskForApproval::Granular.sandbox_approval is false"),
            "{output}"
        );
    }
    if !later_input_allowed {
        let probe = mount_function_call_agent_response(
            harness.server(),
            "probe",
            &json!({"session_id":1000, "chars":"test -z \"${LATER-}\" && printf 'REJECTED_%s_OK\\n' INPUT; exit\n"}).to_string(),
            "write_stdin",
        )
        .await;
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "verify rejected input was not delivered".into(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::OnRequest),
                    ..Default::default()
                }),
            )
            .await?;
        loop {
            match wait_for_event(&test.codex, |_| true).await {
                EventMsg::ExecApprovalRequest(request) => {
                    assert_eq!(
                        (request.kind, request.effective_approval_id()),
                        (ExecApprovalKind::WriteStdin, "probe".to_string())
                    );
                    test.codex
                        .submit(Op::ExecApproval {
                            id: request.effective_approval_id(),
                            turn_id: Some(request.turn_id),
                            decision: ReviewDecision::Approved,
                        })
                        .await?;
                }
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
        let output = probe
            .completion
            .function_call_output_text("probe")
            .expect("probe output");
        assert!(output.contains("REJECTED_INPUT_OK"), "{output}");
    }
    Ok(())
}
