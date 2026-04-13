use anyhow::Result;
use anyhow::bail;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_request_permission_preset_sse_response;
use app_test_support::create_request_permissions_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PermissionPresetId as ApiPermissionPresetId;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use serde_json::Value;
use serde_json::json;
use tokio::time::timeout;
use wiremock::MockServer;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn permission_tools_are_hidden_without_permission_confirmations() -> Result<()> {
    let body = first_responses_request_body_for_permission_confirmations(false).await?;
    assert_permission_tool_exposure(
        &body, /*expect_permissions*/ false, /*expect_preset*/ false,
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn permission_tools_expose_both_with_permission_confirmations() -> Result<()> {
    let body = first_responses_request_body_for_permission_confirmations(true).await?;
    assert_permission_tool_exposure(
        &body, /*expect_permissions*/ true, /*expect_preset*/ true,
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_permissions_round_trips_when_client_supports_app_request() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![
        create_request_permissions_sse_response("call1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_with_permission_confirmations(&mut mcp, true).await?;

    let thread_id = start_thread_and_turn(&mut mcp, "pick a directory").await?;

    let server_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::PermissionsRequestApproval { request_id, params } = server_request else {
        bail!("expected PermissionsRequestApproval request");
    };
    assert_eq!(params.thread_id, thread_id);
    assert_eq!(params.item_id, "call1");
    assert_eq!(params.reason.as_deref(), Some("Select a workspace root"));
    let resolved_request_id = request_id.clone();

    mcp.send_response(
        request_id,
        json!({
            "permissions": {},
            "scope": "turn",
        }),
    )
    .await?;

    wait_for_server_request_resolved_then_turn_completed(&mut mcp, &thread_id, resolved_request_id)
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_permissions_auto_declines_without_app_request() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![
        create_request_permissions_sse_response("call1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "pick a directory".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { .. } = to_response(turn_start_resp)?;

    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Request(request) => {
                let server_request: ServerRequest = request.try_into()?;
                bail!("unexpected app-server request: {server_request:?}");
            }
            JSONRPCMessage::Notification(notification)
                if notification.method == "turn/completed" =>
            {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_permission_preset_round_trips_when_client_supports_app_request() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![
        create_request_permission_preset_sse_response("call1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_with_permission_confirmations(&mut mcp, true).await?;

    let thread_id = start_thread_and_turn(&mut mcp, "make this session full access").await?;

    let server_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::PermissionPresetRequestApproval { request_id, params } = server_request
    else {
        bail!("expected PermissionPresetRequestApproval request");
    };
    assert_eq!(params.thread_id, thread_id);
    assert_eq!(params.item_id, "call1");
    assert_eq!(params.preset, ApiPermissionPresetId::FullAccess);
    let resolved_request_id = request_id.clone();

    mcp.send_response(
        request_id,
        json!({
            "decision": "accepted",
            "preset": "full-access",
        }),
    )
    .await?;

    wait_for_server_request_resolved_then_turn_completed(&mut mcp, &thread_id, resolved_request_id)
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_permission_preset_auto_declines_without_app_request() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![
        create_request_permission_preset_sse_response("call1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "make this session full access".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { .. } = to_response(turn_start_resp)?;

    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Request(request) => {
                let server_request: ServerRequest = request.try_into()?;
                bail!("unexpected app-server request: {server_request:?}");
            }
            JSONRPCMessage::Notification(notification)
                if notification.method == "turn/completed" =>
            {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn initialize_with_permission_confirmations(
    mcp: &mut McpProcess,
    permission_confirmations: bool,
) -> Result<()> {
    let initialized = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_capabilities(
            ClientInfo {
                name: "codex_vscode".to_string(),
                title: Some("Codex VS Code Extension".to_string()),
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                permission_confirmations,
                opt_out_notification_methods: None,
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(_) = initialized else {
        bail!("expected initialize response, got {initialized:?}");
    };
    Ok(())
}

async fn first_responses_request_body_for_permission_confirmations(
    permission_confirmations: bool,
) -> Result<Value> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    if permission_confirmations {
        initialize_with_permission_confirmations(&mut mcp, true).await?;
    } else {
        timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    }

    start_thread_and_turn(&mut mcp, "Hello").await?;
    wait_for_turn_completed_without_server_request(&mut mcp).await?;

    first_responses_request_body(&server).await
}

async fn first_responses_request_body(server: &MockServer) -> Result<Value> {
    let requests = server
        .received_requests()
        .await
        .ok_or_else(|| anyhow::format_err!("failed to fetch received requests"))?;
    requests
        .into_iter()
        .find(|request| request.url.path().ends_with("/responses"))
        .ok_or_else(|| anyhow::format_err!("expected a /responses request"))?
        .body_json()
        .map_err(Into::into)
}

fn assert_permission_tool_exposure(body: &Value, expect_permissions: bool, expect_preset: bool) {
    let tool_names = body
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names.contains(&"request_permissions"),
        expect_permissions
    );
    assert_eq!(
        tool_names.contains(&"request_permission_preset"),
        expect_preset
    );

    let body_text = body.to_string();
    assert_eq!(
        body_text.contains("# request_permissions Tool"),
        expect_permissions
    );
    assert_eq!(
        body_text.contains("# request_permission_preset Tool"),
        expect_preset
    );
}

async fn start_thread_and_turn(mcp: &mut McpProcess, input: &str) -> Result<String> {
    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: input.to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { .. } = to_response(turn_start_resp)?;
    Ok(thread.id)
}

async fn wait_for_turn_completed_without_server_request(mcp: &mut McpProcess) -> Result<()> {
    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Request(request) => {
                let server_request: ServerRequest = request.try_into()?;
                bail!("unexpected app-server request: {server_request:?}");
            }
            JSONRPCMessage::Notification(notification)
                if notification.method == "turn/completed" =>
            {
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn wait_for_server_request_resolved_then_turn_completed(
    mcp: &mut McpProcess,
    thread_id: &str,
    request_id: RequestId,
) -> Result<()> {
    let mut saw_resolved = false;
    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        match notification.method.as_str() {
            "serverRequest/resolved" => {
                let Some(params) = notification.params.clone() else {
                    bail!("serverRequest/resolved notification missing params");
                };
                let resolved: ServerRequestResolvedNotification = serde_json::from_value(params)?;
                assert_eq!(resolved.thread_id, thread_id);
                assert_eq!(resolved.request_id, request_id);
                saw_resolved = true;
            }
            "turn/completed" => {
                assert!(saw_resolved, "serverRequest/resolved should arrive first");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features]
request_permissions_tool = true
"#
        ),
    )
}
