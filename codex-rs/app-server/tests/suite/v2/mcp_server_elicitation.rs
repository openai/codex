use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_config::types::AuthCredentialsStoreMode;
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
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONNECTOR_ID: &str = "calendar";
const CONNECTOR_NAME: &str = "Calendar";
const TOOL_NAME: &str = "calendar_confirm_action";
const QUALIFIED_TOOL_NAME: &str = "mcp__codex_apps__calendar_confirm_action";
const TOOL_CALL_ID: &str = "call-calendar-confirm";
const ELICITATION_MESSAGE: &str = "Allow this request?";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_server_elicitation_round_trip() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let tool_call_arguments = serde_json::to_string(&json!({}))?;
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-0"),
                responses::ev_assistant_message("msg-0", "Warmup"),
                responses::ev_completed("resp-0"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    TOOL_CALL_ID,
                    QUALIFIED_TOOL_NAME,
                    &tool_call_arguments,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let (apps_server_url, apps_server_handle) = start_apps_server().await?;

    let codex_home = TempDir::new()?;
    write_config_toml(codex_home.path(), &responses_server.uri(), &apps_server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

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

    let warmup_turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Warm up connectors.".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let warmup_turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(warmup_turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(warmup_turn_start_resp)?;

    let warmup_completed = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let warmup_completed: TurnCompletedNotification = serde_json::from_value(
        warmup_completed
            .params
            .clone()
            .expect("warmup turn/completed params"),
    )?;
    assert_eq!(warmup_completed.thread_id, thread.id);
    assert_eq!(warmup_completed.turn.status, TurnStatus::Completed);

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Use [$calendar](app://calendar) to run the calendar tool.".to_string(),
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
    let TurnStartResponse { turn } = to_response(turn_start_resp)?;

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
            thread_id: thread.id.clone(),
            turn_id: Some(turn.id.clone()),
            server_name: "codex_apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: None,
                message: ELICITATION_MESSAGE.to_string(),
                requested_schema,
            },
        }
    );

    let resolved_request_id = request_id.clone();
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

    let mut saw_resolved = false;
    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };

        match notification.method.as_str() {
            "serverRequest/resolved" => {
                let resolved: ServerRequestResolvedNotification = serde_json::from_value(
                    notification
                        .params
                        .clone()
                        .expect("serverRequest/resolved params"),
                )?;
                assert_eq!(
                    resolved,
                    ServerRequestResolvedNotification {
                        thread_id: thread.id.clone(),
                        request_id: resolved_request_id.clone(),
                    }
                );
                saw_resolved = true;
            }
            "turn/completed" => {
                let completed: TurnCompletedNotification = serde_json::from_value(
                    notification.params.clone().expect("turn/completed params"),
                )?;
                assert!(saw_resolved, "serverRequest/resolved should arrive first");
                assert_eq!(completed.thread_id, thread.id);
                assert_eq!(completed.turn.id, turn.id);
                assert_eq!(completed.turn.status, TurnStatus::Completed);
                break;
            }
            _ => {}
        }
    }

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let function_call_output = requests[2].function_call_output(TOOL_CALL_ID);
    assert_eq!(
        function_call_output.get("type"),
        Some(&Value::String("function_call_output".to_string()))
    );
    assert_eq!(
        function_call_output.get("call_id"),
        Some(&Value::String(TOOL_CALL_ID.to_string()))
    );
    let output = function_call_output
        .get("output")
        .and_then(Value::as_str)
        .expect("function_call_output output should be a JSON string");
    assert_eq!(
        serde_json::from_str::<Value>(output)?,
        json!([{
            "type": "text",
            "text": "accepted"
        }])
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[derive(Clone)]
struct AppsServerState {
    expected_bearer: String,
    expected_account_id: String,
}

#[derive(Clone, Default)]
struct ElicitationAppsMcpServer;

impl ServerHandler for ElicitationAppsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::V_2025_06_18,
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
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;

        let mut tool = Tool::new(
            Cow::Borrowed(TOOL_NAME),
            Cow::Borrowed("Confirm a calendar action."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));

        let mut meta = Meta::new();
        meta.0
            .insert("connector_id".to_string(), json!(CONNECTOR_ID));
        meta.0
            .insert("connector_name".to_string(), json!(CONNECTOR_NAME));
        tool.meta = Some(meta);

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
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

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

async fn start_apps_server() -> Result<(String, JoinHandle<()>)> {
    let state = Arc::new(AppsServerState {
        expected_bearer: "Bearer chatgpt-token".to_string(),
        expected_account_id: "account-123".to_string(),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let mcp_service = StreamableHttpService::new(
        move || Ok(ElicitationAppsMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = Router::new()
        .route("/connectors/directory/list", get(list_directory_connectors))
        .route(
            "/connectors/directory/list_workspace",
            get(list_directory_connectors),
        )
        .with_state(state)
        .nest_service("/api/codex/apps", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn list_directory_connectors(
    State(state): State<Arc<AppsServerState>>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_bearer);
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_account_id);
    let external_logos_ok = uri
        .query()
        .is_some_and(|query| query.split('&').any(|pair| pair == "external_logos=true"));

    if !bearer_ok || !account_ok {
        Err(StatusCode::UNAUTHORIZED)
    } else if !external_logos_ok {
        Err(StatusCode::BAD_REQUEST)
    } else {
        Ok(Json(json!({
            "apps": [{
                "id": CONNECTOR_ID,
                "name": CONNECTOR_NAME,
                "description": "Calendar connector",
                "logo_url": null,
                "logo_url_dark": null,
                "distribution_channel": null,
                "branding": null,
                "app_metadata": null,
                "labels": null,
                "install_url": null,
                "is_accessible": false,
                "is_enabled": true
            }],
            "next_token": null
        })))
    }
}

fn write_config_toml(
    codex_home: &std::path::Path,
    responses_server_uri: &str,
    apps_server_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_mode = "read-only"

model_provider = "mock_provider"
chatgpt_base_url = "{apps_server_url}"
mcp_oauth_credentials_store = "file"

[features]
apps = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{responses_server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
