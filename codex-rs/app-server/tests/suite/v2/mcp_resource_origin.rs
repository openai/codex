use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Router;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::Implementation;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::ProtocolVersion;
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

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECTOR_ID: &str = "walmart";
const CONNECTOR_NAME: &str = "Walmart";
const LINK_ID: &str = "link_walmart";
const ORIGIN_CALL_ID: &str = "walmart-product-search-call";
const PRODUCT_SEARCH_TOOL: &str = "walmart_product_search";
const PRODUCT_SEARCH_MODEL_NAMESPACE: &str = "mcp__codex_apps__walmart";
const PRODUCT_SEARCH_MODEL_TOOL: &str = "_product_search";
const RESOURCE_URI: &str = "ui://widget/product-search.html";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_with_origin_calls_plugin_runtime_fetch_resource() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, recorded_calls, apps_server_handle) =
        start_plugin_runtime_mcp_server().await?;
    let (_codex_home, mut mcp) =
        start_test_app_server(&apps_server_url, &responses_server.uri()).await?;

    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-product-search"),
                responses::ev_function_call_with_namespace(
                    ORIGIN_CALL_ID,
                    PRODUCT_SEARCH_MODEL_NAMESPACE,
                    PRODUCT_SEARCH_MODEL_TOOL,
                    &json!({ "query": "bed lamps" }).to_string(),
                ),
                responses::ev_completed("resp-product-search"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-done"),
                responses::ev_assistant_message("msg-done", "Done"),
                responses::ev_completed("resp-done"),
            ]),
        ],
    )
    .await;

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Find bed lamps".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;

    let completed = wait_for_mcp_tool_call_completed(&mut mcp, ORIGIN_CALL_ID).await?;
    let ThreadItem::McpToolCall {
        app_context: Some(app_context),
        ..
    } = completed.item
    else {
        anyhow::bail!("originating MCP tool call should include app context");
    };
    assert_eq!(app_context.connector_id, CONNECTOR_ID);
    assert_eq!(app_context.link_id.as_deref(), Some(LINK_ID));
    assert_eq!(app_context.resource_uri.as_deref(), Some(RESOURCE_URI));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let read_request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: Some(thread.id.clone()),
            server: "codex_apps".to_string(),
            uri: RESOURCE_URI.to_string(),
            origin_call_id: Some(ORIGIN_CALL_ID.to_string()),
        })
        .await?;
    let read_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
    )
    .await??;

    assert_eq!(
        to_response::<McpResourceReadResponse>(read_response)?,
        McpResourceReadResponse {
            contents: vec![McpResourceContent::Text {
                uri: RESOURCE_URI.to_string(),
                mime_type: Some("text/html".to_string()),
                text: "<html>Walmart</html>".to_string(),
                meta: Some(json!({ "app": CONNECTOR_ID })),
            }],
        }
    );

    {
        let calls = recorded_calls
            .lock()
            .expect("recorded tool calls lock poisoned");
        let fetch_resource_call = calls
            .iter()
            .find(|call| call["name"] == "fetch_resource")
            .expect("resource read should call fetch_resource");
        assert_eq!(
            fetch_resource_call["arguments"],
            json!({ "uri": RESOURCE_URI })
        );
        assert_eq!(
            fetch_resource_call.pointer("/meta/_codex_apps"),
            Some(&json!({
                "resource_uri": format!("/{CONNECTOR_ID}/{LINK_ID}/fetch_resource"),
                "contains_mcp_source": true,
            }))
        );
        assert_eq!(
            fetch_resource_call
                .pointer("/meta/x-codex-turn-metadata/mcp_request_meta/selected_connector_ids"),
            Some(&json!([CONNECTOR_ID]))
        );
        assert_eq!(fetch_resource_call["meta"]["threadId"], thread.id);
    }

    assert_eq!(response_mock.requests().len(), 2);
    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

async fn start_test_app_server(
    apps_server_url: &str,
    responses_server_uri: &str,
) -> Result<(TempDir, TestAppServer)> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
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
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok((codex_home, mcp))
}

async fn start_plugin_runtime_mcp_server()
-> Result<(String, Arc<Mutex<Vec<Value>>>, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let apps_server_url = format!("http://{addr}");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let server_calls = Arc::clone(&calls);
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(PluginRuntimeMcpServer {
                calls: Arc::clone(&server_calls),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/api/codex/ps/mcp", mcp_service);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((apps_server_url, calls, handle))
}

#[derive(Clone)]
struct PluginRuntimeMcpServer {
    calls: Arc<Mutex<Vec<Value>>>,
}

impl ServerHandler for PluginRuntimeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2025_06_18)
            .with_server_info(Implementation::new("plugin-runtime", "1.0.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"],
            "additionalProperties": false
        }))
        .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        let mut tool = Tool::new(
            Cow::Borrowed(PRODUCT_SEARCH_TOOL),
            Cow::Borrowed("Search Walmart products."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool.meta = Some(Meta(serde_json::Map::from_iter([
            ("connector_id".to_string(), json!(CONNECTOR_ID)),
            ("connector_name".to_string(), json!(CONNECTOR_NAME)),
            (
                "connector_description".to_string(),
                json!("Search Walmart products."),
            ),
            ("link_id".to_string(), json!(LINK_ID)),
            ("openai/outputTemplate".to_string(), json!(RESOURCE_URI)),
            (
                "_codex_apps".to_string(),
                json!({
                    "resource_uri": format!(
                        "/{CONNECTOR_ID}/{LINK_ID}/{PRODUCT_SEARCH_TOOL}"
                    ),
                    "contains_mcp_source": true,
                }),
            ),
        ])));

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
        let mut meta = context.meta;
        meta.0.remove("progressToken");
        self.calls
            .lock()
            .expect("recorded tool calls lock poisoned")
            .push(json!({
                "name": request.name,
                "arguments": request.arguments,
                "meta": meta,
            }));

        match request.name.as_ref() {
            PRODUCT_SEARCH_TOOL => Ok(CallToolResult::structured(json!({ "products": [] }))),
            "fetch_resource" => {
                let mut result = CallToolResult::structured(json!({
                    "contents": [{
                        "uri": RESOURCE_URI,
                        "mimeType": "text/html",
                        "text": "<html>Walmart</html>",
                        "meta": { "app": CONNECTOR_ID },
                    }]
                }));
                result.content = vec![Content::text("Fetched Walmart resource")];
                Ok(result)
            }
            name => Err(rmcp::ErrorData::invalid_params(
                format!("unknown tool: {name}"),
                None,
            )),
        }
    }
}

async fn wait_for_mcp_tool_call_completed(
    mcp: &mut TestAppServer,
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
