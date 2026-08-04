use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use axum::Router;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_utils_path_uri::PathUri;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use core_test_support::responses;
use futures::SinkExt;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::BooleanSchema;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ContentBlock;
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::InitializeRequestParams;
use rmcp::model::InitializeResult;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::MetaObject;
use rmcp::model::PrimitiveSchemaDefinition;
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
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use super::exec_server_test_support::accept_exec_server_environment;
use super::exec_server_test_support::read_exec_server_json;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const AUTO_COMPACT_LIMIT: i64 = 1024;
const LARGE_OUTPUT_AUTO_COMPACT_LIMIT: i64 = 1_000_000;
pub(super) const TEST_SERVER_NAME: &str = "tool_server";
pub(super) const TEST_TOOL_NAME: &str = "echo_tool";
const LARGE_RESPONSE_MESSAGE: &str = "large";
const ELICITATION_TRIGGER_MESSAGE: &str = "confirm";
const ELICITATION_MESSAGE: &str = "Allow this request?";
const URL_ELICITATION_TRIGGER_MESSAGE: &str = "auth";
const URL_ELICITATION_MESSAGE: &str = "Sign in to GitHub to continue.";
const URL_ELICITATION_URL: &str = "https://github.example/login/device";
const LATE_ENVIRONMENT_ID: &str = "late-environment";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_tool_call_returns_tool_result() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_id = thread.id.clone();
    let response: McpServerToolCallResponse = mcp
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: McpServerToolCallParams {
                thread_id: thread_id.clone(),
                server: TEST_SERVER_NAME.to_string(),
                tool: TEST_TOOL_NAME.to_string(),
                arguments: Some(json!({
                    "message": "hello from app",
                })),
                meta: Some(json!({
                    "source": "mcp-app",
                })),
            },
        })
        .await?;

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
            "clientCapabilities": {
                "extensions": {},
            },
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_tool_call_uses_session_client_extensions() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let app_ui = json!({
        "mimeTypes": [
            "text/html;profile=mcp-app",
            "text/x-dil;profile=mcp-app",
        ],
        "futureField": {"preserved": true},
    });
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    mcp.initialize_with_capabilities(
        ClientInfo {
            name: "codex_test".to_string(),
            title: None,
            version: "0.1.0".to_string(),
        },
        Some(InitializeCapabilities {
            experimental_api: true,
            request_attestation: false,
            mcp_server_openai_form_elicitation: true,
            opt_out_notification_methods: None,
            extensions: Some(std::collections::HashMap::from([(
                "io.modelcontextprotocol/ui".to_string(),
                app_ui.clone(),
            )])),
        }),
    )
    .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_id = thread.id;

    let response: McpServerToolCallResponse = mcp
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: McpServerToolCallParams {
                thread_id: thread_id.clone(),
                server: TEST_SERVER_NAME.to_string(),
                tool: TEST_TOOL_NAME.to_string(),
                arguments: Some(json!({"message": "capabilities"})),
                meta: None,
            },
        })
        .await?;

    assert_eq!(
        response.structured_content,
        Some(json!({
            "echoed": "capabilities",
            "threadId": thread_id,
            "clientCapabilities": {
                "extensions": {
                    "openai/form": {},
                    "io.modelcontextprotocol/ui": app_ui,
                }
            },
        }))
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_mcp_tool_call_uses_session_client_extensions() -> Result<()> {
    let call_id = "call-session-capabilities";
    let namespace = format!("mcp__{TEST_SERVER_NAME}");
    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-capabilities"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                TEST_TOOL_NAME,
                &serde_json::to_string(&json!({"message": "capabilities"}))?,
            ),
            responses::ev_completed("resp-capabilities"),
        ]),
        create_final_assistant_message_sse_response("done")?,
    ];
    let responses_server = create_mock_responses_server_sequence(responses).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let app_ui = json!({
        "mimeTypes": [
            "text/html;profile=mcp-app",
            "text/x-dil;profile=mcp-app",
        ],
        "futureField": {"preserved": true},
    });
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    mcp.initialize_with_capabilities(
        ClientInfo {
            name: "codex_test".to_string(),
            title: None,
            version: "0.1.0".to_string(),
        },
        Some(InitializeCapabilities {
            experimental_api: true,
            request_attestation: false,
            mcp_server_openai_form_elicitation: true,
            opt_out_notification_methods: None,
            extensions: Some(std::collections::HashMap::from([(
                "io.modelcontextprotocol/ui".to_string(),
                app_ui.clone(),
            )])),
        }),
    )
    .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    mcp.request::<TurnStartResponse>(|request_id| ClientRequest::TurnStart {
        request_id,
        params: TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Call the MCP tool".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        },
    })
    .await?;

    let completed = wait_for_mcp_tool_call_completed(&mut mcp, call_id).await?;
    let ThreadItem::McpToolCall {
        result: Some(result),
        ..
    } = completed.item
    else {
        panic!("expected completed MCP tool call item");
    };
    assert_eq!(
        result.structured_content,
        Some(json!({
            "echoed": "capabilities",
            "threadId": thread.id,
            "clientCapabilities": {
                "extensions": {
                    "openai/form": {},
                    "io.modelcontextprotocol/ui": app_ui,
                }
            },
        }))
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;
    Ok(())
}

