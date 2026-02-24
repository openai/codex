#![cfg(not(windows))]
//
// Running these tests with the patched zsh fork:
//
// The suite resolves the shared test-only zsh DotSlash file at
// `app-server/tests/suite/zsh` via DotSlash on first use, so `dotslash` and
// network access are required the first time the artifact is fetched.

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn turn_start_shell_zsh_fork_executes_command_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let Some(zsh_path) = find_test_zsh_path()? else {
        eprintln!("skipping zsh fork test: no zsh executable found");
        return Ok(());
    };
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());

    let responses = vec![create_shell_command_sse_response(
        vec!["echo".to_string(), "hi".to_string()],
        None,
        Some(5000),
        "call-zsh-fork",
    )?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "never",
        &BTreeMap::from([
            (Feature::ShellZshFork, true),
            (Feature::UnifiedExec, false),
            (Feature::ShellSnapshot, false),
        ]),
        &zsh_path,
    )?;

    let mut mcp = create_zsh_test_mcp_process(&codex_home, &workspace).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run echo hi".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(codex_protocol::openai_models::ReasoningEffort::Medium),
            summary: Some(codex_protocol::config_types::ReasoningSummary::Auto),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let started_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::CommandExecution { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        id,
        status,
        command,
        cwd,
        ..
    } = started_command_execution
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-zsh-fork");
    assert_eq!(status, CommandExecutionStatus::InProgress);
    assert!(command.starts_with(&zsh_path.display().to_string()));
    assert!(command.contains(" -lc 'echo hi'"));
    assert_eq!(cwd, workspace);

    mcp.interrupt_turn_and_wait_for_aborted(thread.id, turn.id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test]
async fn turn_start_shell_zsh_fork_exec_approval_decline_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let Some(zsh_path) = find_test_zsh_path()? else {
        eprintln!("skipping zsh fork decline test: no zsh executable found");
        return Ok(());
    };
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());

    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call-zsh-fork-decline",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::from([
            (Feature::ShellZshFork, true),
            (Feature::UnifiedExec, false),
            (Feature::ShellSnapshot, false),
        ]),
        &zsh_path,
    )?;

    let mut mcp = create_zsh_test_mcp_process(&codex_home, &workspace).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "call-zsh-fork-decline");
    assert_eq!(params.thread_id, thread.id);

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Decline,
        })?,
    )
    .await?;

    let completed_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::CommandExecution { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        id,
        status,
        exit_code,
        aggregated_output,
        ..
    } = completed_command_execution
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-zsh-fork-decline");
    assert_eq!(status, CommandExecutionStatus::Declined);
    assert!(exit_code.is_none());
    assert!(aggregated_output.is_none());

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_shell_zsh_fork_exec_approval_cancel_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let Some(zsh_path) = find_test_zsh_path()? else {
        eprintln!("skipping zsh fork cancel test: no zsh executable found");
        return Ok(());
    };
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());

    let responses = vec![create_shell_command_sse_response(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            "print(42)".to_string(),
        ],
        None,
        Some(5000),
        "call-zsh-fork-cancel",
    )?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::from([
            (Feature::ShellZshFork, true),
            (Feature::UnifiedExec, false),
            (Feature::ShellSnapshot, false),
        ]),
        &zsh_path,
    )?;

    let mut mcp = create_zsh_test_mcp_process(&codex_home, &workspace).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "call-zsh-fork-cancel");
    assert_eq!(params.thread_id, thread.id.clone());

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Cancel,
        })?,
    )
    .await?;

    let completed_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::CommandExecution { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution { id, status, .. } = completed_command_execution else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-zsh-fork-cancel");
    assert_eq!(status, CommandExecutionStatus::Declined);

    let completed_notif = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    Ok(())
}

