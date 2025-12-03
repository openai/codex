use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::create_shell_command_sse_response;
use app_test_support::format_with_current_shell;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::RemoveConversationSubscriptionResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::SendUserTurnResponse;
use codex_app_server_protocol::ServerRequest;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use pretty_assertions::assert_eq;
use std::env;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_codex_jsonrpc_conversation_flow() -> Result<()> {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let tmp = TempDir::new()?;
    // Temporary Codex home with config pointing at the mock server.
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    // Create a mock model server that immediately ends each turn.
    // Two turns are expected: initial session configure + one user message.
    let responses = vec![
        create_shell_command_sse_response(
            vec!["ls".to_string()],
            Some(&working_directory),
            Some(5000),
            "call1234",
        )?,
        create_final_assistant_message_sse_response("Enjoy your new git repo!")?,
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri())?;

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // 1) newConversation
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(working_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let new_conv_resp = to_response::<NewConversationResponse>(new_conv_resp)?;
    let NewConversationResponse {
        conversation_id,
        model,
        reasoning_effort: _,
        rollout_path: _,
    } = new_conv_resp;
    assert_eq!(model, "mock-model");

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;
    let AddConversationSubscriptionResponse { subscription_id } =
        to_response::<AddConversationSubscriptionResponse>(add_listener_resp)?;

    // 3) sendUserMessage (should trigger notifications; we only validate an OK response)
    let send_user_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "text".to_string(),
            }],
        })
        .await?;
    let send_user_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_user_id)),
    )
    .await??;
    let SendUserMessageResponse {} = to_response::<SendUserMessageResponse>(send_user_resp)?;

    // Verify the task_finished notification is received.
    // Note this also ensures that the final request to the server was made.
    let task_finished_notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    let serde_json::Value::Object(map) = task_finished_notification
        .params
        .expect("notification should have params")
    else {
        panic!("task_finished_notification should have params");
    };
    assert_eq!(
        map.get("conversationId")
            .expect("should have conversationId"),
        &serde_json::Value::String(conversation_id.to_string())
    );

    // 4) removeConversationListener
    let remove_listener_id = mcp
        .send_remove_conversation_listener_request(RemoveConversationListenerParams {
            subscription_id,
        })
        .await?;
    let remove_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(remove_listener_id)),
    )
    .await??;
    let RemoveConversationSubscriptionResponse {} = to_response(remove_listener_resp)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_send_user_turn_changes_approval_policy_behavior() -> Result<()> {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    // Mock server will request a python shell call for the first and second turn, then finish.
    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            Some(&working_directory),
            Some(5000),
            "call1",
        )?,
        create_final_assistant_message_sse_response("done 1")?,
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            Some(&working_directory),
            Some(5000),
            "call2",
        )?,
        create_final_assistant_message_sse_response("done 2")?,
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri())?;

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // 1) Start conversation with approval_policy=untrusted
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(working_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_conv_resp)?;

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let _: AddConversationSubscriptionResponse = to_response::<AddConversationSubscriptionResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
        )
        .await??,
    )?;

    // 3) sendUserMessage triggers a shell call; approval policy is Untrusted so we should get an elicitation
    let send_user_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "run python".to_string(),
            }],
        })
        .await?;
    let _send_user_resp: SendUserMessageResponse = to_response::<SendUserMessageResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_user_id)),
        )
        .await??,
    )?;

    // Expect an ExecCommandApproval request (elicitation)
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ExecCommandApproval { request_id, params } = request else {
        panic!("expected ExecCommandApproval request, got: {request:?}");
    };

    assert_eq!(
        ExecCommandApprovalParams {
            conversation_id,
            call_id: "call1".to_string(),
            command: format_with_current_shell("python3 -c 'print(42)'"),
            cwd: working_directory.clone(),
            reason: None,
            risk: None,
            parsed_cmd: vec![ParsedCommand::Unknown {
                cmd: "python3 -c 'print(42)'".to_string()
            }],
        },
        params
    );

    // Approve so the first turn can complete
    mcp.send_response(
        request_id,
        serde_json::json!({ "decision": codex_core::protocol::ReviewDecision::Approved }),
    )
    .await?;

    // Wait for first TaskComplete
    let _ = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    // 4) sendUserTurn with approval_policy=never should run without elicitation
    let send_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "run python again".to_string(),
            }],
            cwd: working_directory.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await?;
    // Acknowledge sendUserTurn
    let _send_turn_resp: SendUserTurnResponse = to_response::<SendUserTurnResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_turn_id)),
        )
        .await??,
    )?;

    // Ensure we do NOT receive an ExecCommandApproval request before the task completes.
    // If any Request is seen while waiting for task_complete, the helper will error and the test fails.
    let _ = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

// Helper: minimal config.toml pointing at mock provider.

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_send_user_turn_updates_sandbox_and_cwd_between_turns() -> Result<()> {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

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
        create_shell_command_sse_response(
            vec!["echo".to_string(), "first".to_string(), "turn".to_string()],
            None,
            Some(5000),
            "call-first",
        )?,
        create_final_assistant_message_sse_response("done first")?,
        create_shell_command_sse_response(
            vec!["echo".to_string(), "second".to_string(), "turn".to_string()],
            None,
            Some(5000),
            "call-second",
        )?,
        create_final_assistant_message_sse_response("done second")?,
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(first_cwd.to_string_lossy().into_owned()),
            approval_policy: Some(AskForApproval::Never),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id,
        model,
        ..
    } = to_response::<NewConversationResponse>(new_conv_resp)?;

    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;

    let first_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "first turn".to_string(),
            }],
            cwd: first_cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![first_cwd.clone()],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            model: model.clone(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let second_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "second turn".to_string(),
            }],
            cwd: second_cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: model.clone(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn_id)),
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
    assert_eq!(
        exec_begin.cwd, second_cwd,
        "exec turn should run from updated cwd"
    );
    let expected_command = format_with_current_shell("echo second turn");
    assert_eq!(
        exec_begin.command, expected_command,
        "exec turn should run expected command"
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"

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
