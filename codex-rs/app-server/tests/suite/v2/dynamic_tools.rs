use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Ensures dynamic tool specs are serialized into the model request payload.
#[tokio::test]
async fn thread_start_injects_dynamic_tools_into_model_requests() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Use a minimal JSON schema so we can assert the tool payload round-trips.
    let input_schema = json!({
        "type": "object",
        "properties": {
            "city": { "type": "string" }
        },
        "required": ["city"],
        "additionalProperties": false,
    });
    let dynamic_tool = DynamicToolSpec {
        name: "demo_tool".to_string(),
        description: "Demo dynamic tool".to_string(),
        input_schema: input_schema.clone(),
    };

    // Thread start injects dynamic tools into the thread's tool registry.
    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            dynamic_tools: Some(vec![dynamic_tool.clone()]),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    // Start a turn so a model request is issued.
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    // Inspect the captured model request to assert the tool spec made it through.
    let bodies = responses_bodies(&server).await?;
    let body = bodies
        .first()
        .context("expected at least one responses request")?;
    let tool = find_tool(body, &dynamic_tool.name)
        .context("expected dynamic tool to be injected into request")?;

    assert_eq!(
        tool.get("description"),
        Some(&Value::String(dynamic_tool.description.clone()))
    );
    assert_eq!(tool.get("parameters"), Some(&input_schema));

    Ok(())
}

/// Exercises the full dynamic tool call path (server request, client response, model output).
#[tokio::test]
async fn dynamic_tool_call_round_trip_sends_output_to_model() -> Result<()> {
    let call_id = "dyn-call-1";
    let tool_name = "demo_tool";
    let tool_args = json!({ "city": "Paris" });
    let tool_call_arguments = serde_json::to_string(&tool_args)?;

    // First response triggers a dynamic tool call, second closes the turn.
    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, tool_name, &tool_call_arguments),
            responses::ev_completed("resp-1"),
        ]),
        create_final_assistant_message_sse_response("Done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let dynamic_tool = DynamicToolSpec {
        name: tool_name.to_string(),
        description: "Demo dynamic tool".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"],
            "additionalProperties": false,
        }),
    };

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            dynamic_tools: Some(vec![dynamic_tool]),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    // Start a turn so the tool call is emitted.
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Run the tool".to_string(),
                text_elements: Vec::new(),
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

    // Read the tool call request from the app server.
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let (request_id, params) = match request {
        ServerRequest::DynamicToolCall { request_id, params } => (request_id, params),
        other => panic!("expected DynamicToolCall request, got {other:?}"),
    };

    let expected = DynamicToolCallParams {
        thread_id: thread.id,
        turn_id: turn.id,
        call_id: call_id.to_string(),
        tool: tool_name.to_string(),
        arguments: tool_args.clone(),
    };
    assert_eq!(params, expected);

    // Respond to the tool call so the model receives a function_call_output.
    let response = DynamicToolCallResponse {
        output: "dynamic-ok".to_string(),
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let bodies = responses_bodies(&server).await?;
    let output = bodies
        .iter()
        .find_map(|body| function_call_output_text(body, call_id))
        .context("expected function_call_output in follow-up request")?;
    assert_eq!(output, "dynamic-ok");

    Ok(())
}

async fn responses_bodies(server: &MockServer) -> Result<Vec<Value>> {
    let requests = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?;

    requests
        .into_iter()
        .filter(|req| req.url.path().ends_with("/responses"))
        .map(|req| {
            req.body_json::<Value>()
                .context("request body should be JSON")
        })
        .collect()
}

fn find_tool<'a>(body: &'a Value, name: &str) -> Option<&'a Value> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        })
}

fn function_call_output_text(body: &Value, call_id: &str) -> Option<String> {
    body.get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            })
        })
        .and_then(|item| item.get("output"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
