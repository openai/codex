#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[derive(Clone, Copy)]
enum TargetPath {
    Workspace(&'static str),
    OutsideWorkspace(&'static str),
}

impl TargetPath {
    fn resolve_for_patch(self, test: &TestCodex) -> (PathBuf, String) {
        match self {
            TargetPath::Workspace(name) => {
                let path = test.cwd.path().join(name);
                (path, name.to_string())
            }
            TargetPath::OutsideWorkspace(name) => {
                let path = env::current_dir()
                    .expect("current dir should be available")
                    .join(name);
                (path.clone(), path.display().to_string())
            }
        }
    }
}

#[derive(Clone)]
enum ActionKind {
    WriteFile {
        target: TargetPath,
        content: &'static str,
    },
    FetchUrl {
        endpoint: &'static str,
        response_body: &'static str,
    },
    RunCommand {
        command: &'static [&'static str],
    },
    ApplyPatchFunction {
        target: TargetPath,
        content: &'static str,
    },
    ApplyPatchShell {
        target: TargetPath,
        content: &'static str,
    },
}

impl ActionKind {
    async fn prepare(
        &self,
        test: &TestCodex,
        server: &MockServer,
        call_id: &str,
        with_escalated_permissions: bool,
    ) -> Result<(Value, Option<Vec<String>>)> {
        match self {
            ActionKind::WriteFile { target, content } => {
                let (path, _) = target.resolve_for_patch(test);
                let _ = fs::remove_file(&path);
                let command = vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    format!(
                        "printf {content:?} > {path:?} && cat {path:?}",
                        content = content,
                        path = path
                    ),
                ];
                let event = shell_event(call_id, &command, 1_000, with_escalated_permissions)?;
                Ok((event, Some(command)))
            }
            ActionKind::FetchUrl {
                endpoint,
                response_body,
            } => {
                Mock::given(method("GET"))
                    .and(path(*endpoint))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_string(response_body.to_string()),
                    )
                    .mount(server)
                    .await;

                let url = format!("{}{}", server.uri(), endpoint);
                let script = format!(
                    "import sys\nimport urllib.request\nurl = {url:?}\ntry:\n    data = urllib.request.urlopen(url, timeout=2).read().decode()\n    print('OK:' + data.strip())\nexcept Exception as exc:\n    print('ERR:' + exc.__class__.__name__)\n    sys.exit(1)",
                );

                let command = vec!["python3".to_string(), "-c".to_string(), script];
                let event = shell_event(call_id, &command, 1_000, with_escalated_permissions)?;
                Ok((event, Some(command)))
            }
            ActionKind::RunCommand { command } => {
                let command: Vec<String> = command
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                let event = shell_event(call_id, &command, 1_000, with_escalated_permissions)?;
                Ok((event, Some(command)))
            }
            ActionKind::ApplyPatchFunction { target, content } => {
                let (path, patch_path) = target.resolve_for_patch(test);
                let _ = fs::remove_file(&path);
                let patch = build_add_file_patch(&patch_path, content);
                Ok((ev_apply_patch_function_call(call_id, &patch), None))
            }
            ActionKind::ApplyPatchShell { target, content } => {
                let (path, patch_path) = target.resolve_for_patch(test);
                let _ = fs::remove_file(&path);
                let patch = build_add_file_patch(&patch_path, content);
                let command = shell_apply_patch_command(&patch);
                let event = shell_event(call_id, &command, 5_000, with_escalated_permissions)?;
                Ok((event, Some(command)))
            }
        }
    }
}

fn build_add_file_patch(patch_path: &str, content: &str) -> String {
    format!("*** Begin Patch\n*** Add File: {patch_path}\n+{content}\n*** End Patch\n")
}

fn shell_apply_patch_command(patch: &str) -> Vec<String> {
    let mut script = String::from("apply_patch <<'PATCH'\n");
    script.push_str(patch);
    if !patch.ends_with('\n') {
        script.push('\n');
    }
    script.push_str("PATCH\n");
    vec!["bash".to_string(), "-lc".to_string(), script]
}

