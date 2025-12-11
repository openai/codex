#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecPolicyAmendment;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::sandboxing::SandboxPermissions;
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
use regex_lite::Regex;
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
        command: &'static str,
    },
    RunUnifiedExecCommand {
        command: &'static str,
        justification: Option<&'static str>,
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

const DEFAULT_UNIFIED_EXEC_JUSTIFICATION: &str =
    "Requires escalated permissions to bypass the sandbox in tests.";

impl ActionKind {
    async fn prepare(
        &self,
        test: &TestCodex,
        server: &MockServer,
        call_id: &str,
        sandbox_permissions: SandboxPermissions,
    ) -> Result<(Value, Option<String>)> {
        match self {
            ActionKind::WriteFile { target, content } => {
                let (path, _) = target.resolve_for_patch(test);
                let _ = fs::remove_file(&path);
                let command = format!("printf {content:?} > {path:?} && cat {path:?}");
                let event = shell_event(call_id, &command, 1_000, sandbox_permissions)?;
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
                let escaped_url = url.replace('\'', "\\'");
                let script = format!(
                    "import sys\nimport urllib.request\nurl = '{escaped_url}'\ntry:\n    data = urllib.request.urlopen(url, timeout=2).read().decode()\n    print('OK:' + data.strip())\nexcept Exception as exc:\n    print('ERR:' + exc.__class__.__name__)\n    sys.exit(1)",
                );

                let command = format!("python3 -c \"{script}\"");
                let event = shell_event(call_id, &command, 3_000, sandbox_permissions)?;
                Ok((event, Some(command)))
            }
            ActionKind::RunCommand { command } => {
                let event = shell_event(call_id, command, 1_000, sandbox_permissions)?;
                Ok((event, Some(command.to_string())))
            }
            ActionKind::RunUnifiedExecCommand {
                command,
                justification,
            } => {
                let event = exec_command_event(
                    call_id,
                    command,
                    Some(1000),
                    sandbox_permissions,
                    *justification,
                )?;
                Ok((event, Some(command.to_string())))
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
                let event = shell_event(call_id, &command, 5_000, sandbox_permissions)?;
                Ok((event, Some(command)))
            }
        }
    }
}

fn build_add_file_patch(patch_path: &str, content: &str) -> String {
    format!("*** Begin Patch\n*** Add File: {patch_path}\n+{content}\n*** End Patch\n")
}

fn shell_apply_patch_command(patch: &str) -> String {
    let mut script = String::from("apply_patch <<'PATCH'\n");
    script.push_str(patch);
    if !patch.ends_with('\n') {
        script.push('\n');
    }
    script.push_str("PATCH\n");
    script
}

fn shell_event(
    call_id: &str,
    command: &str,
    timeout_ms: u64,
    sandbox_permissions: SandboxPermissions,
) -> Result<Value> {
    let mut args = json!({
        "command": command,
        "timeout_ms": timeout_ms,
    });
    if sandbox_permissions.requires_escalated_permissions() {
        args["sandbox_permissions"] = json!(sandbox_permissions);
    }
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "shell_command", &args_str))
}

fn exec_command_event(
    call_id: &str,
    cmd: &str,
    yield_time_ms: Option<u64>,
    sandbox_permissions: SandboxPermissions,
    justification: Option<&str>,
) -> Result<Value> {
    let mut args = json!({
        "cmd": cmd.to_string(),
    });
    if let Some(yield_time_ms) = yield_time_ms {
        args["yield_time_ms"] = json!(yield_time_ms);
    }
    if sandbox_permissions.requires_escalated_permissions() {
        args["sandbox_permissions"] = json!(sandbox_permissions);
        let reason = justification.unwrap_or(DEFAULT_UNIFIED_EXEC_JUSTIFICATION);
        args["justification"] = json!(reason);
    }
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
}

#[derive(Clone)]
enum Expectation {
    FileCreated {
        target: TargetPath,
        content: &'static str,
    },
    FileCreatedNoExitCode {
        target: TargetPath,
        content: &'static str,
    },
    PatchApplied {
        target: TargetPath,
        content: &'static str,
    },
    FileNotCreated {
        target: TargetPath,
        message_contains: &'static [&'static str],
    },
    NetworkSuccess {
        body_contains: &'static str,
    },
    NetworkSuccessNoExitCode {
        body_contains: &'static str,
    },
    NetworkFailure {
        expect_tag: &'static str,
    },
    CommandSuccess {
        stdout_contains: &'static str,
    },
    CommandSuccessNoExitCode {
        stdout_contains: &'static str,
    },
    CommandFailure {
        output_contains: &'static str,
    },
}

