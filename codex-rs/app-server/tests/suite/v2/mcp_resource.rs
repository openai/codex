use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Router;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ProtocolVersion;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::ResourceContents;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_RESOURCE_URI: &str = "test://codex/resource";
const TEST_BLOB_RESOURCE_URI: &str = "test://codex/resource.bin";
const TEST_RESOURCE_BLOB: &str = "YmluYXJ5LXJlc291cmNl";
const TEST_RESOURCE_TEXT: &str = "Resource body from the MCP server.";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_returns_resource_contents() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let apps_server_url = format!("http://{addr}");

    let mcp_service = StreamableHttpService::new(
        move || Ok(ResourceAppsMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/api/codex/apps", mcp_service);
    let apps_server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let codex_home = TempDir::new()?;
    let responses_server_uri = responses_server.uri();
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

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            environment_id: None,
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let read_request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: thread.id,
            server: "codex_apps".to_string(),
            uri: TEST_RESOURCE_URI.to_string(),
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
            contents: vec![
                McpResourceContent::Text {
                    uri: TEST_RESOURCE_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: TEST_RESOURCE_TEXT.to_string(),
                    meta: None,
                },
                McpResourceContent::Blob {
                    uri: TEST_BLOB_RESOURCE_URI.to_string(),
                    mime_type: Some("application/octet-stream".to_string()),
                    blob: TEST_RESOURCE_BLOB.to_string(),
                    meta: None,
                },
            ],
        }
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test]
async fn mcp_resource_read_returns_error_for_unknown_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: "00000000-0000-4000-8000-000000000000".to_string(),
            server: "codex_apps".to_string(),
            uri: TEST_RESOURCE_URI.to_string(),
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

#[derive(Clone, Default)]
struct ResourceAppsMcpServer;

impl ServerHandler for ResourceAppsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_resources().build(),
            ..ServerInfo::default()
        }
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let uri = request.uri;
        if uri != TEST_RESOURCE_URI {
            return Err(rmcp::ErrorData::resource_not_found(
                format!("resource not found: {uri}"),
                None,
            ));
        }

        Ok(ReadResourceResult {
            contents: vec![
                ResourceContents::TextResourceContents {
                    uri: TEST_RESOURCE_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: TEST_RESOURCE_TEXT.to_string(),
                    meta: None,
                },
                ResourceContents::BlobResourceContents {
                    uri: TEST_BLOB_RESOURCE_URI.to_string(),
                    mime_type: Some("application/octet-stream".to_string()),
                    blob: TEST_RESOURCE_BLOB.to_string(),
                    meta: None,
                },
            ],
        })
    }
}