fn shell_event(
    call_id: &str,
    command: &[String],
    timeout_ms: u64,
    with_escalated_permissions: bool,
) -> Result<Value> {
    let mut args = json!({
        "command": command,
        "timeout_ms": timeout_ms,
    });
    if with_escalated_permissions {
        args["with_escalated_permissions"] = json!(true);
    }
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "shell", &args_str))
}

struct CommandResult {
    exit_code: Option<i64>,
    stdout: String,
}

async fn submit_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    Ok(())
}

fn parse_result(item: &Value) -> CommandResult {
    let output_str = item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output payload");
    match serde_json::from_str::<Value>(output_str) {
        Ok(parsed) => parse_result_json(&parsed),
        Err(_) => parse_result_freeform(output_str),
    }
}

fn parse_result_json(parsed: &Value) -> CommandResult {
    let exit_code = parsed["metadata"]["exit_code"].as_i64();
    let stdout = parsed["output"].as_str().unwrap_or_default().to_string();
    CommandResult { exit_code, stdout }
}

fn parse_result_freeform(output_str: &str) -> CommandResult {
    CommandResult {
        exit_code: None,
        stdout: output_str.to_string(),
    }
}

async fn expect_exec_approval(
    test: &TestCodex,
    expected_command: &[String],
) -> ExecApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TaskComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ExecApprovalRequest(approval) => {
            assert_eq!(approval.command, expected_command);
            approval
        }
        EventMsg::TaskComplete(_) => panic!("expected approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn expect_patch_approval(
    test: &TestCodex,
    expected_call_id: &str,
) -> ApplyPatchApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TaskComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ApplyPatchApprovalRequest(approval) => {
            assert_eq!(approval.call_id, expected_call_id);
            approval
        }
        EventMsg::TaskComplete(_) => panic!("expected patch approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TaskComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TaskComplete(_) => {}
        EventMsg::ExecApprovalRequest(event) => {
            panic!("unexpected approval request: {:?}", event.command)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;
}

fn workspace_write(network_access: bool) -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    }
}

async fn prepare_test(
    server: &MockServer,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    requires_apply_patch_tool: bool,
    model_override: Option<&'static str>,
) -> Result<TestCodex> {
    let sandbox_policy_for_config = sandbox_policy.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = approval_policy;
        config.sandbox_policy = sandbox_policy_for_config.clone();
        let model = model_override.unwrap_or("gpt-5");
        config.model = model.to_string();
        config.model_family =
            find_family_for_model(model).expect("model should map to a known family");
        if requires_apply_patch_tool {
            config.include_apply_patch_tool = true;
        }
    });

    builder.build(server).await
}

async fn complete_auto(test: &TestCodex) -> Result<()> {
    wait_for_completion_without_approval(test).await;
    Ok(())
}

async fn complete_with_exec_approval(
    test: &TestCodex,
    expected_command: &[String],
    decision: ReviewDecision,
    expected_reason: Option<&'static str>,
) -> Result<()> {
    let approval = expect_exec_approval(test, expected_command).await;
    if let Some(expected_reason) = expected_reason {
        assert_eq!(
            approval.reason.as_deref(),
            Some(expected_reason),
            "unexpected approval reason",
        );
    }
    test.codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision,
        })
        .await?;
    wait_for_completion(test).await;
    Ok(())
}

async fn complete_with_patch_approval(
    test: &TestCodex,
    call_id: &str,
    decision: ReviewDecision,
    expected_reason: Option<&'static str>,
) -> Result<()> {
    let approval = expect_patch_approval(test, call_id).await;
    if let Some(expected_reason) = expected_reason {
        assert_eq!(
            approval.reason.as_deref(),
            Some(expected_reason),
            "unexpected patch approval reason",
        );
    }
    test.codex
        .submit(Op::PatchApproval {
            id: "0".into(),
            decision,
        })
        .await?;
    wait_for_completion(test).await;
    Ok(())
}

fn assert_file_created(
    test: &TestCodex,
    result: &CommandResult,
    target: TargetPath,
    content: &'static str,
) -> Result<()> {
    let (path, _) = target.resolve_for_patch(test);
    assert_eq!(
        result.exit_code,
        Some(0),
        "expected successful exit for {:?}",
        path
    );
    assert!(
        result.stdout.contains(content),
        "stdout missing {content:?}: {}",
        result.stdout
    );
    let file_contents = fs::read_to_string(&path)?;
    assert!(
        file_contents.contains(content),
        "file contents missing {content:?}: {file_contents}"
    );
    let _ = fs::remove_file(path);
    Ok(())
}

