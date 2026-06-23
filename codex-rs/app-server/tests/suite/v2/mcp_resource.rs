use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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
const RAW_SKILL_DESCRIPTION: &str = "Deploy\nthrough the <hosted> orchestrator.";
const SKILL_DESCRIPTION: &str = "Deploy through the &lt;hosted&gt; orchestrator.";
const SKILL_RESOURCE_URI: &str = "skill://plugin_demo/deploy";
const SKILL_MAIN_PROMPT_URI: &str = "skill://plugin_demo/deploy/SKILL.md";
const SKILL_REFERENCE_URI: &str = "skill://plugin_demo/deploy/references/deploy.md";
const SKILL_MARKER: &str = "ORCHESTRATOR_SKILL_BODY_MARKER";
const SKILL_CONTENTS: &str = concat!(
    "---\n",
    "name: deploy\n",
    "description: Deploy through the orchestrator.\n",
    "---\n\n",
    "# Deploy\n\n",
    "ORCHESTRATOR_SKILL_BODY_MARKER\n\n",
    "Read the [deployment reference](skill://plugin_demo/deploy/references/deploy.md).\n",
);
const SKILL_REFERENCE_CONTENTS: &str =
    "# Deploy reference\n\nUse the orchestrator deployment API.\n";
const SKILLS_LIST_CALL_ID: &str = "skills-list";
const SKILLS_READ_CALL_ID: &str = "skills-read";
const SKILLS_READ_AGAIN_CALL_ID: &str = "skills-read-again";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_returns_resource_contents() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, _apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) =
        start_resource_test_app_server(&apps_server_url, &responses_server_uri).await?;

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

    let read_request_id = mcp
        .send_mcp_resource_read_request(McpResourceReadParams {
            thread_id: Some(thread.id),
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
async fn orchestrator_skill_can_read_referenced_resource_without_an_executor() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) =
        start_resource_test_app_server(&apps_server_url, &responses_server_uri).await?;

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

    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-skills-list"),
                responses::ev_function_call_with_namespace(
                    SKILLS_LIST_CALL_ID,
                    "skills",
                    "list",
                    &json!({
                        "authority": {
                            "kind": "orchestrator",
                        },
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-list"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-skills-read"),
                responses::ev_function_call_with_namespace(
                    SKILLS_READ_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "authority": {
                            "kind": "orchestrator",
                        },
                        "package": SKILL_RESOURCE_URI,
                        "resource": SKILL_REFERENCE_URI,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-read"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-skills-read-again"),
                responses::ev_function_call_with_namespace(
                    SKILLS_READ_AGAIN_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "authority": {
                            "kind": "orchestrator",
                        },
                        "package": SKILL_RESOURCE_URI,
                        "resource": SKILL_REFERENCE_URI,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-read-again"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-orchestrator-skill"),
                responses::ev_assistant_message("msg-orchestrator-skill", "Done"),
                responses::ev_completed("resp-orchestrator-skill"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-orchestrator-skill-after-refresh"),
                responses::ev_assistant_message("msg-orchestrator-skill-after-refresh", "Done"),
                responses::ev_completed("resp-orchestrator-skill-after-refresh"),
            ]),
        ],
    )
    .await;
    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
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

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let first_request = &requests[0];
    assert!(first_request.tool_by_name("skills", "list").is_some());
    assert!(first_request.tool_by_name("skills", "read").is_some());
    assert!(first_request.tool_by_name("skills", "search").is_none());

    let developer_messages = first_request.message_input_texts("developer");
    let catalog_line = format!(
        "- {SKILL_NAME}: {SKILL_DESCRIPTION} (orchestrator resource: {SKILL_RESOURCE_URI})"
    );
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
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("do not treat `skill://` identifiers as filesystem paths"))
    );
    let skill_fragments = first_request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(1, skill_fragments.len());
    assert!(skill_fragments[0].contains(&format!("<name>{SKILL_NAME}</name>")));
    assert!(skill_fragments[0].contains(SKILL_MARKER));
    assert!(skill_fragments[0].contains(SKILL_REFERENCE_URI));

    let list_output = requests[1]
        .function_call_output_text(SKILLS_LIST_CALL_ID)
        .ok_or_else(|| anyhow::anyhow!("skills.list output should be sent to the model"))?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&list_output)?,
        json!({
            "skills": [{
                "authority": {
                    "kind": "orchestrator",
                },
                "package": SKILL_RESOURCE_URI,
                "name": SKILL_NAME,
                "description": SKILL_DESCRIPTION,
                "main_resource": SKILL_MAIN_PROMPT_URI,
            }],
            "warnings": ["Orchestrator skill discovery stopped after 2 resource pages: failed to list orchestrator skill resources: resources/list failed for `codex_apps`: Mcp error: -32603: simulated later-page failure"],
        })
    );

    let read_output = requests[2]
        .function_call_output_text(SKILLS_READ_CALL_ID)
        .ok_or_else(|| anyhow::anyhow!("skills.read output should be sent to the model"))?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&read_output)?,
        json!({
            "resource": SKILL_REFERENCE_URI,
            "contents": SKILL_REFERENCE_CONTENTS,
        })
    );
    let repeated_read_output = requests[3]
        .function_call_output_text(SKILLS_READ_AGAIN_CALL_ID)
        .ok_or_else(|| {
            anyhow::anyhow!("repeated skills.read output should be sent to the model")
        })?;
    assert_eq!(read_output, repeated_read_output);
    assert_eq!(
        ResourceAppsMcpCallCounts {
            list_resources: 3,
            main_prompt_reads: 1,
            reference_reads: 1,
        },
        apps_server_calls.snapshot()
    );

    let refresh_request_id = mcp
        .send_raw_request("config/mcpServer/reload", /*params*/ None)
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(refresh_request_id)),
    )
    .await??;

    let refreshed_turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: format!("Use ${SKILL_NAME} after refreshing MCP"),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(refreshed_turn_start_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        ResourceAppsMcpCallCounts {
            list_resources: 6,
            main_prompt_reads: 2,
            reference_reads: 1,
        },
        apps_server_calls.snapshot()
    );
    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_executor_does_not_expose_orchestrator_skills() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, _apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) =
        start_resource_test_app_server(&apps_server_url, &responses_server_uri).await?;

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

    let response_mock = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-no-orchestrator-skill"),
            responses::ev_assistant_message("msg-no-orchestrator-skill", "Done"),
            responses::ev_completed("resp-no-orchestrator-skill"),
        ]),
    )
    .await;
    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
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
    assert!(request.tool_by_name("skills", "list").is_none());
    assert!(request.tool_by_name("skills", "read").is_none());
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .all(|text| !text.contains(SKILL_NAME))
    );
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .all(|text| !text.contains(SKILL_MARKER))
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_orchestrator_skills_do_not_expose_skills_namespace() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) = start_resource_test_app_server_with_extra_config(
        &apps_server_url,
        &responses_server_uri,
        r#"
[orchestrator.skills]
enabled = false
"#,
    )
    .await?;

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

    let response_mock = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-disabled-orchestrator-skills"),
            responses::ev_assistant_message("msg-disabled-orchestrator-skills", "Done"),
            responses::ev_completed("resp-disabled-orchestrator-skills"),
        ]),
    )
    .await;
    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
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
    assert!(request.tool_by_name("skills", "list").is_none());
    assert!(request.tool_by_name("skills", "read").is_none());
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .all(|text| !text.contains(SKILL_NAME))
    );
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .all(|text| !text.contains(SKILL_MARKER))
    );
    assert_eq!(
        ResourceAppsMcpCallCounts {
            list_resources: 0,
            main_prompt_reads: 0,
            reference_reads: 0,
        },
        apps_server_calls.snapshot()
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_returns_resource_contents_without_thread() -> Result<()> {
    let (apps_server_url, _apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;

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

async fn start_resource_test_app_server(
    apps_server_url: &str,
    responses_server_uri: &str,
) -> Result<(TempDir, TestAppServer)> {
    start_resource_test_app_server_with_extra_config(apps_server_url, responses_server_uri, "")
        .await
}

async fn start_resource_test_app_server_with_extra_config(
    apps_server_url: &str,
    responses_server_uri: &str,
    extra_config: &str,
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

[skills]
include_instructions = true
{extra_config}

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
    Ok((codex_home, mcp))
}

async fn start_resource_apps_mcp_server()
-> Result<(String, Arc<ResourceAppsMcpCalls>, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let apps_server_url = format!("http://{addr}");
    let calls = Arc::new(ResourceAppsMcpCalls::default());
    let server_calls = Arc::clone(&calls);

    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(ResourceAppsMcpServer {
                calls: Arc::clone(&server_calls),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/api/codex/ps/mcp", mcp_service);
    let apps_server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((apps_server_url, calls, apps_server_handle))
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

#[derive(Debug, Default)]
struct ResourceAppsMcpCalls {
    list_resources: AtomicUsize,
    main_prompt_reads: AtomicUsize,
    reference_reads: AtomicUsize,
}

impl ResourceAppsMcpCalls {
    fn snapshot(&self) -> ResourceAppsMcpCallCounts {
        ResourceAppsMcpCallCounts {
            list_resources: self.list_resources.load(Ordering::Relaxed),
            main_prompt_reads: self.main_prompt_reads.load(Ordering::Relaxed),
            reference_reads: self.reference_reads.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ResourceAppsMcpCallCounts {
    list_resources: usize,
    main_prompt_reads: usize,
    reference_reads: usize,
}

#[derive(Clone)]
struct ResourceAppsMcpServer {
    calls: Arc<ResourceAppsMcpCalls>,
}

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
        self.calls.list_resources.fetch_add(1, Ordering::Relaxed);
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
            self.calls.main_prompt_reads.fetch_add(1, Ordering::Relaxed);
            return Ok(ReadResourceResult::new(vec![
                ResourceContents::TextResourceContents {
                    uri: SKILL_MAIN_PROMPT_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: SKILL_CONTENTS.to_string(),
                    meta: None,
                },
            ]));
        }
        if uri == SKILL_REFERENCE_URI {
            self.calls.reference_reads.fetch_add(1, Ordering::Relaxed);
            return Ok(ReadResourceResult::new(vec![
                ResourceContents::TextResourceContents {
                    uri: SKILL_REFERENCE_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: SKILL_REFERENCE_CONTENTS.to_string(),
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
