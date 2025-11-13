use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::create_mock_chat_completions_server_unchecked;
use app_test_support::create_shell_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn turn_start_emits_notifications_and_accepts_model_override() -> Result<()> {
    // Provide a mock server and config so model wiring is valid.
    // Three Codex turns hit the mock model (session start + two turn/start calls).
    let responses = vec![
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
    ];
    let server = create_mock_chat_completions_server_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread (v2) and capture its id.
    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(thread_resp)?;

    // Start a turn with only input and thread_id set (no overrides).
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    assert!(!turn.id.is_empty());

    // Expect a turn/started notification.
    let notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    let started: TurnStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    assert_eq!(
        started.turn.status,
        codex_app_server_protocol::TurnStatus::InProgress
    );

    // Send a second turn that exercises the overrides path: change the model.
    let turn_req2 = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Second".to_string(),
            }],
            model: Some("mock-model-override".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_resp2: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req2)),
    )
    .await??;
    let TurnStartResponse { turn: turn2 } = to_response::<TurnStartResponse>(turn_resp2)?;
    assert!(!turn2.id.is_empty());
    // Ensure the second turn has a different id than the first.
    assert_ne!(turn.id, turn2.id);

    // Expect a second turn/started notification as well.
    let _notif2: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    // And we should ultimately get a task_complete without having to add a
    // legacy conversation listener explicitly (auto-attached by thread/start).
    let _task_complete: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_accepts_local_image_input() -> Result<()> {
    // Two Codex turns hit the mock model (session start + turn/start).
    let responses = vec![
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
    ];
    // Use the unchecked variant because the request payload includes a LocalImage
    // which the strict matcher does not currently cover.
    let server = create_mock_chat_completions_server_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(thread_resp)?;

    let image_path = codex_home.path().join("image.png");
    // No need to actually write the file; we just exercise the input path.

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::LocalImage { path: image_path }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    assert!(!turn.id.is_empty());

    // This test only validates that turn/start responds and returns a turn.
    Ok(())
}

#[tokio::test]
async fn turn_start_exec_approval_toggle_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().to_path_buf();

    // Mock server: first turn requests a shell call (elicitation), then completes.
    // Second turn same, but we'll set approval_policy=never to avoid elicitation.
    let responses = vec![
        create_shell_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call1",
        )?,
        create_final_assistant_message_sse_response("done 1")?,
        create_shell_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call2",
        )?,
        create_final_assistant_message_sse_response("done 2")?,
    ];
    let server = create_mock_chat_completions_server(responses).await;
    // Default approval is untrusted to force elicitation on first turn.
    create_config_toml(codex_home.as_path(), &server.uri(), "untrusted")?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // thread/start
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(start_resp)?;

    // turn/start — expect ExecCommandApproval request from server
    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
            }],
            ..Default::default()
        })
        .await?;
    // Acknowledge RPC
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn_id)),
    )
    .await??;

    // Receive elicitation
    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ExecCommandApproval { request_id, params } = server_req else {
        panic!("expected ExecCommandApproval request");
    };
    assert_eq!(params.call_id, "call1");
    assert_eq!(
        params.parsed_cmd,
        vec![ParsedCommand::Unknown {
            cmd: "python3 -c 'print(42)'".to_string()
        }]
    );

    // Approve and wait for task completion
    mcp.send_response(
        request_id,
        serde_json::json!({ "decision": codex_core::protocol::ReviewDecision::Approved }),
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    // Second turn with approval_policy=never should not elicit approval
    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python again".to_string(),
            }],
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn_id)),
    )
    .await??;

    // Ensure we do NOT receive an ExecCommandApproval request before task completes
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_updates_sandbox_and_cwd_between_turns_v2() -> Result<()> {
    // When returning Result from a test, pass an Ok(()) to the skip macro
    // so the early return type matches. The no-arg form returns unit.
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir(&workspace_root)?;
    let first_cwd = workspace_root.join("turn1");
    let second_cwd = workspace_root.join("turn2");
    std::fs::create_dir(&first_cwd)?;
    std::fs::create_dir(&second_cwd)?;

    let responses = vec![
        create_shell_sse_response(
            vec![
                "bash".to_string(),
                "-lc".to_string(),
                "echo first turn".to_string(),
            ],
            None,
            Some(5000),
            "call-first",
        )?,
        create_final_assistant_message_sse_response("done first")?,
        create_shell_sse_response(
            vec![
                "bash".to_string(),
                "-lc".to_string(),
                "echo second turn".to_string(),
            ],
            None,
            Some(5000),
            "call-second",
        )?,
        create_final_assistant_message_sse_response("done second")?,
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri(), "untrusted")?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // thread/start
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(start_resp)?;

    // first turn with workspace-write sandbox and first_cwd
    let first_turn = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "first turn".to_string(),
            }],
            cwd: Some(first_cwd.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![first_cwd.clone()],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    // second turn with workspace-write and second_cwd, ensure exec begins in second_cwd
    let second_turn = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "second turn".to_string(),
            }],
            cwd: Some(second_cwd.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn)),
    )
    .await??;

    let exec_begin_notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/exec_command_begin"),
    )
    .await??;
    let params = exec_begin_notification
        .params
        .clone()
        .expect("exec_command_begin params");
    let event: Event = serde_json::from_value(params).expect("deserialize exec begin event");
    let exec_begin = match event.msg {
        EventMsg::ExecCommandBegin(exec_begin) => exec_begin,
        other => panic!("expected ExecCommandBegin event, got {other:?}"),
    };
    assert_eq!(exec_begin.cwd, second_cwd);
    assert_eq!(
        exec_begin.command,
        vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo second turn".to_string()
        ]
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