fn assert_patch_applied(
    test: &TestCodex,
    result: &CommandResult,
    target: TargetPath,
    content: &'static str,
) -> Result<()> {
    let (path, _) = target.resolve_for_patch(test);
    match result.exit_code {
        Some(0) | None => {
            if result.exit_code.is_none() {
                assert!(
                    result.stdout.contains("Success."),
                    "patch output missing success indicator: {}",
                    result.stdout
                );
            }
        }
        Some(code) => panic!(
            "expected successful patch exit for {:?}, got {code} with stdout {}",
            path, result.stdout
        ),
    }
    let file_contents = fs::read_to_string(&path)?;
    assert!(
        file_contents.contains(content),
        "patched file missing {content:?}: {file_contents}"
    );
    let _ = fs::remove_file(path);
    Ok(())
}

fn assert_file_not_created(
    test: &TestCodex,
    result: &CommandResult,
    target: TargetPath,
    message_contains: &'static [&'static str],
) -> Result<()> {
    let (path, _) = target.resolve_for_patch(test);
    assert_ne!(
        result.exit_code,
        Some(0),
        "expected non-zero exit for {path:?}"
    );
    for needle in message_contains {
        if needle.contains('|') {
            let options: Vec<&str> = needle.split('|').collect();
            let matches_any = options.iter().any(|option| result.stdout.contains(option));
            assert!(
                matches_any,
                "stdout missing one of {options:?}: {}",
                result.stdout
            );
        } else {
            assert!(
                result.stdout.contains(needle),
                "stdout missing {needle:?}: {}",
                result.stdout
            );
        }
    }
    assert!(!path.exists(), "command should not create {path:?}");
    Ok(())
}

fn assert_network_success(result: &CommandResult, body_contains: &'static str) -> Result<()> {
    assert_eq!(
        result.exit_code,
        Some(0),
        "expected successful network exit: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains("OK:"),
        "stdout missing OK prefix: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains(body_contains),
        "stdout missing body text {body_contains:?}: {}",
        result.stdout
    );
    Ok(())
}

fn assert_network_failure(result: &CommandResult, expect_tag: &'static str) -> Result<()> {
    assert_ne!(
        result.exit_code,
        Some(0),
        "expected non-zero exit for network failure: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains("ERR:"),
        "stdout missing ERR prefix: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains(expect_tag),
        "stdout missing expected tag {expect_tag:?}: {}",
        result.stdout
    );
    Ok(())
}

