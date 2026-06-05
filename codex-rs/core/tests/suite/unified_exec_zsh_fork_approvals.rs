use anyhow::Context;
use anyhow::Result;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::types::ApprovalsReviewer;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use core_test_support::zsh_fork::build_unified_exec_zsh_fork_test;
use core_test_support::zsh_fork::restrictive_workspace_write_profile;
use core_test_support::zsh_fork::zsh_fork_runtime;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use toml_edit::Key as TomlKey;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_preserves_denied_reads() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let denied_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let denied_path = denied_dir.path().join("secret.env");
    let secret = "unified-exec-zsh-fork-denied-read-secret";
    fs::write(&denied_path, format!("{secret}\n"))?;
    let permission_profile = denied_read_permission_profile(&denied_path)?;
    assert!(
        permission_profile
            .file_system_sandbox_policy()
            .has_denied_read_restrictions(),
        "test must exercise a permission profile with denied reads"
    );

    let approval_policy = AskForApproval::OnRequest;
    let command = format!("cat {denied_path:?}");
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork denied-read approval test",
        approval_policy,
        permission_profile,
        move |_home| {},
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval-denied-read";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-denied-read",
        call_id,
        &command,
        "attempt a denied read for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec denied read through zsh fork",
        approval_policy,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;
    wait_for_completion_without_approval(&test).await;

    let result = command_result(&results, call_id);
    assert_ne!(
        result.exit_code.unwrap_or(0),
        0,
        "denied-read command should stay sandboxed after parent approval"
    );
    assert!(
        !result.stdout.contains(secret),
        "denied-read command unexpectedly printed the secret: {}",
        result.stdout
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_escalates_intercepted_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-parent-approval.txt");
    let command = format!("printf hi > {outside_path:?}");

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork parent approval test",
        approval_policy,
        permission_profile,
        move |_home| {
            let _ = fs::remove_file(&outside_path_for_hook);
        },
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-parent-approval",
        call_id,
        &command,
        "write outside the workspace for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec through zsh fork",
        approval_policy,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;
    wait_for_completion_without_approval(&test).await;

    let result = command_result(&results, call_id);
    assert_eq!(
        result.exit_code.unwrap_or(0),
        0,
        "approved unified exec zsh-fork command should complete: {}",
        result.stdout
    );
    let contents = fs::read_to_string(&outside_path)
        .with_context(|| format!("read {}", outside_path.display()))?;
    assert_eq!(
        contents, "hi",
        "approved parent sandbox override should allow zsh-fork shell redirection to write outside the workspace"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_keeps_explicit_prompt_rule() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-explicit-prompt-rule.txt");
    let command = format!("touch {outside_path:?}");
    let rules = r#"prefix_rule(pattern=["touch"], decision="prompt")"#.to_string();

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork prompt rule approval test",
        approval_policy,
        permission_profile,
        move |home| {
            let _ = fs::remove_file(&outside_path_for_hook);
            let rules_dir = home.join("rules");
            fs::create_dir_all(&rules_dir).unwrap();
            fs::write(rules_dir.join("default.rules"), &rules).unwrap();
        },
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval-explicit-prompt-rule";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-prompt-rule",
        call_id,
        &command,
        "write outside the workspace for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec prompt rule through zsh fork",
        approval_policy,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;

    let approval_event = wait_for_event_with_timeout(
        &test.codex,
        |event| {
            matches!(
                event,
                EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
            )
        },
        Duration::from_secs(10),
    )
    .await;
    let EventMsg::ExecApprovalRequest(inner_approval) = approval_event else {
        panic!("expected explicit prompt rule approval before completion");
    };
    assert!(
        inner_approval
            .command
            .iter()
            .any(|arg| arg.ends_with("/touch"))
            && inner_approval
                .command
                .iter()
                .any(|arg| arg == outside_path.to_string_lossy().as_ref()),
        "expected explicit prompt rule approval for intercepted touch, got: {:?}",
        inner_approval.command
    );

    approve_exec(&test, inner_approval.effective_approval_id()).await?;
    wait_for_completion(&test).await;

    let result = command_result(&results, call_id);
    assert_eq!(
        result.exit_code.unwrap_or(0),
        0,
        "approved unified exec zsh-fork prompt-rule command should complete: {}",
        result.stdout
    );
    assert!(
        outside_path.exists(),
        "approved intercepted touch should create the out-of-workspace file"
    );

    Ok(())
}

struct CommandResult {
    exit_code: Option<i64>,
    stdout: String,
}

async fn build_unified_exec_zsh_fork_test_or_skip<F>(
    test_name: &str,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    pre_build_hook: F,
) -> Result<Option<(MockServer, TestCodex)>>
where
    F: FnOnce(&Path) + Send + 'static,
{
    let Some(runtime) = zsh_fork_runtime(test_name)? else {
        return Ok(None);
    };

    let server = start_mock_server().await;
    let test = build_unified_exec_zsh_fork_test(
        &server,
        runtime,
        approval_policy,
        permission_profile,
        pre_build_hook,
    )
    .await?;
    Ok(Some((server, test)))
}

fn denied_read_permission_profile(denied_path: &Path) -> Result<PermissionProfile> {
    let denied_path_key = TomlKey::new(denied_path.to_string_lossy().into_owned());
    permission_profile_from_toml(&format!(
        r#"
[filesystem]
"/" = "read"
":project_roots" = "write"
{denied_path_key} = "deny"

[network]
enabled = false
"#
    ))
}

fn permission_profile_from_toml(profile: &str) -> Result<PermissionProfile> {
    let profile = toml::from_str::<PermissionProfileToml>(profile)
        .context("test permission profile should deserialize")?;
    let filesystem = profile
        .filesystem
        .as_ref()
        .context("test permission profile should include filesystem entries")?;
    let entries = filesystem
        .entries
        .iter()
        .map(|(path, permission)| {
            let FilesystemPermissionToml::Access(access) = permission else {
                anyhow::bail!("unexpected scoped filesystem permission in test profile: {path}");
            };
            let path = match path.as_str() {
                "/" => FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                ":project_roots" => FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                _ if *access == FileSystemAccessMode::Deny => FileSystemPath::GlobPattern {
                    pattern: path.clone(),
                },
                _ => anyhow::bail!("unexpected filesystem entry in test profile: {path}"),
            };
            Ok(FileSystemSandboxEntry {
                path,
                access: *access,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(entries);
    file_system_sandbox_policy.glob_scan_max_depth = filesystem.glob_scan_max_depth;
    let network_sandbox_policy = match profile.network.as_ref().and_then(|network| network.enabled)
    {
        Some(true) => NetworkSandboxPolicy::Enabled,
        Some(false) | None => NetworkSandboxPolicy::Restricted,
    };

    Ok(PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        network_sandbox_policy,
    ))
}

async fn mount_unified_exec_command(
    server: &MockServer,
    response_prefix: &str,
    call_id: &str,
    command: &str,
    justification: &str,
) -> Result<ResponseMock> {
    let first_response_id = format!("resp-{response_prefix}-1");
    let second_response_id = format!("resp-{response_prefix}-2");
    let message_id = format!("msg-{response_prefix}-1");
    let event = exec_command_event(
        call_id,
        command,
        Some(30_000),
        SandboxPermissions::RequireEscalated,
        justification,
    )?;
    let _ = mount_sse_once(
        server,
        sse(vec![
            ev_response_created(&first_response_id),
            event,
            ev_completed(&first_response_id),
        ]),
    )
    .await;
    let results = mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message(&message_id, "done"),
            ev_completed(&second_response_id),
        ]),
    )
    .await;
    Ok(results)
}

async fn submit_turn_with_session_permissions(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) = turn_permission_fields(
        test.session_configured.permission_profile.clone(),
        test.cwd.path(),
    );
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                cwd: Some(test.cwd.path().to_path_buf()),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    Ok(())
}