#[tokio::test]
async fn turn_start_shell_zsh_fork_subcommand_decline_marks_parent_declined_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let Some(zsh_path) = find_test_zsh_path()? else {
        eprintln!("skipping zsh fork subcommand decline test: no zsh executable found");
        return Ok(());
    };
    if !supports_exec_wrapper_intercept(&zsh_path) {
        eprintln!(
            "skipping zsh fork subcommand decline test: zsh does not support EXEC_WRAPPER intercepts ({})",
            zsh_path.display()
        );
        return Ok(());
    }
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());
    let zsh_path_for_config = {
        // App-server config accepts only a zsh path, not extra argv. Use a
        // wrapper so this test can force `-df` and downgrade `-lc` to `-c`
        // to avoid rc/login-shell startup noise.
        let path = workspace.join("zsh-no-rc");
        std::fs::write(
            &path,
            format!(
                r#"#!/bin/sh
if [ "$1" = "-lc" ]; then
  shift
  set -- -c "$@"
fi
exec "{}" -df "$@"
"#,
                zsh_path.display()
            ),
        )?;
        let mut permissions = std::fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions)?;
        path
    };

    let tool_call_arguments = serde_json::to_string(&serde_json::json!({
        "command": "/usr/bin/true && /usr/bin/true",
        "workdir": serde_json::Value::Null,
        "timeout_ms": 5000
    }))?;
    let response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(
            "call-zsh-fork-subcommand-decline",
            "shell_command",
            &tool_call_arguments,
        ),
        responses::ev_completed("resp-1"),
    ]);
    let no_op_response = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_completed("resp-2"),
    ]);
    // Linux CI has occasionally issued a second `/responses` POST after the
    // subcommand-decline flow. This test is about approval/decline behavior in
    // the zsh fork, not exact model request count, so allow an extra request
    // and return a harmless no-op response if it arrives.
    let server =
        create_mock_responses_server_sequence_unchecked(vec![response, no_op_response]).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "on-request",
        &BTreeMap::from([
            (Feature::ShellZshFork, true),
            (Feature::UnifiedExec, false),
            (Feature::ShellSnapshot, false),
        ]),
        &zsh_path_for_config,
    )?;

    let mut mcp = create_zsh_test_mcp_process(&codex_home, &workspace).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run true true".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::OnRequest),
            sandbox_policy: Some(if cfg!(target_os = "linux") {
                // The zsh exec-bridge wrapper uses a Unix socket back to the parent
                // process. Linux restricted sandbox seccomp denies connect(2), so use
                // full access here; this test is validating zsh approval/decline
                // behavior, not Linux sandboxing.
                codex_app_server_protocol::SandboxPolicy::DangerFullAccess
            } else {
                codex_app_server_protocol::SandboxPolicy::ReadOnly {
                    access: codex_app_server_protocol::ReadOnlyAccess::FullAccess,
                }
            }),
            model: Some("mock-model".to_string()),
            effort: Some(codex_protocol::openai_models::ReasoningEffort::Medium),
            summary: Some(codex_protocol::config_types::ReasoningSummary::Auto),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let mut approval_ids = Vec::new();
    let mut saw_parent_approval = false;
    let target_decisions = [
        CommandExecutionApprovalDecision::Accept,
        CommandExecutionApprovalDecision::Cancel,
    ];
    let mut target_decision_index = 0;
    while target_decision_index < target_decisions.len() {
        let server_req = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_request_message(),
        )
        .await??;
        let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req
        else {
            panic!("expected CommandExecutionRequestApproval request");
        };
        assert_eq!(params.item_id, "call-zsh-fork-subcommand-decline");
        assert_eq!(params.thread_id, thread.id);
        let is_target_subcommand = params.command.as_deref() == Some("/usr/bin/true");
        if is_target_subcommand {
            approval_ids.push(
                params
                    .approval_id
                    .clone()
                    .expect("approval_id must be present for zsh subcommand approvals"),
            );
        }
        let decision = if is_target_subcommand {
            let decision = target_decisions[target_decision_index].clone();
            target_decision_index += 1;
            decision
        } else {
            let command = params
                .command
                .as_deref()
                .expect("approval command should be present");
            assert!(
                !saw_parent_approval,
                "unexpected extra non-target approval: {command}"
            );
            assert!(
                command.contains("zsh-no-rc"),
                "expected parent zsh wrapper approval, got: {command}"
            );
            assert!(
                command.contains("/usr/bin/true && /usr/bin/true"),
                "expected tool command in parent approval, got: {command}"
            );
            saw_parent_approval = true;
            CommandExecutionApprovalDecision::Accept
        };
        mcp.send_response(
            request_id,
            serde_json::to_value(CommandExecutionRequestApprovalResponse { decision })?,
        )
        .await?;
    }

    assert_eq!(approval_ids.len(), 2);
    assert_ne!(approval_ids[0], approval_ids[1]);
    let parent_completed_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::CommandExecution { id, .. } = &completed.item
                && id == "call-zsh-fork-subcommand-decline"
            {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await;

    match parent_completed_command_execution {
        Ok(Ok(parent_completed_command_execution)) => {
            let ThreadItem::CommandExecution {
                id,
                status,
                aggregated_output,
                ..
            } = parent_completed_command_execution
            else {
                unreachable!("loop ensures we break on parent command execution item");
            };
            assert_eq!(id, "call-zsh-fork-subcommand-decline");
            assert_eq!(status, CommandExecutionStatus::Declined);
            assert!(
                aggregated_output.is_none()
                    || aggregated_output == Some("exec command rejected by user".to_string())
            );

            mcp.interrupt_turn_and_wait_for_aborted(
                thread.id.clone(),
                turn.id.clone(),
                DEFAULT_READ_TIMEOUT,
            )
            .await?;
        }
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            // Some zsh builds abort the turn immediately after the rejected
            // subcommand without emitting a parent `item/completed`.
            let completed_notif = timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_notification_message("turn/completed"),
            )
            .await??;
            let completed: TurnCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .expect("turn/completed params must be present"),
            )?;
            assert_eq!(completed.thread_id, thread.id);
            assert_eq!(completed.turn.id, turn.id);
            assert_eq!(completed.turn.status, TurnStatus::Interrupted);
        }
    }

    Ok(())
}