#[tokio::test]
async fn mcp_server_tool_call_returns_error_for_unknown_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

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
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;

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
            .required_property(
                "confirmed",
                PrimitiveSchemaDefinition::Boolean(BooleanSchema::new()),
            )
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

    let response: McpServerToolCallResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_response(tool_call_request_id),
    )
    .await??;
    assert_eq!(response.content.len(), 1);
    assert_eq!(response.content[0].get("type"), Some(&json!("text")));
    assert_eq!(response.content[0].get("text"), Some(&json!("accepted")));

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_elicitation_survives_environment_runtime_refresh() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let exec_listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", exec_listener.local_addr()?);
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .enable_feature(Feature::DeferredExecutor)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        // This test adds and refreshes an explicitly selected runtime environment.
        .without_auto_env()
        .build_initialized()
        .await?;
    let add_environment_id = mcp
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": LATE_ENVIRONMENT_ID,
                "execServerUrl": exec_server_url,
                "connectTimeoutMs": 10_000,
            })),
        )
        .await?;
    let _: EnvironmentAddResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(add_environment_id)).await??;

    let capability_root = TempDir::new()?;
    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
            environments: Some(vec![TurnEnvironmentParams {
                environment_id: LATE_ENVIRONMENT_ID.to_string(),
                cwd: codex_utils_absolute_path::AbsolutePathBuf::try_from(
                    capability_root.path().to_path_buf(),
                )?
                .into(),
                runtime_workspace_roots: None,
            }]),
            selected_capability_roots: Some(vec![SelectedCapabilityRoot {
                id: "late-plugin@1".to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id: LATE_ENVIRONMENT_ID.to_string(),
                    path: PathUri::from_host_native_path(capability_root.path())?,
                },
            }]),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_start_id)).await??;

    let tool_call_request_id = mcp
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: thread.id.clone(),
            server: TEST_SERVER_NAME.to_string(),
            tool: TEST_TOOL_NAME.to_string(),
            arguments: Some(json!({"message": ELICITATION_TRIGGER_MESSAGE})),
            meta: None,
        })
        .await?;
    let server_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::McpServerElicitationRequest { request_id, .. } = server_request else {
        panic!("expected MCP elicitation request, got: {server_request:?}");
    };

    let (filesystem_request_tx, filesystem_request_rx) = oneshot::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let exec_server_handle = tokio::spawn(serve_environment_until_shutdown(
        exec_listener,
        filesystem_request_tx,
        shutdown_rx,
    ));
    let mut filesystem_request_rx = filesystem_request_rx;
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let status_request_id = mcp
                .send_raw_request("mcpServerStatus/list", Some(json!({"threadId": thread.id})))
                .await?;
            mcp.read_stream_until_response_message(RequestId::Integer(status_request_id))
                .await?;
            if filesystem_request_rx.try_recv().is_ok() {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;

    mcp.send_response(
        request_id,
        serde_json::to_value(McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({"confirmed": true})),
            meta: None,
        })?,
    )
    .await?;
    let response: McpServerToolCallResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_response(tool_call_request_id),
    )
    .await??;
    assert_eq!(response.content[0].get("text"), Some(&json!("accepted")));

    let _ = shutdown_tx.send(());
    exec_server_handle.await??;
    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_tool_call_forwards_url_elicitation() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;

    let tool_call_request_id = mcp
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: thread.id.clone(),
            server: TEST_SERVER_NAME.to_string(),
            tool: TEST_TOOL_NAME.to_string(),
            arguments: Some(json!({
                "message": URL_ELICITATION_TRIGGER_MESSAGE,
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
    assert_eq!(
        params,
        McpServerElicitationRequestParams {
            thread_id: thread.id,
            turn_id: None,
            server_name: TEST_SERVER_NAME.to_string(),
            request: McpServerElicitationRequest::Url {
                meta: None,
                message: URL_ELICITATION_MESSAGE.to_string(),
                url: URL_ELICITATION_URL.to_string(),
                elicitation_id: "github-auth-123".to_string(),
            },
        }
    );

    mcp.send_response(
        request_id,
        serde_json::to_value(McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: None,
            meta: None,
        })?,
    )
    .await?;

    let response: McpServerToolCallResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_response(tool_call_request_id),
    )
    .await??;
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
    let namespace = format!("mcp__{TEST_SERVER_NAME}");
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
    mcp_tool_config(
        &responses_server.uri(),
        &mcp_server_url,
        LARGE_OUTPUT_AUTO_COMPACT_LIMIT,
    )
    .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn, .. } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "Call the large MCP tool".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

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
        app_context: None,
        mcp_app_resource_uri: None,
        plugin_id: None,
        read_only_hint: None,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_call_hint_survives_mid_call_thread_read_and_resume() -> Result<()> {
    let call_id = "call-mid-flight-mcp";
    let namespace = format!("mcp__{TEST_SERVER_NAME}");
    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-mid-flight"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                TEST_TOOL_NAME,
                &serde_json::to_string(&json!({
                    "message": ELICITATION_TRIGGER_MESSAGE,
                }))?,
            ),
            responses::ev_completed("resp-mid-flight"),
        ]),
        create_final_assistant_message_sse_response("done")?,
    ];
    let responses_server = create_mock_responses_server_sequence(responses).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    mcp_tool_config(&responses_server.uri(), &mcp_server_url, AUTO_COMPACT_LIMIT)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn, .. } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "Call the MCP tool".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    let server_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::McpServerElicitationRequest { request_id, .. } = server_request else {
        panic!("expected MCP elicitation while the tool call is in progress");
    };

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse {
        thread: read_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;
    assert_eq!(read_thread.id, thread.id);

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    let expected_item = ThreadItem::McpToolCall {
        id: call_id.to_string(),
        server: TEST_SERVER_NAME.to_string(),
        tool: TEST_TOOL_NAME.to_string(),
        status: McpToolCallStatus::InProgress,
        arguments: json!({ "message": ELICITATION_TRIGGER_MESSAGE }),
        app_context: None,
        mcp_app_resource_uri: None,
        plugin_id: None,
        read_only_hint: Some(true),
        result: None,
        error: None,
        duration_ms: None,
    };
    let resumed_item = resumed_thread
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .find(|item| matches!(item, ThreadItem::McpToolCall { id, .. } if id == call_id))
        .expect("resumed thread should include the in-progress MCP tool call");
    assert_eq!(resumed_item, &expected_item);

    mcp.send_response(
        request_id,
        serde_json::to_value(McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({ "confirmed": true })),
            meta: None,
        })?,
    )
    .await?;

    let completed = wait_for_mcp_tool_call_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.turn_id, turn.id);
    let ThreadItem::McpToolCall {
        status,
        read_only_hint,
        ..
    } = &completed.item
    else {
        panic!("expected the completed MCP tool call item");
    };
    assert_eq!(status, &McpToolCallStatus::Completed);
    assert_eq!(read_only_hint, &Some(true));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let completed_read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse {
        thread: completed_read,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(completed_read_id)).await??;
    let persisted_item = completed_read
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .find(|item| matches!(item, ThreadItem::McpToolCall { id, .. } if id == call_id))
        .expect("completed thread history should include the persisted MCP tool call");
    assert_eq!(persisted_item, &completed.item);

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[derive(Clone, Default)]
struct ToolAppsMcpServer;

