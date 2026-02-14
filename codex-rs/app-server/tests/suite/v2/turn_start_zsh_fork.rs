#![cfg(not(windows))]

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
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
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::ffi::OsString;
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

    let Some(zsh_path) = find_test_zsh_path() else {
        eprintln!("skipping zsh fork test: no zsh executable found");
        return Ok(());
    };
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());

    let responses = vec![
        create_shell_command_sse_response(
            vec!["echo".to_string(), "hi".to_string()],
            None,
            Some(5000),
            "call-zsh-fork",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
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

    let sidecar_binary = match codex_utils_cargo_bin::cargo_bin("codex-zsh-sidecar") {
        Ok(path) => path,
        Err(err) => {
            eprintln!("skipping zsh fork test: could not locate codex-zsh-sidecar binary: {err}");
            return Ok(());
        }
    };
    let sidecar_dir = sidecar_binary.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "codex-zsh-sidecar path has no parent directory: {}",
            sidecar_binary.display()
        )
    })?;
    let path = prepend_path(sidecar_dir);
    let path_str = path.to_string_lossy().into_owned();

    let mut mcp =
        McpProcess::new_with_env(&codex_home, &[("PATH", Some(path_str.as_str()))]).await?;
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
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "run echo hi".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(codex_protocol::openai_models::ReasoningEffort::Medium),
            summary: Some(codex_core::protocol_config_types::ReasoningSummary::Auto),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;

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
    assert_eq!(id, "call-zsh-fork");
    assert_eq!(status, CommandExecutionStatus::Completed);
    assert_eq!(exit_code, Some(0));
    let output = aggregated_output.expect("aggregated output should be present");
    assert!(output.contains("hi"));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

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

    let Some(zsh_path) = find_test_zsh_path() else {
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

    let sidecar_binary = match codex_utils_cargo_bin::cargo_bin("codex-zsh-sidecar") {
        Ok(path) => path,
        Err(err) => {
            eprintln!(
                "skipping zsh fork decline test: could not locate codex-zsh-sidecar binary: {err}"
            );
            return Ok(());
        }
    };
    let sidecar_dir = sidecar_binary.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "codex-zsh-sidecar path has no parent directory: {}",
            sidecar_binary.display()
        )
    })?;
    let path = prepend_path(sidecar_dir);
    let path_str = path.to_string_lossy().into_owned();

    let mut mcp =
        McpProcess::new_with_env(&codex_home, &[("PATH", Some(path_str.as_str()))]).await?;
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

    let Some(zsh_path) = find_test_zsh_path() else {
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

    let sidecar_binary = match codex_utils_cargo_bin::cargo_bin("codex-zsh-sidecar") {
        Ok(path) => path,
        Err(err) => {
            eprintln!(
                "skipping zsh fork cancel test: could not locate codex-zsh-sidecar binary: {err}"
            );
            return Ok(());
        }
    };
    let sidecar_dir = sidecar_binary.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "codex-zsh-sidecar path has no parent directory: {}",
            sidecar_binary.display()
        )
    })?;
    let path = prepend_path(sidecar_dir);
    let path_str = path.to_string_lossy().into_owned();

    let mut mcp =
        McpProcess::new_with_env(&codex_home, &[("PATH", Some(path_str.as_str()))]).await?;
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
// TODO(owen): figure out a way to actually test this or remove
#[ignore = "requires patched zsh + command-shape that preserves compound subcommands"]
async fn turn_start_shell_zsh_fork_exec_approval_multiple_subcommands_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let Some(zsh_path) = find_test_zsh_path() else {
        eprintln!("skipping zsh fork multi-approval test: no zsh executable found");
        return Ok(());
    };
    eprintln!("using zsh path for zsh-fork test: {}", zsh_path.display());

    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "/usr/bin/true".to_string(),
                "&&".to_string(),
                "/usr/bin/true".to_string(),
            ],
            None,
            Some(5000),
            "call-zsh-fork-multi-approval",
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

    let sidecar_binary = match codex_utils_cargo_bin::cargo_bin("codex-zsh-sidecar") {
        Ok(path) => path,
        Err(err) => {
            eprintln!(
                "skipping zsh fork multi-approval test: could not locate codex-zsh-sidecar binary: {err}"
            );
            return Ok(());
        }
    };
    let sidecar_dir = sidecar_binary.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "codex-zsh-sidecar path has no parent directory: {}",
            sidecar_binary.display()
        )
    })?;
    let path = prepend_path(sidecar_dir);
    let path_str = path.to_string_lossy().into_owned();

    let mut mcp =
        McpProcess::new_with_env(&codex_home, &[("PATH", Some(path_str.as_str()))]).await?;
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
                text: "run two subcommands".to_string(),
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
    let ServerRequest::CommandExecutionRequestApproval {
        request_id: first_request_id,
        params: first_params,
    } = server_req
    else {
        panic!("expected first CommandExecutionRequestApproval request");
    };
    assert_eq!(first_params.item_id, "call-zsh-fork-multi-approval");

    mcp.send_response(
        first_request_id.clone(),
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
        })?,
    )
    .await?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval {
        request_id: second_request_id,
        params: second_params,
    } = server_req
    else {
        panic!("expected second CommandExecutionRequestApproval request");
    };
    assert_eq!(second_params.item_id, "call-zsh-fork-multi-approval");
    assert_ne!(first_request_id, second_request_id);

    mcp.send_response(
        second_request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
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
        ..
    } = completed_command_execution
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-zsh-fork-multi-approval");
    assert_eq!(status, CommandExecutionStatus::Completed);
    assert_eq!(exit_code, Some(0));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
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

fn find_test_zsh_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_TEST_ZSH_PATH") {
        let path = std::path::PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
        panic!(
            "CODEX_TEST_ZSH_PATH is set but is not a file: {}",
            path.display()
        );
    }

    for candidate in ["/bin/zsh", "/usr/bin/zsh"] {
        let path = Path::new(candidate);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }

    let shell = std::env::var_os("SHELL")?;
    let shell_path = std::path::PathBuf::from(shell);
    if shell_path
        .file_name()
        .is_some_and(|file_name| file_name == "zsh")
        && shell_path.is_file()
    {
        return Some(shell_path);
    }

    None
}

fn prepend_path(prefix: &Path) -> OsString {
    let mut path = OsString::from(prefix.as_os_str());
    if let Some(existing) = std::env::var_os("PATH") {
        path.push(":");
        path.push(existing);
    }
    path
}