async fn approve_expected_exec(test: &TestCodex, expected_command: &str) -> Result<()> {
    let approval = expect_exec_approval(test, expected_command).await;
    approve_exec(test, approval.effective_approval_id()).await
}

async fn approve_exec(test: &TestCodex, approval_id: String) -> Result<()> {
    test.codex
        .submit(Op::ExecApproval {
            id: approval_id,
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    Ok(())
}

fn command_result(results: &ResponseMock, call_id: &str) -> CommandResult {
    parse_result(&results.single_request().function_call_output(call_id))
}

fn exec_command_event(
    call_id: &str,
    cmd: &str,
    yield_time_ms: Option<u64>,
    sandbox_permissions: SandboxPermissions,
    justification: &str,
) -> Result<Value> {
    let mut args = json!({
        "cmd": cmd.to_string(),
    });
    if let Some(yield_time_ms) = yield_time_ms {
        args["yield_time_ms"] = json!(yield_time_ms);
    }
    if sandbox_permissions.requests_sandbox_override() {
        args["sandbox_permissions"] = json!(sandbox_permissions);
        args["justification"] = json!(justification);
    }
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
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
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
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
            assert_eq!(
                last_arg, expected_command,
                "approval request should be for the parent unified-exec command"
            );
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(event) => {
            panic!("unexpected approval request: {:?}", event.command)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}
