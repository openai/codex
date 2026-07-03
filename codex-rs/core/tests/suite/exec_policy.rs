#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_features::Feature;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
#[cfg(windows)]
use codex_protocol::protocol::ExecApprovalPurpose;
use codex_protocol::protocol::Op;
#[cfg(windows)]
use codex_protocol::protocol::ReviewDecision;
#[cfg(windows)]
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
use std::fs;
#[cfg(windows)]
use std::path::PathBuf;

fn collaboration_mode_for_model(model: String) -> CollaborationMode {
    CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model,
            reasoning_effort: None,
            developer_instructions: Some("exercise approvals in collaboration mode".to_string()),
        },
    }
}

async fn submit_user_turn(
    test: &core_test_support::test_codex::TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    collaboration_mode: Option<CollaborationMode>,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: collaboration_mode.or({
                    Some(codex_protocol::config_types::CollaborationMode {
                        mode: codex_protocol::config_types::ModeKind::Default,
                        settings: codex_protocol::config_types::Settings {
                            model: session_model,
                            reasoning_effort: None,
                            developer_instructions: None,
                        },
                    })
                }),
                ..Default::default()
            },
        })
        .await?;
    Ok(())
}

fn assert_no_matched_rules_invariant(output_item: &Value) {
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function call output should include a string output payload");
    assert!(
        !output.contains("invariant failed: matched_rules must be non-empty"),
        "unexpected invariant panic surfaced in output: {output}"
    );
}

#[cfg(windows)]
fn installed_windows_powershell() -> PathBuf {
    codex_shell_command::powershell::try_find_powershell_executable_blocking()
        .expect("Windows PowerShell must be installed")
        .into_path_buf()
}

#[cfg(windows)]
fn enable_unified_exec(config: &mut codex_core::config::Config) {
    config
        .features
        .enable(Feature::UnifiedExec)
        .expect("test config should allow feature update");
}