impl Expectation {
    fn verify(&self, test: &TestCodex, result: &CommandResult) -> Result<()> {
        match self {
            Expectation::FileCreated { target, content } => {
                let (path, _) = target.resolve_for_patch(test);
                assert_eq!(
                    result.exit_code,
                    Some(0),
                    "expected successful exit for {path:?}"
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
            }
            Expectation::FileCreatedNoExitCode { target, content } => {
                let (path, _) = target.resolve_for_patch(test);
                assert!(
                    result.exit_code.is_none() || result.exit_code == Some(0),
                    "expected no exit code for {path:?}",
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
            }
            Expectation::PatchApplied { target, content } => {
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
            }
            Expectation::FileNotCreated {
                target,
                message_contains,
            } => {
                let (path, _) = target.resolve_for_patch(test);
                assert_ne!(
                    result.exit_code,
                    Some(0),
                    "expected non-zero exit for {path:?}"
                );
                for needle in *message_contains {
                    if needle.contains('|') {
                        let options: Vec<&str> = needle.split('|').collect();
                        let matches_any =
                            options.iter().any(|option| result.stdout.contains(option));
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
                assert!(
                    !path.exists(),
                    "command should not create {path:?}, but file exists"
                );
            }
            Expectation::NetworkSuccess { body_contains } => {
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
            }
            Expectation::NetworkSuccessNoExitCode { body_contains } => {
                assert!(
                    result.exit_code.is_none() || result.exit_code == Some(0),
                    "expected no exit code for successful network call: {}",
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
            }
            Expectation::NetworkFailure { expect_tag } => {
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
            }
            Expectation::CommandSuccess { stdout_contains } => {
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
            }
            Expectation::CommandSuccessNoExitCode { stdout_contains } => {
                assert!(
                    result.exit_code.is_none() || result.exit_code == Some(0),
                    "expected no exit code for trusted command: {}",
                    result.stdout
                );
                assert!(
                    result.stdout.contains(stdout_contains),
                    "trusted command stdout missing {stdout_contains:?}: {}",
                    result.stdout
                );
            }
            Expectation::CommandFailure { output_contains } => {
                assert_ne!(
                    result.exit_code,
                    Some(0),
                    "expected non-zero exit for command failure: {}",
                    result.stdout
                );
                assert!(
                    result.stdout.contains(output_contains),
                    "command failure stderr missing {output_contains:?}: {}",
                    result.stdout
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
enum Outcome {
    Auto,
    ExecApproval {
        decision: ReviewDecision,
        expected_reason: Option<&'static str>,
    },
    PatchApproval {
        decision: ReviewDecision,
        expected_reason: Option<&'static str>,
    },
}

#[derive(Clone)]
struct ScenarioSpec {
    name: &'static str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    action: ActionKind,
    sandbox_permissions: SandboxPermissions,
    features: Vec<Feature>,
    model_override: Option<&'static str>,
    outcome: Outcome,
    expectation: Expectation,
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
        Ok(parsed) => {
            let exit_code = parsed["metadata"]["exit_code"].as_i64();
            let stdout = parsed["output"].as_str().unwrap_or_default().to_string();
            CommandResult { exit_code, stdout }
        }
        Err(_) => {
            let structured = Regex::new(r"(?s)^Exit code:\s*(-?\d+).*?Output:\n(.*)$").unwrap();
            let regex =
                Regex::new(r"(?s)^.*?Process exited with code (\d+)\n.*?Output:\n(.*)$").unwrap();
            // parse freeform output
            if let Some(captures) = structured.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                CommandResult {
                    exit_code: Some(exit_code),
                    stdout: output.to_string(),
                }
            } else if let Some(captures) = regex.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                CommandResult {
                    exit_code: Some(exit_code),
                    stdout: output.to_string(),
                }
            } else {
                CommandResult {
                    exit_code: None,
                    stdout: output_str.to_string(),
                }
            }
        }
    }
}

async fn expect_exec_approval(
    test: &TestCodex,
    expected_command: &str,
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
            let last_arg = approval
                .command
                .last()
                .map(std::string::String::as_str)
                .unwrap_or_default();
            assert_eq!(last_arg, expected_command);
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

fn scenarios() -> Vec<ScenarioSpec> {
    use AskForApproval::*;

    let workspace_write = |network_access| SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    vec![
        ScenarioSpec {
            name: "danger_full_access_on_request_allows_outside_write",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_on_request.txt"),
                content: "danger-on-request",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("dfa_on_request.txt"),
                content: "danger-on-request",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_on_request_allows_outside_write_gpt_5_1_no_exit",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_on_request_5_1.txt"),
                content: "danger-on-request",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("dfa_on_request_5_1.txt"),
                content: "danger-on-request",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_on_request_allows_network",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::FetchUrl {
                endpoint: "/dfa/network",
                response_body: "danger-network-ok",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::NetworkSuccess {
                body_contains: "danger-network-ok",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_on_request_allows_network_gpt_5_1_no_exit",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::FetchUrl {
                endpoint: "/dfa/network",
                response_body: "danger-network-ok",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::NetworkSuccessNoExitCode {
                body_contains: "danger-network-ok",
            },
        },
        ScenarioSpec {
            name: "trusted_command_unless_trusted_runs_without_prompt",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::RunCommand {
                command: "echo trusted-unless",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccess {
                stdout_contains: "trusted-unless",
            },
        },
        ScenarioSpec {
            name: "trusted_command_unless_trusted_runs_without_prompt_gpt_5_1_no_exit",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::RunCommand {
                command: "echo trusted-unless",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccessNoExitCode {
                stdout_contains: "trusted-unless",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_on_failure_allows_outside_write",
            approval_policy: OnFailure,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_on_failure.txt"),
                content: "danger-on-failure",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("dfa_on_failure.txt"),
                content: "danger-on-failure",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_on_failure_allows_outside_write_gpt_5_1_no_exit",
            approval_policy: OnFailure,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_on_failure_5_1.txt"),
                content: "danger-on-failure",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::OutsideWorkspace("dfa_on_failure_5_1.txt"),
                content: "danger-on-failure",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_unless_trusted_requests_approval",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_unless_trusted.txt"),
                content: "danger-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("dfa_unless_trusted.txt"),
                content: "danger-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_unless_trusted_requests_approval_gpt_5_1_no_exit",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_unless_trusted_5_1.txt"),
                content: "danger-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::OutsideWorkspace("dfa_unless_trusted_5_1.txt"),
                content: "danger-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_never_allows_outside_write",
            approval_policy: Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_never.txt"),
                content: "danger-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("dfa_never.txt"),
                content: "danger-never",
            },
        },
        ScenarioSpec {
            name: "danger_full_access_never_allows_outside_write_gpt_5_1_no_exit",
            approval_policy: Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("dfa_never_5_1.txt"),
                content: "danger-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::OutsideWorkspace("dfa_never_5_1.txt"),
                content: "danger-never",
            },
        },
        ScenarioSpec {
            name: "read_only_on_request_requires_approval",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_on_request.txt"),
                content: "read-only-approval",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::Workspace("ro_on_request.txt"),
                content: "read-only-approval",
            },
        },
        ScenarioSpec {
            name: "read_only_on_request_requires_approval_gpt_5_1_no_exit",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_on_request_5_1.txt"),
                content: "read-only-approval",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::Workspace("ro_on_request_5_1.txt"),
                content: "read-only-approval",
            },
        },
        ScenarioSpec {
            name: "trusted_command_on_request_read_only_runs_without_prompt",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::RunCommand {
                command: "echo trusted-read-only",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccess {
                stdout_contains: "trusted-read-only",
            },
        },
        ScenarioSpec {
            name: "trusted_command_on_request_read_only_runs_without_prompt_gpt_5_1_no_exit",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::RunCommand {
                command: "echo trusted-read-only",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccessNoExitCode {
                stdout_contains: "trusted-read-only",
            },
        },
        ScenarioSpec {
            name: "read_only_on_request_blocks_network",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::FetchUrl {
                endpoint: "/ro/network-blocked",
                response_body: "should-not-see",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::Auto,
            expectation: Expectation::NetworkFailure { expect_tag: "ERR:" },
        },
        ScenarioSpec {
            name: "read_only_on_request_denied_blocks_execution",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_on_request_denied.txt"),
                content: "should-not-write",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: None,
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Denied,
                expected_reason: None,
            },
            expectation: Expectation::FileNotCreated {
                target: TargetPath::Workspace("ro_on_request_denied.txt"),
                message_contains: &["exec command rejected by user"],
            },
        },
        #[cfg(not(target_os = "linux"))] // TODO (pakrym): figure out why linux behaves differently
        ScenarioSpec {
            name: "read_only_on_failure_escalates_after_sandbox_error",
            approval_policy: OnFailure,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_on_failure.txt"),
                content: "read-only-on-failure",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: Some("command failed; retry without sandbox?"),
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::Workspace("ro_on_failure.txt"),
                content: "read-only-on-failure",
            },
        },
        #[cfg(not(target_os = "linux"))]
        ScenarioSpec {
            name: "read_only_on_failure_escalates_after_sandbox_error_gpt_5_1_no_exit",
            approval_policy: OnFailure,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_on_failure_5_1.txt"),
                content: "read-only-on-failure",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: Some("command failed; retry without sandbox?"),
            },
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::Workspace("ro_on_failure_5_1.txt"),
                content: "read-only-on-failure",
            },
        },
        ScenarioSpec {
            name: "read_only_on_request_network_escalates_when_approved",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::FetchUrl {
                endpoint: "/ro/network-approved",
                response_body: "read-only-network-ok",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::NetworkSuccess {
                body_contains: "read-only-network-ok",
            },
        },
        ScenarioSpec {
            name: "read_only_on_request_network_escalates_when_approved_gpt_5_1_no_exit",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::FetchUrl {
                endpoint: "/ro/network-approved",
                response_body: "read-only-network-ok",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::NetworkSuccessNoExitCode {
                body_contains: "read-only-network-ok",
            },
        },
        ScenarioSpec {
            name: "apply_patch_shell_command_requires_patch_approval",
            approval_policy: UnlessTrusted,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchShell {
                target: TargetPath::Workspace("apply_patch_shell.txt"),
                content: "shell-apply-patch",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::PatchApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::PatchApplied {
                target: TargetPath::Workspace("apply_patch_shell.txt"),
                content: "shell-apply-patch",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_auto_inside_workspace",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::Workspace("apply_patch_function.txt"),
                content: "function-apply-patch",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::Auto,
            expectation: Expectation::PatchApplied {
                target: TargetPath::Workspace("apply_patch_function.txt"),
                content: "function-apply-patch",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_danger_allows_outside_workspace",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::OutsideWorkspace("apply_patch_function_danger.txt"),
                content: "function-patch-danger",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![Feature::ApplyPatchFreeform],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::Auto,
            expectation: Expectation::PatchApplied {
                target: TargetPath::OutsideWorkspace("apply_patch_function_danger.txt"),
                content: "function-patch-danger",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_outside_requires_patch_approval",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::OutsideWorkspace("apply_patch_function_outside.txt"),
                content: "function-patch-outside",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::PatchApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::PatchApplied {
                target: TargetPath::OutsideWorkspace("apply_patch_function_outside.txt"),
                content: "function-patch-outside",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_outside_denied_blocks_patch",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::OutsideWorkspace("apply_patch_function_outside_denied.txt"),
                content: "function-patch-outside-denied",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::PatchApproval {
                decision: ReviewDecision::Denied,
                expected_reason: None,
            },
            expectation: Expectation::FileNotCreated {
                target: TargetPath::OutsideWorkspace("apply_patch_function_outside_denied.txt"),
                message_contains: &["patch rejected by user"],
            },
        },
        ScenarioSpec {
            name: "apply_patch_shell_command_outside_requires_patch_approval",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchShell {
                target: TargetPath::OutsideWorkspace("apply_patch_shell_outside.txt"),
                content: "shell-patch-outside",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::PatchApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::PatchApplied {
                target: TargetPath::OutsideWorkspace("apply_patch_shell_outside.txt"),
                content: "shell-patch-outside",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_unless_trusted_requires_patch_approval",
            approval_policy: UnlessTrusted,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::Workspace("apply_patch_function_unless_trusted.txt"),
                content: "function-patch-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::PatchApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::PatchApplied {
                target: TargetPath::Workspace("apply_patch_function_unless_trusted.txt"),
                content: "function-patch-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "apply_patch_function_never_rejects_outside_workspace",
            approval_policy: Never,
            sandbox_policy: workspace_write(false),
            action: ActionKind::ApplyPatchFunction {
                target: TargetPath::OutsideWorkspace("apply_patch_function_never.txt"),
                content: "function-patch-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1-codex"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileNotCreated {
                target: TargetPath::OutsideWorkspace("apply_patch_function_never.txt"),
                message_contains: &[
                    "patch rejected: writing outside of the project; rejected by user approval settings",
                ],
            },
        },
        ScenarioSpec {
            name: "read_only_unless_trusted_requires_approval",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_unless_trusted.txt"),
                content: "read-only-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::Workspace("ro_unless_trusted.txt"),
                content: "read-only-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "read_only_unless_trusted_requires_approval_gpt_5_1_no_exit",
            approval_policy: UnlessTrusted,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_unless_trusted_5_1.txt"),
                content: "read-only-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5.1"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreatedNoExitCode {
                target: TargetPath::Workspace("ro_unless_trusted_5_1.txt"),
                content: "read-only-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "read_only_never_reports_sandbox_failure",
            approval_policy: Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ro_never.txt"),
                content: "read-only-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::Auto,
            expectation: Expectation::FileNotCreated {
                target: TargetPath::Workspace("ro_never.txt"),
                message_contains: if cfg!(target_os = "linux") {
                    &["Permission denied"]
                } else {
                    &[
                        "Permission denied|Operation not permitted|operation not permitted|\
                         Read-only file system",
                    ]
                },
            },
        },
        ScenarioSpec {
            name: "trusted_command_never_runs_without_prompt",
            approval_policy: Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::RunCommand {
                command: "echo trusted-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccess {
                stdout_contains: "trusted-never",
            },
        },
        ScenarioSpec {
            name: "workspace_write_on_request_allows_workspace_write",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::WriteFile {
                target: TargetPath::Workspace("ww_on_request.txt"),
                content: "workspace-on-request",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::FileCreated {
                target: TargetPath::Workspace("ww_on_request.txt"),
                content: "workspace-on-request",
            },
        },
        ScenarioSpec {
            name: "workspace_write_network_disabled_blocks_network",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::FetchUrl {
                endpoint: "/ww/network-blocked",
                response_body: "workspace-network-blocked",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::Auto,
            expectation: Expectation::NetworkFailure { expect_tag: "ERR:" },
        },
        ScenarioSpec {
            name: "workspace_write_on_request_requires_approval_outside_workspace",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(false),
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("ww_on_request_outside.txt"),
                content: "workspace-on-request-outside",
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("ww_on_request_outside.txt"),
                content: "workspace-on-request-outside",
            },
        },
        ScenarioSpec {
            name: "workspace_write_network_enabled_allows_network",
            approval_policy: OnRequest,
            sandbox_policy: workspace_write(true),
            action: ActionKind::FetchUrl {
                endpoint: "/ww/network-ok",
                response_body: "workspace-network-ok",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::NetworkSuccess {
                body_contains: "workspace-network-ok",
            },
        },
        #[cfg(not(target_os = "linux"))] // TODO (pakrym): figure out why linux behaves differently
        ScenarioSpec {
            name: "workspace_write_on_failure_escalates_outside_workspace",
            approval_policy: OnFailure,
            sandbox_policy: workspace_write(false),
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("ww_on_failure.txt"),
                content: "workspace-on-failure",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: Some("command failed; retry without sandbox?"),
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("ww_on_failure.txt"),
                content: "workspace-on-failure",
            },
        },
        ScenarioSpec {
            name: "workspace_write_unless_trusted_requires_approval_outside_workspace",
            approval_policy: UnlessTrusted,
            sandbox_policy: workspace_write(false),
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("ww_unless_trusted.txt"),
                content: "workspace-unless-trusted",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: None,
            },
            expectation: Expectation::FileCreated {
                target: TargetPath::OutsideWorkspace("ww_unless_trusted.txt"),
                content: "workspace-unless-trusted",
            },
        },
        ScenarioSpec {
            name: "workspace_write_never_blocks_outside_workspace",
            approval_policy: Never,
            sandbox_policy: workspace_write(false),
            action: ActionKind::WriteFile {
                target: TargetPath::OutsideWorkspace("ww_never.txt"),
                content: "workspace-never",
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![],
            model_override: None,
            outcome: Outcome::Auto,
            expectation: Expectation::FileNotCreated {
                target: TargetPath::OutsideWorkspace("ww_never.txt"),
                message_contains: if cfg!(target_os = "linux") {
                    &["Permission denied"]
                } else {
                    &[
                        "Permission denied|Operation not permitted|operation not permitted|\
                         Read-only file system",
                    ]
                },
            },
        },
        ScenarioSpec {
            name: "unified exec on request no approval for safe command",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::RunUnifiedExecCommand {
                command: "echo \"hello unified exec\"",
                justification: None,
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![Feature::UnifiedExec],
            model_override: Some("gpt-5"),
            outcome: Outcome::Auto,
            expectation: Expectation::CommandSuccess {
                stdout_contains: "hello unified exec",
            },
        },
        #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
        // Linux sandbox arg0 test workaround doesn't work on ARM
        ScenarioSpec {
            name: "unified exec on request escalated requires approval",
            approval_policy: OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            action: ActionKind::RunUnifiedExecCommand {
                command: "python3 -c 'print('\"'\"'escalated unified exec'\"'\"')'",
                justification: Some(DEFAULT_UNIFIED_EXEC_JUSTIFICATION),
            },
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            features: vec![Feature::UnifiedExec],
            model_override: Some("gpt-5"),
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Approved,
                expected_reason: Some(DEFAULT_UNIFIED_EXEC_JUSTIFICATION),
            },
            expectation: Expectation::CommandSuccess {
                stdout_contains: "escalated unified exec",
            },
        },
        ScenarioSpec {
            name: "unified exec on request requires approval unless trusted",
            approval_policy: AskForApproval::UnlessTrusted,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            action: ActionKind::RunUnifiedExecCommand {
                command: "git reset --hard",
                justification: None,
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            features: vec![Feature::UnifiedExec],
            model_override: None,
            outcome: Outcome::ExecApproval {
                decision: ReviewDecision::Denied,
                expected_reason: None,
            },
            expectation: Expectation::CommandFailure {
                output_contains: "rejected by user",
            },
        },
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approval_matrix_covers_all_modes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for scenario in scenarios() {
        run_scenario(&scenario).await?;
    }

    Ok(())
}

async fn run_scenario(scenario: &ScenarioSpec) -> Result<()> {
    eprintln!("running approval scenario: {}", scenario.name);
    let server = start_mock_server().await;
    let approval_policy = scenario.approval_policy;
    let sandbox_policy = scenario.sandbox_policy.clone();
    let features = scenario.features.clone();
    let model_override = scenario.model_override;
    let model = model_override.unwrap_or("gpt-5.1");

    let mut builder = test_codex().with_model(model).with_config(move |config| {
        config.approval_policy = approval_policy;
        config.sandbox_policy = sandbox_policy.clone();
        for feature in features {
            config.features.enable(feature);
        }
    });
    let test = builder.build(&server).await?;

    let call_id = scenario.name;
    let (event, expected_command) = scenario
        .action
        .prepare(&test, &server, call_id, scenario.sandbox_permissions)
        .await?;

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

    submit_turn(
        &test,
        scenario.name,
        scenario.approval_policy,
        scenario.sandbox_policy.clone(),
    )
    .await?;

    match &scenario.outcome {
        Outcome::Auto => {
            wait_for_completion_without_approval(&test).await;
        }
        Outcome::ExecApproval {
            decision,
            expected_reason,
        } => {
            let command = expected_command
                .as_deref()
                .expect("exec approval requires shell command");
            let approval = expect_exec_approval(&test, command).await;
            if let Some(expected_reason) = expected_reason {
                assert_eq!(
                    approval.reason.as_deref(),
                    Some(*expected_reason),
                    "unexpected approval reason for {}",
                    scenario.name
                );
            }
            test.codex
                .submit(Op::ExecApproval {
                    id: "0".into(),
                    decision: decision.clone(),
                })
                .await?;
            wait_for_completion(&test).await;
        }
        Outcome::PatchApproval {
            decision,
            expected_reason,
        } => {
            let approval = expect_patch_approval(&test, call_id).await;
            if let Some(expected_reason) = expected_reason {
                assert_eq!(
                    approval.reason.as_deref(),
                    Some(*expected_reason),
                    "unexpected patch approval reason for {}",
                    scenario.name
                );
            }
            test.codex
                .submit(Op::PatchApproval {
                    id: "0".into(),
                    decision: decision.clone(),
                })
                .await?;
            wait_for_completion(&test).await;
        }
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let result = parse_result(&output_item);
    scenario.expectation.verify(&test, &result)?;

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(unix)]
async fn approving_execpolicy_amendment_persists_policy_and_skips_future_prompts() -> Result<()> {
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::UnlessTrusted;
    let sandbox_policy = SandboxPolicy::ReadOnly;
    let sandbox_policy_for_config = sandbox_policy.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = approval_policy;
        config.sandbox_policy = sandbox_policy_for_config;
    });
    let test = builder.build(&server).await?;
    let allow_prefix_path = test.cwd.path().join("allow-prefix.txt");
    let _ = fs::remove_file(&allow_prefix_path);

    let call_id_first = "allow-prefix-first";
    let (first_event, expected_command) = ActionKind::RunCommand {
        command: "touch allow-prefix.txt",
    }
    .prepare(
        &test,
        &server,
        call_id_first,
        SandboxPermissions::UseDefault,
    )
    .await?;
    let expected_command =
        expected_command.expect("execpolicy amendment scenario should produce a shell command");
    let expected_execpolicy_amendment =
        ExecPolicyAmendment::new(vec!["touch".to_string(), "allow-prefix.txt".to_string()]);

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-allow-prefix-1"),
            first_event,
            ev_completed("resp-allow-prefix-1"),
        ]),
    )
    .await;
    let first_results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-allow-prefix-1", "done"),
            ev_completed("resp-allow-prefix-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "allow-prefix-first",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    let approval = expect_exec_approval(&test, expected_command.as_str()).await;
    assert_eq!(
        approval.proposed_execpolicy_amendment,
        Some(expected_execpolicy_amendment.clone())
    );

    test.codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment: expected_execpolicy_amendment.clone(),
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let policy_path = test.home.path().join("rules").join("default.rules");
    let policy_contents = fs::read_to_string(&policy_path)?;
    assert!(
        policy_contents
            .contains(r#"prefix_rule(pattern=["touch", "allow-prefix.txt"], decision="allow")"#),
        "unexpected policy contents: {policy_contents}"
    );

    let first_output = parse_result(
        &first_results
            .single_request()
            .function_call_output(call_id_first),
    );
    assert_eq!(first_output.exit_code.unwrap_or(0), 0);
    assert!(
        first_output.stdout.is_empty(),
        "unexpected stdout: {}",
        first_output.stdout
    );
    assert_eq!(
        fs::read_to_string(&allow_prefix_path)?,
        "",
        "unexpected file contents after first run"
    );

    let call_id_second = "allow-prefix-second";
    let (second_event, second_command) = ActionKind::RunCommand {
        command: "touch allow-prefix.txt",
    }
    .prepare(
        &test,
        &server,
        call_id_second,
        SandboxPermissions::UseDefault,
    )
    .await?;
    assert_eq!(second_command.as_deref(), Some(expected_command.as_str()));

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-allow-prefix-3"),
            second_event,
            ev_completed("resp-allow-prefix-3"),
        ]),
    )
    .await;
    let second_results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-allow-prefix-2", "done"),
            ev_completed("resp-allow-prefix-4"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "allow-prefix-second",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    wait_for_completion_without_approval(&test).await;

    let second_output = parse_result(
        &second_results
            .single_request()
            .function_call_output(call_id_second),
    );
    assert_eq!(second_output.exit_code.unwrap_or(0), 0);
    assert!(
        second_output.stdout.is_empty(),
        "unexpected stdout: {}",
        second_output.stdout
    );
    assert_eq!(
        fs::read_to_string(&allow_prefix_path)?,
        "",
        "unexpected file contents after second run"
    );

    Ok(())
}