fn assert_command_success(result: &CommandResult, stdout_contains: &'static str) -> Result<()> {
    assert_eq!(
        result.exit_code,
        Some(0),
        "expected successful trusted command exit: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains(stdout_contains),
        "trusted command stdout missing {stdout_contains:?}: {}",
        result.stdout
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_on_request_allows_outside_write() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "danger_full_access_on_request_allows_outside_write";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("dfa_on_request.txt"),
        content: "danger-on-request",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("dfa_on_request.txt"),
        "danger-on-request",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_on_request_allows_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "danger_full_access_on_request_allows_network";
    let action = ActionKind::FetchUrl {
        endpoint: "/dfa/network",
        response_body: "danger-network-ok",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_network_success(&result, "danger-network-ok")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_command_unless_trusted_runs_without_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "trusted_command_unless_trusted_runs_without_prompt";
    let action = ActionKind::RunCommand {
        command: &["echo", "trusted-unless"],
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_command_success(&result, "trusted-unless")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_on_failure_allows_outside_write() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::OnFailure;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "danger_full_access_on_failure_allows_outside_write";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("dfa_on_failure.txt"),
        content: "danger-on-failure",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("dfa_on_failure.txt"),
        "danger-on-failure",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_unless_trusted_requests_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "danger_full_access_unless_trusted_requests_approval";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("dfa_unless_trusted.txt"),
        content: "danger-unless-trusted",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("dfa_unless_trusted.txt"),
        "danger-unless-trusted",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_never_allows_outside_write() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::Never;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "danger_full_access_never_allows_outside_write";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("dfa_never.txt"),
        content: "danger-never",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("dfa_never.txt"),
        "danger-never",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_on_request_requires_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_on_request_requires_approval";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ro_on_request.txt"),
        content: "read-only-approval",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, true).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::Workspace("ro_on_request.txt"),
        "read-only-approval",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_command_on_request_read_only_runs_without_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "trusted_command_on_request_read_only_runs_without_prompt";
    let action = ActionKind::RunCommand {
        command: &["echo", "trusted-read-only"],
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_command_success(&result, "trusted-read-only")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_on_request_blocks_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_on_request_blocks_network";
    let action = ActionKind::FetchUrl {
        endpoint: "/ro/network-blocked",
        response_body: "should-not-see",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_network_failure(&result, "ERR:")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_on_request_denied_blocks_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_on_request_denied_blocks_execution";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ro_on_request_denied.txt"),
        content: "should-not-write",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, true).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Denied, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_not_created(
        &test,
        &result,
        TargetPath::Workspace("ro_on_request_denied.txt"),
        &["exec command rejected by user"],
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_on_failure_escalates_after_sandbox_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnFailure;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_on_failure_escalates_after_sandbox_error";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ro_on_failure.txt"),
        content: "read-only-on-failure",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(
        &test,
        exec_command,
        ReviewDecision::Approved,
        Some("command failed; retry without sandbox?"),
    )
    .await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::Workspace("ro_on_failure.txt"),
        "read-only-on-failure",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_on_request_network_escalates_when_approved() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_on_request_network_escalates_when_approved";
    let action = ActionKind::FetchUrl {
        endpoint: "/ro/network-approved",
        response_body: "read-only-network-ok",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, true).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_network_success(&result, "read-only-network-ok")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_requires_patch_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(&server, approval_policy, sandbox_policy.clone(), true, None).await?;

    let call_id = "apply_patch_shell_requires_patch_approval";
    let action = ActionKind::ApplyPatchShell {
        target: TargetPath::Workspace("apply_patch_shell.txt"),
        content: "shell-apply-patch",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_with_patch_approval(&test, call_id, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::Workspace("apply_patch_shell.txt"),
        "shell-apply-patch",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_auto_inside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_auto_inside_workspace";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::Workspace("apply_patch_function.txt"),
        content: "function-apply-patch",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::Workspace("apply_patch_function.txt"),
        "function-apply-patch",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_danger_allows_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_danger_allows_outside_workspace";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::OutsideWorkspace("apply_patch_function_danger.txt"),
        content: "function-patch-danger",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::OutsideWorkspace("apply_patch_function_danger.txt"),
        "function-patch-danger",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_outside_requires_patch_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_outside_requires_patch_approval";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::OutsideWorkspace("apply_patch_function_outside.txt"),
        content: "function-patch-outside",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_with_patch_approval(&test, call_id, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::OutsideWorkspace("apply_patch_function_outside.txt"),
        "function-patch-outside",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_outside_denied_blocks_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_outside_denied_blocks_patch";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::OutsideWorkspace("apply_patch_function_outside_denied.txt"),
        content: "function-patch-outside-denied",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_with_patch_approval(&test, call_id, ReviewDecision::Denied, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_not_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("apply_patch_function_outside_denied.txt"),
        &["patch rejected by user"],
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_outside_requires_patch_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(&server, approval_policy, sandbox_policy.clone(), true, None).await?;

    let call_id = "apply_patch_shell_outside_requires_patch_approval";
    let action = ActionKind::ApplyPatchShell {
        target: TargetPath::OutsideWorkspace("apply_patch_shell_outside.txt"),
        content: "shell-patch-outside",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_with_patch_approval(&test, call_id, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::OutsideWorkspace("apply_patch_shell_outside.txt"),
        "shell-patch-outside",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_unless_trusted_requires_patch_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_unless_trusted_requires_patch_approval";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::Workspace("apply_patch_function_unless_trusted.txt"),
        content: "function-patch-unless-trusted",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_with_patch_approval(&test, call_id, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_patch_applied(
        &test,
        &result,
        TargetPath::Workspace("apply_patch_function_unless_trusted.txt"),
        "function-patch-unless-trusted",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_never_rejects_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::Never;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        true,
        Some("gpt-5-codex"),
    )
    .await?;

    let call_id = "apply_patch_function_never_rejects_outside_workspace";
    let action = ActionKind::ApplyPatchFunction {
        target: TargetPath::OutsideWorkspace("apply_patch_function_never.txt"),
        content: "function-patch-never",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_not_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("apply_patch_function_never.txt"),
        &["patch rejected: writing outside of the project; rejected by user approval settings"],
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_unless_trusted_requires_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_unless_trusted_requires_approval";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ro_unless_trusted.txt"),
        content: "read-only-unless-trusted",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::Workspace("ro_unless_trusted.txt"),
        "read-only-unless-trusted",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_only_never_reports_sandbox_failure() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::Never;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "read_only_never_reports_sandbox_failure";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ro_never.txt"),
        content: "read-only-never",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_not_created(
        &test,
        &result,
        TargetPath::Workspace("ro_never.txt"),
        if cfg!(target_os = "linux") {
            &["Permission denied"]
        } else {
            &["Permission denied|Operation not permitted|Read-only file system"]
        },
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_command_never_runs_without_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let approval_policy = AskForApproval::Never;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "trusted_command_never_runs_without_prompt";
    let action = ActionKind::RunCommand {
        command: &["echo", "trusted-never"],
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_command_success(&result, "trusted-never")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_on_request_allows_workspace_write() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_on_request_allows_workspace_write";
    let action = ActionKind::WriteFile {
        target: TargetPath::Workspace("ww_on_request.txt"),
        content: "workspace-on-request",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::Workspace("ww_on_request.txt"),
        "workspace-on-request",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_network_disabled_blocks_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_network_disabled_blocks_network";
    let action = ActionKind::FetchUrl {
        endpoint: "/ww/network-blocked",
        response_body: "workspace-network-blocked",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_network_failure(&result, "ERR:")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_on_request_requires_approval_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_on_request_requires_approval_outside_workspace";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("ww_on_request_outside.txt"),
        content: "workspace-on-request-outside",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, true).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("ww_on_request_outside.txt"),
        "workspace-on-request-outside",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_network_enabled_allows_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(true);
    let approval_policy = AskForApproval::OnRequest;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_network_enabled_allows_network";
    let action = ActionKind::FetchUrl {
        endpoint: "/ww/network-ok",
        response_body: "workspace-network-ok",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_network_success(&result, "workspace-network-ok")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_on_failure_escalates_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::OnFailure;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_on_failure_escalates_outside_workspace";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("ww_on_failure.txt"),
        content: "workspace-on-failure",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(
        &test,
        exec_command,
        ReviewDecision::Approved,
        Some("command failed; retry without sandbox?"),
    )
    .await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("ww_on_failure.txt"),
        "workspace-on-failure",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_unless_trusted_requires_approval_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::UnlessTrusted;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_unless_trusted_requires_approval_outside_workspace";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("ww_unless_trusted.txt"),
        content: "workspace-unless-trusted",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let exec_command = _expected_command
        .as_ref()
        .expect("exec approval requires shell command");
    complete_with_exec_approval(&test, exec_command, ReviewDecision::Approved, None).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("ww_unless_trusted.txt"),
        "workspace-unless-trusted",
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_never_blocks_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let sandbox_policy = workspace_write(false);
    let approval_policy = AskForApproval::Never;

    let test = prepare_test(
        &server,
        approval_policy,
        sandbox_policy.clone(),
        false,
        None,
    )
    .await?;

    let call_id = "workspace_write_never_blocks_outside_workspace";
    let action = ActionKind::WriteFile {
        target: TargetPath::OutsideWorkspace("ww_never.txt"),
        content: "workspace-never",
    };

    let (event, _expected_command) = action.prepare(&test, &server, call_id, false).await?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    complete_auto(&test).await?;

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    assert_file_not_created(
        &test,
        &result,
        TargetPath::OutsideWorkspace("ww_never.txt"),
        if cfg!(target_os = "linux") {
            &["Permission denied"]
        } else {
            &["Permission denied|Operation not permitted|Read-only file system"]
        },
    )?;

    Ok(())
}