impl ServerHandler for ToolAppsMcpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        context.peer.set_peer_info(request);
        Ok(self.get_info())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
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

        Ok(ListToolsResult::with_all_items(vec![tool]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
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
            .0
            .get("threadId")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let client_capabilities = context.peer.peer_info().map(|request| {
            json!({
                "extensions": request.capabilities.extensions.clone().unwrap_or_default(),
            })
        });

        let mut meta = MetaObject::new();
        meta.0.insert("calledBy".to_string(), json!("mcp-app"));

        if message == LARGE_RESPONSE_MESSAGE {
            let large_text = "large-mcp-content-".repeat(DEFAULT_OUTPUT_BYTES_CAP / 8);
            let mut result = CallToolResult::structured(json!({
                "large": "structured-value-".repeat(DEFAULT_OUTPUT_BYTES_CAP / 8),
            }));
            result.content = vec![ContentBlock::text(large_text)];
            result.meta = Some(meta);
            return Ok(result.into());
        }

        if message == ELICITATION_TRIGGER_MESSAGE {
            let requested_schema = ElicitationSchema::builder()
                .required_property(
                    "confirmed",
                    PrimitiveSchemaDefinition::Boolean(BooleanSchema::new()),
                )
                .build()
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            let result = context
                .peer
                .create_elicitation(ElicitRequestParams::FormElicitationParams {
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
                _ => {
                    return Err(rmcp::ErrorData::invalid_params(
                        "unsupported MCP elicitation action",
                        None,
                    ));
                }
            };
            return Ok(CallToolResult::success(vec![ContentBlock::text(output)]).into());
        }

        if message == URL_ELICITATION_TRIGGER_MESSAGE {
            let result = context
                .peer
                .create_elicitation(ElicitRequestParams::UrlElicitationParams {
                    meta: None,
                    message: URL_ELICITATION_MESSAGE.to_string(),
                    url: URL_ELICITATION_URL.to_string(),
                    elicitation_id: "github-auth-123".to_string(),
                })
                .await
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            let output = match result.action {
                ElicitationAction::Accept => {
                    assert_eq!(result.content, Some(json!({})));
                    "accepted"
                }
                ElicitationAction::Decline => "declined",
                ElicitationAction::Cancel => "cancelled",
                _ => {
                    return Err(rmcp::ErrorData::invalid_params(
                        "unsupported MCP elicitation action",
                        None,
                    ));
                }
            };
            return Ok(CallToolResult::success(vec![ContentBlock::text(output)]).into());
        }

        let mut structured_content = json!({
            "echoed": message,
            "threadId": thread_id,
        });
        if let Some(client_capabilities) = client_capabilities {
            structured_content["clientCapabilities"] = client_capabilities;
        }
        let mut result = CallToolResult::structured(structured_content);
        result.content = vec![ContentBlock::text(format!("echo: {message}"))];
        result.meta = Some(meta);
        Ok(result.into())
    }
}

pub(super) async fn start_mcp_server() -> Result<(String, JoinHandle<()>)> {
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

async fn serve_environment_until_shutdown(
    listener: TcpListener,
    filesystem_request_tx: oneshot::Sender<()>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<()> {
    let mut websocket = accept_exec_server_environment(
        listener,
        json!({"shell": {"name": "zsh", "path": "/bin/zsh"}}),
    )
    .await?;

    let mut filesystem_request_tx = Some(filesystem_request_tx);
    loop {
        let request = tokio::select! {
            request = read_exec_server_json(&mut websocket) => request?,
            _ = &mut shutdown_rx => return Ok(()),
        };
        if request["method"]
            .as_str()
            .is_some_and(|method| method.starts_with("fs/"))
            && let Some(tx) = filesystem_request_tx.take()
        {
            let _ = tx.send(());
        }
        if request.get("id").is_some() {
            websocket
                .send(Message::Text(
                    json!({
                        "id": request["id"],
                        "error": {"code": -32004, "message": "not found"},
                    })
                    .to_string()
                    .into(),
                ))
                .await?;
        }
    }
}

async fn wait_for_mcp_tool_call_completed(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed: ItemCompletedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("item/completed"),
        )
        .await??;
        if matches!(&completed.item, ThreadItem::McpToolCall { id, .. } if id == call_id) {
            return Ok(completed);
        }
    }
}

fn mcp_tool_config(
    server_uri: &str,
    mcp_server_url: &str,
    auto_compact_limit: i64,
) -> MockResponsesConfig {
    MockResponsesConfig::new(server_uri)
        .with_root_config(&format!(
            "compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = {auto_compact_limit}"
        ))
        .with_provider_config("supports_websockets = false")
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_server_url}/mcp\""
        ))
}