#[cfg(windows)]
#[tokio::test]
async fn unified_exec_workspace_powershell_path_requires_one_shot_approval_before_execution()
-> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(enable_unified_exec);
    let test = builder.build(&server).await?;

    let fake_powershell = test.config.cwd.join("powershell.exe");
    fs::copy(installed_windows_powershell(), &fake_powershell)
        .expect("copy Windows PowerShell into the test workspace");
    let fake_powershell = fake_powershell
        .to_str()
        .expect("the test workspace path must be valid UTF-8")
        .to_string();

    let sentinel = test.config.cwd.join("workspace-powershell-started.txt");
    let marker = "WORKSPACE_POWERSHELL_STARTED";
    let sentinel_arg = sentinel
        .to_str()
        .expect("the sentinel path must be valid UTF-8")
        .replace('\'', "''");
    let script =
        format!("Set-Content -LiteralPath '{sentinel_arg}' started; Write-Output '{marker}'");
    let expected_command = vec![
        fake_powershell.clone(),
        "-Command".to_string(),
        script.clone(),
    ];
    let call_id = "unified-exec-workspace-powershell-one-shot";
    let args = json!({
        "shell": fake_powershell.clone(),
        "cmd": script,
        "yield_time_ms": 1_000,
    });
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-workspace-powershell-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-workspace-powershell-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-workspace-powershell-1", "done"),
            ev_completed("resp-workspace-powershell-2"),
        ]),
    )
    .await;

    submit_user_turn(
        &test,
        "run a read-only command with a workspace PowerShell copy",
        AskForApproval::UnlessTrusted,
        PermissionProfile::Disabled,
        /*collaboration_mode*/ None,
    )
    .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_)
                | EventMsg::ExecCommandBegin(_)
                | EventMsg::ExecCommandEnd(_)
                | EventMsg::TurnAborted(_)
                | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        !sentinel.exists(),
        "the requested workspace PowerShell must not start before approval"
    );
    let approval = match event {
        EventMsg::ExecApprovalRequest(approval) => approval,
        EventMsg::ExecCommandBegin(begin) => {
            panic!("workspace PowerShell began before approval: {begin:?}")
        }
        EventMsg::ExecCommandEnd(end) => {
            panic!("workspace PowerShell completed before approval: {end:?}")
        }
        EventMsg::TurnAborted(aborted) => {
            panic!("turn aborted before one-shot approval: {aborted:?}")
        }
        EventMsg::TurnComplete(_) => panic!("expected one-shot approval before completion"),
        _ => unreachable!(),
    };
    assert_eq!(approval.call_id, call_id);
    assert_eq!(approval.command, expected_command);
    assert_eq!(approval.cwd, test.config.cwd);
    let approval_id = approval
        .approval_id
        .clone()
        .expect("one-shot approval must carry a callback ID");
    assert_ne!(approval_id, call_id);
    assert_eq!(
        approval.approval_purpose,
        Some(ExecApprovalPurpose::Initial)
    );
    assert_eq!(
        approval.effective_approval_purpose(),
        ExecApprovalPurpose::Initial
    );
    assert_eq!(
        approval.effective_available_decisions(),
        vec![ReviewDecision::Approved, ReviewDecision::Abort]
    );
    assert_eq!(approval.proposed_execpolicy_amendment, None);
    assert!(!sentinel.exists());

    let approval_turn_id = approval.turn_id.clone();
    test.codex
        .submit(Op::ExecApproval {
            id: approval_id,
            turn_id: Some(approval_turn_id.clone()),
            decision: ReviewDecision::Abort,
        })
        .await?;

    let aborted = match wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecCommandBegin(_)
                | EventMsg::ExecCommandEnd(_)
                | EventMsg::TurnAborted(_)
                | EventMsg::TurnComplete(_)
        )
    })
    .await
    {
        EventMsg::TurnAborted(aborted) => aborted,
        EventMsg::ExecCommandBegin(begin) => {
            panic!("workspace PowerShell began after abort: {begin:?}")
        }
        EventMsg::ExecCommandEnd(end) => {
            panic!("workspace PowerShell completed after abort: {end:?}")
        }
        EventMsg::TurnComplete(_) => panic!("aborted approval unexpectedly completed the turn"),
        _ => unreachable!(),
    };
    assert_eq!(aborted.turn_id.as_deref(), Some(approval_turn_id.as_str()));
    assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    assert!(
        !sentinel.exists(),
        "aborting the one-shot approval must never start the requested workspace PowerShell"
    );
    assert!(
        results_mock.requests().is_empty(),
        "aborting the one-shot approval must not continue the turn"
    );

    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn unified_exec_configured_authoritative_windows_powershell_runs_without_approval()
-> Result<()> {
    let server = start_mock_server().await;
    let powershell = installed_windows_powershell();
    let configured_shell = codex_core::shell::get_shell_by_model_provided_path(&powershell);
    let mut builder = test_codex()
        .with_config(enable_unified_exec)
        .with_user_shell(configured_shell);
    let test = builder.build(&server).await?;

    let call_id = "unified-exec-authoritative-windows-powershell";
    let args = json!({
        "cmd": "Get-Location",
        "yield_time_ms": 1_000,
    });
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-authoritative-powershell-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-authoritative-powershell-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-authoritative-powershell-1", "done"),
            ev_completed("resp-authoritative-powershell-2"),
        ]),
    )
    .await;

    submit_user_turn(
        &test,
        "run a read-only command with authoritative Windows PowerShell",
        AskForApproval::UnlessTrusted,
        PermissionProfile::Disabled,
        /*collaboration_mode*/ None,
    )
    .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(approval) => {
            panic!(
                "authoritative Windows PowerShell should not require approval: {:?}",
                approval.command
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function call output should include a string output payload");
    assert!(
        !output.contains("rejected:") && !output.contains("blocked by policy"),
        "unexpected output: {output}"
    );

    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn unified_exec_disabled_windows_sandbox_rejects_managed_read_only_command() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        enable_unified_exec(config);
        config
            .features
            .disable(Feature::WindowsSandbox)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::WindowsSandboxElevated)
            .expect("test config should allow feature update");
        config.set_windows_sandbox_enabled(false);
        config.set_windows_elevated_sandbox_enabled(false);
    });
    let test = builder.build(&server).await?;
    let call_id = "unified-exec-disabled-windows-sandbox-read-only";
    let args = json!({
        "cmd": "cmd.exe /c dir",
        "yield_time_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-disabled-windows-sandbox-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-disabled-windows-sandbox-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-disabled-windows-sandbox-1", "done"),
            ev_completed("resp-disabled-windows-sandbox-2"),
        ]),
    )
    .await;

    submit_user_turn(
        &test,
        "run unified exec with disabled Windows sandbox",
        AskForApproval::Never,
        PermissionProfile::read_only(),
        None,
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function call output should include a string output payload");
    assert!(
        output.contains("cmd.exe /c dir") && output.contains("rejected: blocked by policy"),
        "unexpected output: {output}",
    );

    Ok(())
}