async fn create_zsh_test_mcp_process(codex_home: &Path, zdotdir: &Path) -> Result<McpProcess> {
    let zdotdir = zdotdir.to_string_lossy().into_owned();
    McpProcess::new_with_env(codex_home, &[("ZDOTDIR", Some(zdotdir.as_str()))]).await
}

fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
    feature_flags: &BTreeMap<Feature, bool>,
    zsh_path: &Path,
) -> std::io::Result<()> {
    let mut features = BTreeMap::from([(Feature::RemoteModels, false)]);
    for (feature, enabled) in feature_flags {
        features.insert(*feature, *enabled);
    }
    let feature_entries = features
        .into_iter()
        .map(|(feature, enabled)| {
            let key = FEATURES
                .iter()
                .find(|spec| spec.id == feature)
                .map(|spec| spec.key)
                .unwrap_or_else(|| panic!("missing feature key for {feature:?}"));
            format!("{key} = {enabled}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"
zsh_path = "{zsh_path}"

model_provider = "mock_provider"

[features]
{feature_entries}

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            approval_policy = approval_policy,
            zsh_path = zsh_path.display()
        ),
    )
}

fn find_test_zsh_path() -> Result<Option<std::path::PathBuf>> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let dotslash_zsh = repo_root.join("codex-rs/app-server/tests/suite/zsh");
    if !dotslash_zsh.is_file() {
        eprintln!(
            "skipping zsh fork test: shared zsh DotSlash file not found at {}",
            dotslash_zsh.display()
        );
        return Ok(None);
    }
    match core_test_support::fetch_dotslash_file(&dotslash_zsh, None) {
        Ok(path) => return Ok(Some(path)),
        Err(error) => {
            eprintln!("failed to fetch vendored zsh via dotslash: {error:#}");
        }
    }

    Ok(None)
}

fn supports_exec_wrapper_intercept(zsh_path: &Path) -> bool {
    let status = std::process::Command::new(zsh_path)
        .arg("-fc")
        .arg("/usr/bin/true")
        .env("EXEC_WRAPPER", "/usr/bin/false")
        .status();
    match status {
        Ok(status) => !status.success(),
        Err(_) => false,
    }
}
