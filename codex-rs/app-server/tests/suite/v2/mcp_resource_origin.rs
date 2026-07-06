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
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadResumeParams;
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
const CONNECTOR_DESCRIPTION: &str = "Search catalog.";
const PRODUCT_SEARCH_MODEL_TOOL: &str = "_product_search";
const RESOURCE_URI: &str = "ui://widget/product-search.html";
const WRONG_RESOURCE_URI: &str = "ui://widget/wrong-product-search.html";

#[derive(Clone, Copy)]
struct AppFixture {
    connector_id: &'static str,
    link_id: &'static str,
    origin_call_id: &'static str,
    product_search_tool: &'static str,
    model_namespace: &'static str,
    html: &'static str,
}

const CATALOG_A: AppFixture = AppFixture {
    connector_id: "catalog-a",
    link_id: "link-catalog-a",
    origin_call_id: "catalog-a-search-call",
    product_search_tool: "catalog_a_product_search",
    model_namespace: "mcp__codex_apps__catalog_a",
    html: "<html>Catalog A</html>",
};

const CATALOG_B: AppFixture = AppFixture {
    connector_id: "catalog-b",
    link_id: "link-catalog-b",
    origin_call_id: "catalog-b-search-call",
    product_search_tool: "catalog_b_product_search",
    model_namespace: "mcp__codex_apps__catalog_b",
    html: "<html>Catalog B</html>",
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopened_mcp_resource_reads_are_scoped_to_the_originating_app() -> Result<()> {
    let mut scenario = start_origin_test_scenario(/*ephemeral*/ false).await?;

    let catalog_a_response = read_mcp_resource(
        &mut scenario.mcp,
        &scenario.thread_id,
        RESOURCE_URI,
        CATALOG_A.origin_call_id,
    )
    .await?;
    assert_eq!(
        catalog_a_response,
        expected_resource_response(CATALOG_A),
        "catalog A's call must render catalog A's HTML"
    );
    let catalog_b_response = read_mcp_resource(
        &mut scenario.mcp,
        &scenario.thread_id,
        RESOURCE_URI,
        CATALOG_B.origin_call_id,
    )
    .await?;
    assert_eq!(
        catalog_b_response,
        expected_resource_response(CATALOG_B),
        "catalog B's call must render catalog B's HTML"
    );

    assert_fetch_resource_calls(
        &scenario.recorded_calls,
        &[CATALOG_A, CATALOG_B],
        &scenario.thread_id,
    );

    scenario.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_ephemeral_origin_is_retained_and_identity_mismatches_fail_closed() -> Result<()>
{
    let mut scenario = start_origin_test_scenario(/*ephemeral*/ true).await?;

    let response = read_mcp_resource(
        &mut scenario.mcp,
        &scenario.thread_id,
        RESOURCE_URI,
        CATALOG_A.origin_call_id,
    )
    .await?;
    assert_eq!(response, expected_resource_response(CATALOG_A));

    let wrong_uri_error = read_mcp_resource_error(
        &mut scenario.mcp,
        &scenario.thread_id,
        WRONG_RESOURCE_URI,
        CATALOG_A.origin_call_id,
    )
    .await?;
    assert!(
        wrong_uri_error.error.message.contains("does not match"),
        "unexpected wrong-URI error: {wrong_uri_error:?}"
    );

    read_mcp_resource_error(
        &mut scenario.mcp,
        &scenario.thread_id,
        RESOURCE_URI,
        "unknown-product-search-call",
    )
    .await?;
    assert_fetch_resource_calls(&scenario.recorded_calls, &[CATALOG_A], &scenario.thread_id);

    scenario.stop().await;
    Ok(())
}

struct OriginTestScenario {
    _codex_home: TempDir,
    mcp: TestAppServer,
    thread_id: String,
    recorded_calls: Arc<Mutex<Vec<Value>>>,
    apps_server_handle: JoinHandle<()>,
}

impl OriginTestScenario {
    async fn stop(self) {
        self.apps_server_handle.abort();
        let _ = self.apps_server_handle.await;
    }
}

async fn start_origin_test_scenario(ephemeral: bool) -> Result<OriginTestScenario> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, recorded_calls, apps_server_handle) =
        start_plugin_runtime_mcp_server().await?;
    let (codex_home, mut mcp) =
        start_test_app_server(&apps_server_url, &responses_server.uri()).await?;

    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            product_search_response("resp-catalog-a-search", CATALOG_A),
            product_search_response("resp-catalog-b-search", CATALOG_B),
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
            ephemeral: Some(ephemeral),
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
                text: "Compare product results".to_string(),
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

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    if !ephemeral {
        drop(mcp);
        mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
        timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id: thread.id.clone(),
                ..Default::default()
            })
            .await?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
        )
        .await??;
    }

    assert_eq!(response_mock.requests().len(), 3);
    Ok(OriginTestScenario {
        _codex_home: codex_home,
        mcp,
        thread_id: thread.id,
        recorded_calls,
        apps_server_handle,
    })
}

fn product_search_response(response_id: &str, app: AppFixture) -> String {
    responses::sse(vec![
        responses::ev_response_created(response_id),
        responses::ev_function_call_with_namespace(
            app.origin_call_id,
            app.model_namespace,
            PRODUCT_SEARCH_MODEL_TOOL,
            &json!({ "query": "bed lamps" }).to_string(),
        ),
        responses::ev_completed(response_id),
    ])
}

