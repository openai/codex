use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Router;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ListResourcesResult;
use rmcp::model::Meta;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::RawResource;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceContents;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
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
const TEST_RESOURCE_URI: &str = "test://codex/resource";
const TEST_BLOB_RESOURCE_URI: &str = "test://codex/resource.bin";
const TEST_RESOURCE_BLOB: &str = "YmluYXJ5LXJlc291cmNl";
const TEST_RESOURCE_TEXT: &str = "Resource body from the MCP server.";
const SKILL_NAME: &str = "demo-plugin:deploy";
const RAW_SKILL_DESCRIPTION: &str = "Deploy\nthrough <hosted> backend.";
const SKILL_DESCRIPTION: &str = "Deploy through &lt;hosted&gt; backend.";
const SKILL_RESOURCE_URI: &str = "skill://plugin_demo/deploy";
const SKILL_MAIN_PROMPT_URI: &str = "skill://plugin_demo/deploy/SKILL.md";
const SKILL_MARKER: &str = "BACKEND_SKILL_BODY_MARKER";
const SKILL_CONTENTS: &str = "---\nname: deploy\ndescription: Deploy through the hosted backend.\n---\n\n# Deploy\n\nBACKEND_SKILL_BODY_MARKER\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_resources_support_backend_skills_without_an_environment() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, apps_server_handle) = start_resource_apps_mcp_server().await?;

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

[skills]
include_instructions = true

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

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            environments: Some(Vec::new()),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;
    let thread_id = thread.id;

    let response_mock = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-backend-skill"),
            responses::ev_assistant_message("msg-backend-skill", "Done"),
            responses::ev_completed("resp-backend-skill"),
        ]),
    )
    .await;
    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: format!("Use ${SKILL_NAME}"),
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

    let request = response_mock.single_request();
    let developer_messages = request.message_input_texts("developer");
    let catalog_line = format!("- {SKILL_NAME}: {SKILL_DESCRIPTION} (file: {SKILL_RESOURCE_URI})");
    assert_eq!(
        1,
        developer_messages
            .iter()
            .filter(|text| text.contains(&catalog_line))
            .count()
    );
    assert!(
        developer_messages
            .iter()
            .all(|text| !text.contains("ignored-plugin:ignored"))
    );
    let skill_fragments = request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(1, skill_fragments.len());
    assert!(skill_fragments[0].contains(&format!("<name>{SKILL_NAME}</name>")));
    assert!(skill_fragments[0].contains(SKILL_MARKER));

    let read_request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: Some(thread_id),
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
        expected_resource_read_response()
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_returns_resource_contents_without_thread() -> Result<()> {
    let (apps_server_url, apps_server_handle) = start_resource_apps_mcp_server().await?;

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{apps_server_url}"
mcp_oauth_credentials_store = "file"

[features]
apps = true
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

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let read_request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: None,
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
        expected_resource_read_response()
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test]
async fn mcp_resource_read_returns_error_for_unknown_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;
    // This negative-path test does not need the stdio subprocess; keeping it
    // in-process avoids child-process teardown timing in nextest leak detection.
    let client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config),
        cli_overrides: Vec::new(),
        loader_overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        thread_config_loader: Arc::new(codex_config::NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: None,
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;

    let response = client
        .request(ClientRequest::McpResourceRead {
            request_id: RequestId::Integer(1),
            params: McpResourceReadParams {
                thread_id: Some("00000000-0000-4000-8000-000000000000".to_string()),
                server: "codex_apps".to_string(),
                uri: TEST_RESOURCE_URI.to_string(),
            },
        })
        .await;
    client.shutdown().await?;

    let error = match response? {
        Ok(result) => anyhow::bail!("expected thread-not-found error, got response: {result:?}"),
        Err(error) => error,
    };
    assert!(
        error.message.contains("thread not found"),
        "expected thread-not-found error, got: {error:?}"
    );

    Ok(())
}

async fn start_resource_apps_mcp_server() -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let apps_server_url = format!("http://{addr}");

    let mcp_service = StreamableHttpService::new(
        move || Ok(ResourceAppsMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/api/codex/ps/mcp", mcp_service);
    let apps_server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((apps_server_url, apps_server_handle))
}

fn expected_resource_read_response() -> McpResourceReadResponse {
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
}

#[derive(Clone, Default)]
struct ResourceAppsMcpServer;

impl ServerHandler for ResourceAppsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_resources().build())
            .with_protocol_version(ProtocolVersion::V_2025_06_18)
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        let cursor = request.and_then(|request| request.cursor);
        if cursor.is_none() {
            return Ok(ListResourcesResult {
                resources: vec![skill_resource(
                    "skill://plugin_ignored/ignored",
                    "plugin_ignored/ignored",
                    "Not an MCP skill resource.",
                    "text/plain",
                    "ignored-plugin",
                    "ignored",
                )],
                next_cursor: Some("skills-page".to_string()),
                meta: None,
            });
        }
        if cursor.as_deref() == Some("failing-page") {
            return Err(rmcp::ErrorData::internal_error(
                "simulated later-page failure",
                /*data*/ None,
            ));
        }
        if cursor.as_deref() != Some("skills-page") {
            return Err(rmcp::ErrorData::invalid_params(
                "unexpected resources/list cursor",
                /*data*/ None,
            ));
        }

        Ok(ListResourcesResult {
            resources: vec![skill_resource(
                SKILL_RESOURCE_URI,
                "plugin_demo/deploy",
                RAW_SKILL_DESCRIPTION,
                "mcp/skill",
                "demo-plugin",
                "deploy",
            )],
            next_cursor: Some("failing-page".to_string()),
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let uri = request.uri;
        if uri == SKILL_MAIN_PROMPT_URI {
            return Ok(ReadResourceResult::new(vec![
                ResourceContents::TextResourceContents {
                    uri: SKILL_MAIN_PROMPT_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: SKILL_CONTENTS.to_string(),
                    meta: None,
                },
            ]));
        }
        if uri != TEST_RESOURCE_URI {
            return Err(rmcp::ErrorData::resource_not_found(
                format!("resource not found: {uri}"),
                None,
            ));
        }

        Ok(ReadResourceResult::new(vec![
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
        ]))
    }
}

fn skill_resource(
    uri: &str,
    name: &str,
    description: &str,
    mime_type: &str,
    plugin_name: &str,
    skill_name: &str,
) -> Resource {
    Resource::new(
        RawResource::new(uri, name)
            .with_description(description)
            .with_mime_type(mime_type)
            .with_meta(skill_resource_meta(plugin_name, skill_name)),
        /*annotations*/ None,
    )
}

fn skill_resource_meta(plugin_name: &str, skill_name: &str) -> Meta {
    Meta(serde_json::Map::from_iter([
        ("plugin_name".to_string(), json!(plugin_name)),
        ("skill_name".to_string(), json!(skill_name)),
    ]))
}
