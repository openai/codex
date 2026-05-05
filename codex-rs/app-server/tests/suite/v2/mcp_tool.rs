use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use axum::Router;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::BooleanSchema;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::PrimitiveSchema;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_SERVER_NAME: &str = "tool_server";
const TEST_TOOL_NAME: &str = "echo_tool";
const LARGE_RESPONSE_MESSAGE: &str = "large";
const ELICITATION_TRIGGER_MESSAGE: &str = "confirm";
const ELICITATION_MESSAGE: &str = "Allow this request?";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_tool_call_returns_tool_result() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.{TEST_SERVER_NAME}]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

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
    let thread_id = thread.id.clone();

    let tool_call_request_id = mcp
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: thread_id.clone(),
            server: TEST_SERVER_NAME.to_string(),
            tool: TEST_TOOL_NAME.to_string(),
            arguments: Some(json!({
                "message": "hello from app",
            })),
            meta: Some(json!({
                "source": "mcp-app",
            })),
        })
        .await?;
    let tool_call_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(tool_call_request_id)),
    )
    .await??;
    let response: McpServerToolCallResponse = to_response(tool_call_response)?;

    assert_eq!(response.content.len(), 1);
    assert_eq!(response.content[0].get("type"), Some(&json!("text")));
    assert_eq!(
        response.content[0].get("text"),
        Some(&json!("echo: hello from app"))
    );
    assert_eq!(
        response.structured_content,
        Some(json!({
            "echoed": "hello from app",
            "threadId": thread_id,
        }))
    );
    assert_eq!(response.is_error, Some(false));
    assert_eq!(
        response.meta,
        Some(json!({
            "calledBy": "mcp-app",
        }))
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test]
async fn mcp_server_tool_call_returns_error_for_unknown_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: "00000000-0000-4000-8000-000000000000".to_string(),
            server: TEST_SERVER_NAME.to_string(),
            tool: TEST_TOOL_NAME.to_string(),
            arguments: Some(json!({})),
            meta: None,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert!(
        error.error.message.contains("thread not found"),
        "expected thread-not-found error, got: {error:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_tool_call_round_trips_elicitation() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.{TEST_SERVER_NAME}]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let tool_call_request_id = mcp
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: thread.id.clone(),
            server: TEST_SERVER_NAME.to_string(),
            tool: TEST_TOOL_NAME.to_string(),
            arguments: Some(json!({
                "message": ELICITATION_TRIGGER_MESSAGE,
            })),
            meta: None,
        })
        .await?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::McpServerElicitationRequest { request_id, params } = server_req else {
        panic!("expected McpServerElicitationRequest request, got: {server_req:?}");
    };
    let requested_schema: McpElicitationSchema = serde_json::from_value(serde_json::to_value(
        ElicitationSchema::builder()
            .required_property("confirmed", PrimitiveSchema::Boolean(BooleanSchema::new()))
            .build()
            .map_err(anyhow::Error::msg)?,
    )?)?;
    assert_eq!(
        params,
        McpServerElicitationRequestParams {
            thread_id: thread.id,
            turn_id: None,
            server_name: TEST_SERVER_NAME.to_string(),
            request: McpServerElicitationRequest::Form {
                meta: None,
                message: ELICITATION_MESSAGE.to_string(),
                requested_schema,
            },
        }
    );

    mcp.send_response(
        request_id,
        serde_json::to_value(McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({
                "confirmed": true,
            })),
            meta: None,
        })?,
    )
    .await?;

    let tool_call_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(tool_call_request_id)),
    )
    .await??;
    let response: McpServerToolCallResponse = to_response(tool_call_response)?;
    assert_eq!(response.content.len(), 1);
    assert_eq!(response.content[0].get("type"), Some(&json!("text")));
    assert_eq!(response.content[0].get("text"), Some(&json!("accepted")));

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_call_completion_notification_contains_truncated_large_result() -> Result<()> {
    let call_id = "call-large-mcp";
    let namespace = format!("mcp__{TEST_SERVER_NAME}__");
    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                TEST_TOOL_NAME,
                &serde_json::to_string(&json!({
                    "message": LARGE_RESPONSE_MESSAGE,
                }))?,
            ),
            responses::ev_completed("resp-1"),
        ]),
        create_final_assistant_message_sse_response("done")?,
    ];
    let responses_server = create_mock_responses_server_sequence(responses).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1_000_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.{TEST_SERVER_NAME}]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

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
                text: "Call the large MCP tool".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { turn, .. } = to_response(turn_start_resp)?;

    let completed = wait_for_mcp_tool_call_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.turn_id, turn.id);

    let ThreadItem::McpToolCall {
        id,
        server,
        tool,
        status,
        result: Some(result),
        error,
        ..
    } = completed.item
    else {
        panic!("expected completed MCP tool call item");
    };
    assert_eq!(id, call_id);
    assert_eq!(server, TEST_SERVER_NAME);
    assert_eq!(tool, TEST_TOOL_NAME);
    assert_eq!(status, McpToolCallStatus::Completed);
    assert_eq!(error, None);
    assert_eq!(result.structured_content, None);
    assert_eq!(result.meta, None);
    assert_eq!(result.content.len(), 1);

    let text = result.content[0]
        .get("text")
        .and_then(serde_json::Value::as_str)
        .expect("truncated MCP event result should be represented as text content");
    assert!(text.contains("truncated"));
    assert!(text.len() < DEFAULT_OUTPUT_BYTES_CAP + 1024);

    let serialized_item = serde_json::to_string(&ThreadItem::McpToolCall {
        id,
        server,
        tool,
        status,
        arguments: json!({ "message": LARGE_RESPONSE_MESSAGE }),
        mcp_app_resource_uri: None,
        result: Some(result),
        error: None,
        duration_ms: None,
    })?;
    assert!(serialized_item.len() < DEFAULT_OUTPUT_BYTES_CAP * 2 + 2048);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[derive(Clone, Default)]