#[tokio::test]
async fn execpolicy_blocks_shell_invocation() -> Result<()> {
    let mut builder = test_codex().with_config(|config| {
        let policy_path = config.codex_home.join("rules").join("policy.rules");
        fs::create_dir_all(
            policy_path
                .parent()
                .expect("policy directory must have a parent"),
        )
        .expect("create policy directory");
        fs::write(
            &policy_path,
            r#"prefix_rule(pattern=["echo"], decision="forbidden")"#,
        )
        .expect("write policy file");
    });
    let server = start_mock_server().await;
    let test = builder.build(&server).await?;

    let call_id = "shell-forbidden";
    let args = json!({
        "command": "echo blocked",
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run shell command".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let EventMsg::ExecCommandEnd(end) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandEnd(_))
    })
    .await
    else {
        unreachable!()
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert!(
        end.aggregated_output
            .contains("policy forbids commands starting with `echo`"),
        "unexpected output: {}",
        end.aggregated_output
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_empty_script_with_collaboration_mode_does_not_panic() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::CollaborationModes)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let call_id = "shell-empty-script-collab";
    let args = json!({
        "command": "",
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-empty-shell-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-empty-shell-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-empty-shell-1", "done"),
            ev_completed("resp-empty-shell-2"),
        ]),
    )
    .await;

    let collaboration_mode = collaboration_mode_for_model(test.session_configured.model.clone());
    submit_user_turn(
        &test,
        "run an empty shell command",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
        Some(collaboration_mode),
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_item = results_mock.single_request().function_call_output(call_id);
    assert_no_matched_rules_invariant(&output_item);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_empty_script_with_collaboration_mode_does_not_panic() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::CollaborationModes)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let call_id = "unified-exec-empty-script-collab";
    let args = json!({
        "cmd": "",
        "yield_time_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-empty-unified-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-empty-unified-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-empty-unified-1", "done"),
            ev_completed("resp-empty-unified-2"),
        ]),
    )
    .await;

    let collaboration_mode = collaboration_mode_for_model(test.session_configured.model.clone());
    submit_user_turn(
        &test,
        "run empty unified exec command",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
        Some(collaboration_mode),
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_item = results_mock.single_request().function_call_output(call_id);
    assert_no_matched_rules_invariant(&output_item);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_whitespace_script_with_collaboration_mode_does_not_panic() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::CollaborationModes)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let call_id = "shell-whitespace-script-collab";
    let args = json!({
        "command": "  \n\t  ",
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-whitespace-shell-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-whitespace-shell-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-whitespace-shell-1", "done"),
            ev_completed("resp-whitespace-shell-2"),
        ]),
    )
    .await;

    let collaboration_mode = collaboration_mode_for_model(test.session_configured.model.clone());
    submit_user_turn(
        &test,
        "run whitespace shell command",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
        Some(collaboration_mode),
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_item = results_mock.single_request().function_call_output(call_id);
    assert_no_matched_rules_invariant(&output_item);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_whitespace_script_with_collaboration_mode_does_not_panic() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::CollaborationModes)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let call_id = "unified-exec-whitespace-script-collab";
    let args = json!({
        "cmd": " \n \t",
        "yield_time_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-whitespace-unified-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-whitespace-unified-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-whitespace-unified-1", "done"),
            ev_completed("resp-whitespace-unified-2"),
        ]),
    )
    .await;

    let collaboration_mode = collaboration_mode_for_model(test.session_configured.model.clone());
    submit_user_turn(
        &test,
        "run whitespace unified exec command",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
        Some(collaboration_mode),
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_item = results_mock.single_request().function_call_output(call_id);
    assert_no_matched_rules_invariant(&output_item);

    Ok(())
}
