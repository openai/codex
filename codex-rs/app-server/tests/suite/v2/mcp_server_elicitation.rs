use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
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
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
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
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::BooleanSchema;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::InitializeRequestParams;
use rmcp::model::InitializeResult;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::PrimitiveSchema;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::ServerRequest as McpServerRequest;
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

use super::connection_handling_websocket::WsClient;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::read_jsonrpc_message;
use super::connection_handling_websocket::read_notification_for_method;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_jsonrpc;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONNECTOR_ID: &str = "calendar";
const CONNECTOR_NAME: &str = "Calendar";
const TOOL_NAMESPACE: &str = "mcp__codex_apps__calendar";
const CALLABLE_TOOL_NAME: &str = "_confirm_action";
const TOOL_NAME: &str = "calendar_confirm_action";
const TOOL_CALL_ID: &str = "call-calendar-confirm";
const ELICITATION_MESSAGE: &str = "Allow this request?";
const OPENAI_FORM_MESSAGE: &str = "Select a template";
const IMAGE_DATA_URL: &str =
    "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";

#[derive(Clone, Copy)]
enum ElicitationScenario {
    StandardForm,
    OpenAiForm,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_server_form_elicitation_round_trip() -> Result<()> {
    let mut fixture = ElicitationRoundTripFixture::start(ElicitationScenario::StandardForm).await?;
    let (request_id, params) = fixture.read_elicitation().await?;
    let requested_schema: McpElicitationSchema = serde_json::from_value(serde_json::to_value(
        ElicitationSchema::builder()
            .required_property("confirmed", PrimitiveSchema::Boolean(BooleanSchema::new()))
            .build()
            .map_err(anyhow::Error::msg)?,
    )?)?;
    assert_eq!(
        params,
        McpServerElicitationRequestParams {
            thread_id: fixture.thread_id.clone(),
            turn_id: Some(fixture.turn_id.clone()),
            server_name: "codex_apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: None,
                message: ELICITATION_MESSAGE.to_string(),
                requested_schema,
            },
        }
    );