struct ToolAppsMcpServer;

impl ServerHandler for ToolAppsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string"
                }
            },
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;

        let mut tool = Tool::new(
            Cow::Borrowed(TEST_TOOL_NAME),
            Cow::Borrowed("Echo a message."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        assert_eq!(request.name.as_ref(), TEST_TOOL_NAME);
        let message = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let thread_id = context
            .meta
            .0
            .get("threadId")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        let mut meta = Meta::new();
        meta.0.insert("calledBy".to_string(), json!("mcp-app"));

        if message == LARGE_RESPONSE_MESSAGE {
            let large_text = "large-mcp-content-".repeat(DEFAULT_OUTPUT_BYTES_CAP / 8);
            let mut result = CallToolResult::structured(json!({
                "large": "structured-value-".repeat(DEFAULT_OUTPUT_BYTES_CAP / 8),
            }));
            result.content = vec![Content::text(large_text)];
            result.meta = Some(meta);
            return Ok(result);
        }

        if message == ELICITATION_TRIGGER_MESSAGE {
            let requested_schema = ElicitationSchema::builder()
                .required_property("confirmed", PrimitiveSchema::Boolean(BooleanSchema::new()))
                .build()
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            let result = context
                .peer
                .create_elicitation(CreateElicitationRequestParams::FormElicitationParams {
                    meta: None,
                    message: ELICITATION_MESSAGE.to_string(),
                    requested_schema,
                })
                .await
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            let output = match result.action {
                ElicitationAction::Accept => {
                    assert_eq!(
                        result.content,
                        Some(json!({
                            "confirmed": true,
                        }))
                    );
                    "accepted"
                }
                ElicitationAction::Decline => "declined",
                ElicitationAction::Cancel => "cancelled",
            };
            return Ok(CallToolResult::success(vec![Content::text(output)]));
        }

        let mut result = CallToolResult::structured(json!({
            "echoed": message,
            "threadId": thread_id,
        }));
        result.content = vec![Content::text(format!("echo: {message}"))];
        result.meta = Some(meta);
        Ok(result)
    }
}

async fn start_mcp_server() -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let mcp_service = StreamableHttpService::new(
        || Ok(ToolAppsMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/mcp", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn wait_for_mcp_tool_call_completed(
    mcp: &mut McpProcess,
    call_id: &str,
) -> Result<ItemCompletedNotification> {
    loop {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/completed"),
        )
        .await??;
        let Some(params) = notification.params else {
            continue;
        };
        let completed: ItemCompletedNotification = serde_json::from_value(params)?;
        if matches!(&completed.item, ThreadItem::McpToolCall { id, .. } if id == call_id) {
            return Ok(completed);
        }
    }
}