async fn read_mcp_resource(
    mcp: &mut TestAppServer,
    thread_id: &str,
    uri: &str,
    origin_call_id: &str,
) -> Result<McpResourceReadResponse> {
    let request_id = send_read_request(mcp, thread_id, uri, origin_call_id).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_mcp_resource_error(
    mcp: &mut TestAppServer,
    thread_id: &str,
    uri: &str,
    origin_call_id: &str,
) -> Result<JSONRPCError> {
    let request_id = send_read_request(mcp, thread_id, uri, origin_call_id).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}

async fn send_read_request(
    mcp: &mut TestAppServer,
    thread_id: &str,
    uri: &str,
    origin_call_id: &str,
) -> Result<i64> {
    mcp.send_mcp_resource_read_request(McpResourceReadParams {
        thread_id: Some(thread_id.to_string()),
        server: "codex_apps".to_string(),
        uri: uri.to_string(),
        origin_call_id: Some(origin_call_id.to_string()),
    })
    .await
}

fn expected_resource_response(app: AppFixture) -> McpResourceReadResponse {
    McpResourceReadResponse {
        contents: vec![McpResourceContent::Text {
            uri: RESOURCE_URI.to_string(),
            mime_type: Some("text/html".to_string()),
            text: app.html.to_string(),
            meta: Some(json!({ "app": app.connector_id })),
        }],
    }
}

fn assert_fetch_resource_calls(
    recorded_calls: &Arc<Mutex<Vec<Value>>>,
    apps: &[AppFixture],
    thread_id: &str,
) {
    let actual = recorded_calls
        .lock()
        .expect("recorded tool calls lock poisoned")
        .iter()
        .filter(|call| call["name"] == "fetch_resource")
        .cloned()
        .collect::<Vec<_>>();
    let expected = apps
        .iter()
        .map(|app| {
            json!({
                "name": "fetch_resource",
                "arguments": { "uri": RESOURCE_URI },
                "meta": {
                    "_codex_apps": {
                        "resource_uri": format!("/{}/{}/fetch_resource", app.connector_id, app.link_id),
                        "contains_mcp_source": true,
                    },
                    "connector_name": app.connector_id,
                    "connector_description": CONNECTOR_DESCRIPTION,
                    "x-codex-turn-metadata": {
                        "mcp_request_meta": { "selected_connector_ids": [app.connector_id] },
                    },
                    "threadId": thread_id,
                },
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
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
        Ok(ListToolsResult {
            tools: [CATALOG_A, CATALOG_B]
                .into_iter()
                .map(product_search_tool)
                .collect(),
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
        let resource_route = meta
            .0
            .get("_codex_apps")
            .and_then(Value::as_object)
            .and_then(|codex_apps| codex_apps.get("resource_uri"))
            .and_then(Value::as_str)
            .map(str::to_string);
        self.calls
            .lock()
            .expect("recorded tool calls lock poisoned")
            .push(json!({
                "name": request.name,
                "arguments": request.arguments,
                "meta": meta,
            }));

        match request.name.as_ref() {
            name if [CATALOG_A.product_search_tool, CATALOG_B.product_search_tool]
                .contains(&name) =>
            {
                Ok(CallToolResult::structured(json!({ "products": [] })))
            }
            "fetch_resource" => fetch_resource_result(resource_route.as_deref()),
            name => Err(rmcp::ErrorData::invalid_params(
                format!("unknown tool: {name}"),
                None,
            )),
        }
    }
}

fn product_search_tool(app: AppFixture) -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(app.product_search_tool),
        Cow::Borrowed(CONNECTOR_DESCRIPTION),
        Arc::new(JsonObject::new()),
    );
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool.meta = Some(Meta(serde_json::Map::from_iter([
        ("connector_id".to_string(), json!(app.connector_id)),
        ("connector_name".to_string(), json!(app.connector_id)),
        (
            "connector_description".to_string(),
            json!(CONNECTOR_DESCRIPTION),
        ),
        ("link_id".to_string(), json!(app.link_id)),
        ("openai/outputTemplate".to_string(), json!(RESOURCE_URI)),
        (
            "_codex_apps".to_string(),
            json!({
                "resource_uri": format!(
                    "/{}/{}/{}",
                    app.connector_id, app.link_id, app.product_search_tool
                ),
                "contains_mcp_source": true,
            }),
        ),
    ])));
    tool
}

fn fetch_resource_result(resource_route: Option<&str>) -> Result<CallToolResult, rmcp::ErrorData> {
    let app = [CATALOG_A, CATALOG_B]
        .into_iter()
        .find(|app| {
            resource_route
                == Some(format!("/{}/{}/fetch_resource", app.connector_id, app.link_id).as_str())
        })
        .ok_or_else(|| {
            rmcp::ErrorData::invalid_params(
                format!("unknown fetch_resource route: {resource_route:?}"),
                None,
            )
        })?;
    Ok(CallToolResult::structured(json!({
        "contents": [{
            "uri": RESOURCE_URI,
            "mimeType": "text/html",
            "text": app.html,
            "meta": { "app": app.connector_id },
        }]
    })))
}