    fixture
        .accept(request_id.clone(), json!({ "confirmed": true }))
        .await?;
    fixture.finish(request_id, "accepted").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_server_openai_form_elicitation_round_trip() -> Result<()> {
    let mut fixture = ElicitationRoundTripFixture::start(ElicitationScenario::OpenAiForm).await?;
    let (request_id, params) = fixture.read_elicitation().await?;
    assert_eq!(
        params,
        McpServerElicitationRequestParams {
            thread_id: fixture.thread_id.clone(),
            turn_id: Some(fixture.turn_id.clone()),
            server_name: "codex_apps".to_string(),
            request: McpServerElicitationRequest::OpenAiForm {
                meta: None,
                message: OPENAI_FORM_MESSAGE.to_string(),
                requested_schema: json!({
                    "type": "object",
                    "properties": {
                        "template": {
                            "type": "openai/imagePicker",
                            "title": "Template",
                            "items": [{
                                "id": "monthly-review",
                                "title": "Monthly review",
                                "image": IMAGE_DATA_URL,
                            }],
                        },
                    },
                    "required": ["template"],
                }),
            },
        }
    );

    fixture
        .accept(request_id.clone(), json!({ "template": "monthly-review" }))
        .await?;
    fixture.finish(request_id, "accepted monthly-review").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn openai_form_capability_follows_the_turn_starting_connection() -> Result<()> {
    let (responses_server, response_mock, apps_server_url, apps_server_handle) =
        start_elicitation_services(ElicitationScenario::OpenAiForm).await?;
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

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;
    let mut supported_client = connect_websocket(bind_addr).await?;
    initialize_websocket_client(
        &mut supported_client,
        /*id*/ 1,
        "supported-client",
        /*supports_openai_form_elicitation*/ true,
    )
    .await?;

    send_request(
        &mut supported_client,
        "thread/start",
        /*id*/ 2,
        Some(serde_json::to_value(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await?;
    let ThreadStartResponse { thread, .. } =
        to_response(read_response_for_id(&mut supported_client, /*id*/ 2).await?)?;

    send_request(
        &mut supported_client,
        "turn/start",
        /*id*/ 3,
        Some(serde_json::to_value(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Warm up connectors.".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await?;
    let _: TurnStartResponse =
        to_response(read_response_for_id(&mut supported_client, /*id*/ 3).await?)?;
    let _: TurnCompletedNotification = serde_json::from_value(
        read_notification_for_method(&mut supported_client, "turn/completed")
            .await?
            .params
            .expect("turn/completed params"),
    )?;

    let mut unsupported_client = connect_websocket(bind_addr).await?;
    initialize_websocket_client(
        &mut unsupported_client,
        /*id*/ 4,
        "unsupported-client",
        /*supports_openai_form_elicitation*/ false,
    )
    .await?;
    send_request(
        &mut unsupported_client,
        "thread/resume",
        /*id*/ 5,
        Some(serde_json::to_value(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })?),
    )
    .await?;
    let _ = read_response_for_id(&mut unsupported_client, /*id*/ 5).await?;

    send_request(
        &mut supported_client,
        "turn/start",
        /*id*/ 6,
        Some(serde_json::to_value(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Use [$calendar](app://calendar) to run the calendar tool.".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await?;
    let TurnStartResponse { turn } =
        to_response(read_response_for_id(&mut supported_client, /*id*/ 6).await?)?;

    let (request_id, params) = loop {
        let JSONRPCMessage::Request(request) = read_jsonrpc_message(&mut supported_client).await?
        else {
            continue;
        };
        let request: ServerRequest = serde_json::from_value(serde_json::to_value(request)?)?;
        let ServerRequest::McpServerElicitationRequest { request_id, params } = request else {
            continue;
        };
        break (request_id, params);
    };
    assert_eq!(
        params.request,
        McpServerElicitationRequest::OpenAiForm {
            meta: None,
            message: OPENAI_FORM_MESSAGE.to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {
                    "template": {
                        "type": "openai/imagePicker",
                        "title": "Template",
                        "items": [{
                            "id": "monthly-review",
                            "title": "Monthly review",
                            "image": IMAGE_DATA_URL,
                        }],
                    },
                },
                "required": ["template"],
            }),
        }
    );
    send_jsonrpc(
        &mut supported_client,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: request_id,
            result: serde_json::to_value(McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Accept,
                content: Some(json!({ "template": "monthly-review" })),
                meta: None,
            })?,
        }),
    )
    .await?;

    let completed: TurnCompletedNotification = serde_json::from_value(
        read_notification_for_method(&mut supported_client, "turn/completed")
            .await?
            .params
            .expect("turn/completed params"),
    )?;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.id, turn.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(response_mock.requests().len(), 3);

    process.kill().await?;
    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

async fn initialize_websocket_client(
    client: &mut WsClient,
    id: i64,
    name: &str,
    supports_openai_form_elicitation: bool,
) -> Result<()> {
    send_request(
        client,
        "initialize",
        id,
        Some(serde_json::to_value(InitializeParams {
            client_info: ClientInfo {
                name: name.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                mcp_server_openai_form_elicitation: supports_openai_form_elicitation,
                ..Default::default()
            }),
        })?),
    )
    .await?;
    let _ = read_response_for_id(client, id).await?;
    Ok(())
}

async fn start_elicitation_services(
    scenario: ElicitationScenario,
) -> Result<(wiremock::MockServer, ResponseMock, String, JoinHandle<()>)> {
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
                responses::ev_function_call_with_namespace(
                    TOOL_CALL_ID,
                    TOOL_NAMESPACE,
                    CALLABLE_TOOL_NAME,
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
    let (apps_server_url, apps_server_handle) = start_apps_server(scenario).await?;
    Ok((
        responses_server,
        response_mock,
        apps_server_url,
        apps_server_handle,
    ))
}

struct ElicitationRoundTripFixture {
    mcp: TestAppServer,
    response_mock: ResponseMock,
    _responses_server: wiremock::MockServer,
    thread_id: String,
    turn_id: String,
    apps_server_handle: JoinHandle<()>,
}

impl ElicitationRoundTripFixture {
    async fn start(scenario: ElicitationScenario) -> Result<Self> {
        let (responses_server, response_mock, apps_server_url, apps_server_handle) =
            start_elicitation_services(scenario).await?;
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

        let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.initialize_with_capabilities(
                ClientInfo {
                    name: "codex-app-server-tests".to_string(),
                    title: None,
                    version: "0.1.0".to_string(),
                },
                Some(InitializeCapabilities {
                    experimental_api: true,
                    mcp_server_openai_form_elicitation: true,
                    ..Default::default()
                }),
            ),
        )
        .await??;

        let thread_start_id = mcp
            .send_thread_start_request_with_auto_env(ThreadStartParams {
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
                client_user_message_id: None,
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
                client_user_message_id: None,
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

        Ok(Self {
            mcp,
            response_mock,
            _responses_server: responses_server,
            thread_id: thread.id,
            turn_id: turn.id,
            apps_server_handle,
        })
    }

    async fn read_elicitation(&mut self) -> Result<(RequestId, McpServerElicitationRequestParams)> {
        let request = timeout(
            DEFAULT_READ_TIMEOUT,
            self.mcp.read_stream_until_request_message(),
        )
        .await??;
        let ServerRequest::McpServerElicitationRequest { request_id, params } = request else {
            panic!("expected McpServerElicitationRequest request, got: {request:?}");
        };
        Ok((request_id, params))
    }

    async fn accept(&mut self, request_id: RequestId, content: Value) -> Result<()> {
        self.mcp
            .send_response(
                request_id,
                serde_json::to_value(McpServerElicitationRequestResponse {
                    action: McpServerElicitationAction::Accept,
                    content: Some(content),
                    meta: None,
                })?,
            )
            .await
    }

    async fn finish(mut self, request_id: RequestId, expected_text: &str) -> Result<()> {
        let mut resolved = false;
        loop {
            let message = timeout(DEFAULT_READ_TIMEOUT, self.mcp.read_next_message()).await??;
            let JSONRPCMessage::Notification(notification) = message else {
                continue;
            };
            match notification.method.as_str() {
                "serverRequest/resolved" => {
                    let notification: ServerRequestResolvedNotification = serde_json::from_value(
                        notification
                            .params
                            .clone()
                            .expect("serverRequest/resolved params"),
                    )?;
                    assert_eq!(notification.thread_id, self.thread_id);
                    assert_eq!(notification.request_id, request_id);
                    resolved = true;
                }
                "turn/completed" => {
                    let notification: TurnCompletedNotification = serde_json::from_value(
                        notification.params.clone().expect("turn/completed params"),
                    )?;
                    assert!(
                        resolved,
                        "server request should resolve before turn completion"
                    );
                    assert_eq!(notification.thread_id, self.thread_id);
                    assert_eq!(notification.turn.id, self.turn_id);
                    assert_eq!(notification.turn.status, TurnStatus::Completed);
                    break;
                }
                _ => {}
            }
        }

        let requests = self.response_mock.requests();
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
        let payload = assert_regex_match(
            r#"(?s)^Wall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\n(.*)$"#,
            output,
        )
        .get(1)
        .expect("wall-time wrapped output should include payload")
        .as_str();
        assert_eq!(
            serde_json::from_str::<Value>(payload)?,
            json!([{ "type": "text", "text": expected_text }])
        );

        self.apps_server_handle.abort();
        let _ = self.apps_server_handle.await;
        Ok(())
    }
}

#[derive(Clone)]
struct AppsServerState {
    expected_bearer: String,
    expected_account_id: String,
}

#[derive(Clone)]
struct ElicitationAppsMcpServer {
    scenario: ElicitationScenario,
}

impl ServerHandler for ElicitationAppsMcpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        if matches!(self.scenario, ElicitationScenario::OpenAiForm) {
            assert_eq!(
                request
                    .capabilities
                    .extensions
                    .as_ref()
                    .and_then(|extensions| extensions.get("openai/form"))
                    .cloned()
                    .map(Value::Object),
                Some(json!({}))
            );
        }
        context.peer.set_peer_info(request);
        Ok(self.get_info())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_06_18)
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
        match self.scenario {
            ElicitationScenario::StandardForm => {
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
                assert_eq!(
                    result.content,
                    Some(json!({
                        "confirmed": true,
                    }))
                );
                let output = match result.action {
                    ElicitationAction::Accept => "accepted",
                    ElicitationAction::Decline => "declined",
                    ElicitationAction::Cancel => "cancelled",
                };
                Ok(CallToolResult::success(vec![Content::text(output)]))
            }
            ElicitationScenario::OpenAiForm => {
                let result = context
                    .peer
                    .send_request(McpServerRequest::CustomRequest(CustomRequest::new(
                        "openai/form",
                        Some(json!({
                            "message": OPENAI_FORM_MESSAGE,
                            "requestedSchema": {
                                "type": "object",
                                "properties": {
                                    "template": {
                                        "type": "openai/imagePicker",
                                        "title": "Template",
                                        "items": [{
                                            "id": "monthly-review",
                                            "title": "Monthly review",
                                            "image": IMAGE_DATA_URL,
                                        }],
                                    },
                                },
                                "required": ["template"],
                            },
                        })),
                    )))
                    .await
                    .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
                let result = match result {
                    rmcp::model::ClientResult::CustomResult(result) => result.0,
                    rmcp::model::ClientResult::CreateElicitationResult(result) => {
                        serde_json::to_value(result)
                            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?
                    }
                    result => {
                        return Err(rmcp::ErrorData::internal_error(
                            format!("unexpected OpenAI form response: {result:?}"),
                            None,
                        ));
                    }
                };
                assert_eq!(
                    result,
                    json!({
                        "action": "accept",
                        "content": {
                            "template": "monthly-review",
                        },
                    })
                );
                Ok(CallToolResult::success(vec![Content::text(
                    "accepted monthly-review",
                )]))
            }
        }
    }
}

async fn start_apps_server(scenario: ElicitationScenario) -> Result<(String, JoinHandle<()>)> {
    let state = Arc::new(AppsServerState {
        expected_bearer: "Bearer chatgpt-token".to_string(),
        expected_account_id: "account-123".to_string(),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let mcp_service = StreamableHttpService::new(
        move || Ok(ElicitationAppsMcpServer { scenario }),
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
        .nest_service("/api/codex/ps/mcp", mcp_service);

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
