use std::path::Path;
use std::pin::pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Poll;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::write_chatgpt_auth;
use axum::Router;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GitInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::McpResourceReadTarget;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::ProjectCreateParams;
use codex_app_server_protocol::ProjectCreateResponse;
use codex_app_server_protocol::ProjectDeleteParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSection;
use codex_app_server_protocol::ThreadSectionMoveParams;
use codex_app_server_protocol::ThreadSectionMoveResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::ThreadConfigContext;
use codex_config::ThreadConfigLoader;
use codex_config::ThreadConfigLoaderFuture;
use codex_config::ThreadConfigSource;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSettingsAppliedEvent;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use codex_rollout::append_rollout_item_to_path;
use codex_state::PINNED_THREAD_SECTION_ID;
use codex_state::PINNED_THREAD_SECTION_NAME;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses;
use futures::poll;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitResult;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::MetaObject;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
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
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::timeout;

#[path = "mcp_resource/plugin_metadata_refresh.rs"]
mod plugin_metadata_refresh;

pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_RESOURCE_URI: &str = "test://codex/resource";
pub(super) const TEST_WIDGET_RESOURCE_URI: &str = "ui://widget/checkout-session.html";
const TEST_BLOB_RESOURCE_URI: &str = "test://codex/resource.bin";
const TEST_RESOURCE_BLOB: &str = "YmluYXJ5LXJlc291cmNl";
const TEST_RESOURCE_TEXT: &str = "Resource body from the MCP server.";
const TEST_ERROR_RESOURCE_URI: &str = "test://codex/error";
const TEST_ELICITATION_RESOURCE_URI: &str = "test://codex/elicitation";
const TEST_ELICITATION_RESOURCE_TEXT: &str = "Threadless elicitation was declined.";
const SKILL_NAME: &str = "demo-plugin:deploy";
const RAW_SKILL_DESCRIPTION: &str = "Deploy\nthrough the <hosted> orchestrator.";
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_returns_resource_contents() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) = start_resource_test_app_server(
        &apps_server_url,
        &responses_server_uri,
        ResourceTestEnvironment::Auto,
    )
    .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    for thread_id in [Some(thread.id), None] {
        for target in [
            None,
            Some(McpResourceReadTarget {
                connector_id: "calendar".to_string(),
                link_id: Some("link_calendar".to_string()),
            }),
            Some(McpResourceReadTarget {
                connector_id: "calendar".to_string(),
                link_id: None,
            }),
        ] {
            let read_response: McpResourceReadResponse = mcp
                .request(|request_id| ClientRequest::McpResourceRead {
                    request_id,
                    params: McpResourceReadParams {
                        thread_id: thread_id.clone(),
                        origin_call_id: None,
                        server: "codex_apps".to_string(),
                        uri: TEST_RESOURCE_URI.to_string(),
                        connector_id: None,
                        target: target.clone(),
                    },
                })
                .await?;
            assert_eq!(read_response, expected_resource_read_response());
            let metadata = apps_server_calls
                .read_metadata
                .lock()
                .expect("read metadata lock");
            let metadata = metadata
                .last()
                .expect("resource read reached the MCP server");
            assert_eq!(
                (
                    metadata.get("connector_id").cloned(),
                    metadata.get("link_id").cloned()
                ),
                match &target {
                    Some(target) => (
                        Some(json!(target.connector_id)),
                        Some(json!(target.link_id)),
                    ),
                    None => (None, None),
                },
            );
        }
    }

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[test_case(ProtocolVersion::V_2025_06_18; "legacy")]
#[test_case(ProtocolVersion::V_2026_07_28; "modern")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_read_preserves_protocol_errors(protocol: ProtocolVersion) -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, _calls, apps_server_handle) = start_resource_apps_mcp_server().await?;
    let codex_home = TempDir::new()?;
    let config = MockResponsesConfig::new(&responses_server.uri()).with_extra_config(&format!(
        "[mcp_servers.resource_server]\nurl = \"{apps_server_url}/api/codex/ps/mcp\""
    ));
    let config = if protocol == ProtocolVersion::V_2026_07_28 {
        config.enable_feature(Feature::Mcp20260728)
    } else {
        config.disable_feature(Feature::Mcp20260728)
    };
    config.write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp.start_thread(ThreadStartParams::default()).await?;

    for (server, expected_error) in [
        (
            "resource_server",
            json!({
                "code": -32042,
                "message": "resource authorization required",
                "data": {
                    "uri": TEST_ERROR_RESOURCE_URI,
                    "protocolVersion": protocol,
                    "_meta": {"_codex_apps": {"connector_auth_failure": {
                        "is_auth_failure": true,
                        "connector_id": "calendar",
                        "requested_scopes": ["calendar.read"],
                    }}},
                },
            }),
        ),
        (
            "missing",
            json!({"code": -32603, "message": "unknown MCP server 'missing'"}),
        ),
    ] {
        for thread_id in [Some(thread.id.clone()), None] {
            let request_id = mcp
                .send_mcp_resource_read_request(McpResourceReadParams {
                    thread_id,
                    origin_call_id: None,
                    server: server.to_string(),
                    uri: TEST_ERROR_RESOURCE_URI.to_string(),
                    connector_id: None,
                    target: None,
                })
                .await?;
            let error = timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
            )
            .await??;
            assert_eq!(
                serde_json::to_value(error)?,
                json!({"id": request_id, "error": expected_error})
            );
        }
    }

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_executor_does_not_expose_cloud_skills() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, _apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) = start_resource_test_app_server(
        &apps_server_url,
        &responses_server_uri,
        // This test exercises the implicit local executor.
        ResourceTestEnvironment::Local,
    )
    .await?;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_start_id)).await??;

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
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_start_id)).await??;
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

#[test_case(""; "default_off")]
#[test_case("[cloud.skills]\nenabled = false"; "explicitly_disabled")]
#[test_case("[orchestrator.skills]\nenabled = true"; "legacy_setting_ignored")]
#[test_case("[cloud.skills]\nenabled = true"; "enabled_without_provider")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_cloud_provider_does_not_expose_skills_namespace(extra_config: &str) -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;
    let responses_server_uri = responses_server.uri();
    let (_codex_home, mut mcp) = start_resource_test_app_server_with_extra_config(
        &apps_server_url,
        &responses_server_uri,
        extra_config,
        ResourceTestEnvironment::Auto,
    )
    .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

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
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_start_id)).await??;
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
async fn mcp_resource_read_returns_contents_and_declines_elicitation_without_thread() -> Result<()>
{
    let (apps_server_url, _apps_server_calls, apps_server_handle) =
        start_resource_apps_mcp_server().await?;

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{apps_server_url}"
mcp_oauth_credentials_store = "file"
approval_policy = "never"
sandbox_mode = "danger-full-access"

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let read_response: McpResourceReadResponse = mcp
        .request(|request_id| ClientRequest::McpResourceRead {
            request_id,
            params: McpResourceReadParams {
                thread_id: None,
                origin_call_id: None,
                server: "codex_apps".to_string(),
                uri: TEST_RESOURCE_URI.to_string(),
                connector_id: None,
                target: None,
            },
        })
        .await?;
    assert_eq!(read_response, expected_resource_read_response());
    let read_response: McpResourceReadResponse = mcp
        .request(|request_id| ClientRequest::McpResourceRead {
            request_id,
            params: McpResourceReadParams {
                thread_id: None,
                origin_call_id: None,
                server: "codex_apps".to_string(),
                uri: TEST_ELICITATION_RESOURCE_URI.to_string(),
                connector_id: None,
                target: None,
            },
        })
        .await?;
    assert_eq!(
        read_response,
        McpResourceReadResponse {
            contents: vec![McpResourceContent::Text {
                uri: TEST_ELICITATION_RESOURCE_URI.to_string(),
                mime_type: Some("text/plain".to_string()),
                text: TEST_ELICITATION_RESOURCE_TEXT.to_string(),
                meta: None,
            }],
            origin_call_id: None,
        }
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test]
async fn mcp_resource_read_returns_error_for_unknown_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    // This negative-path test does not need the stdio subprocess; keeping it
    // in-process avoids child-process teardown timing in nextest leak detection.
    let client = start_resource_in_process_client(
        codex_home.path(),
        Arc::new(codex_config::NoopThreadConfigLoader),
    )
    .await?;

    let response = client
        .request(ClientRequest::McpResourceRead {
            request_id: RequestId::Integer(1),
            params: McpResourceReadParams {
                thread_id: Some("00000000-0000-4000-8000-000000000000".to_string()),
                origin_call_id: None,
                server: "codex_apps".to_string(),
                uri: TEST_RESOURCE_URI.to_string(),
                connector_id: None,
                target: None,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_and_mcp_requests_complete_while_unrelated_resume_loads_config() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, calls, mcp_server_handle) = start_resource_apps_mcp_server().await?;
    calls.tools_enabled.store(/*val*/ true, Ordering::Relaxed);
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.resource_server]\nurl = \"{mcp_server_url}/api/codex/ps/mcp\""
        ))
        .write(codex_home.path())?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let resume_cwd = TempDir::new()?;
    let blocked_resume = Arc::new(BlockedResumeConfig {
        cwd: AbsolutePathBuf::from_absolute_path(resume_cwd.path())?,
        matching_loads: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let client =
        start_resource_in_process_client(codex_home.path(), blocked_resume.clone()).await?;
    let sender = client.sender();
    let ThreadResumeResponse { thread, .. } = serde_json::from_value(
        timeout(
            DEFAULT_READ_TIMEOUT,
            sender.request(ClientRequest::ThreadResume {
                request_id: RequestId::Integer(1),
                params: ThreadResumeParams {
                    thread_id,
                    cwd: Some(codex_home.path().to_string_lossy().into_owned()),
                    ..Default::default()
                },
            }),
        )
        .await??
        .expect("resume persisted target thread"),
    )?;

    let mut resume = pin!(sender.request(ClientRequest::ThreadResume {
        request_id: RequestId::Integer(3),
        params: ThreadResumeParams {
            thread_id: "00000000-0000-4000-8000-000000000000".to_string(),
            cwd: Some(resume_cwd.path().to_string_lossy().into_owned()),
            history: Some(vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Resume this unrelated thread.".to_string(),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            }]),
            ..Default::default()
        },
    }));
    // Submit the request, retaining immediate errors until after cleanup.
    let mut early_resume = poll!(&mut resume);
    let request_results = async {
        // Wait for the slow config load before submitting metadata or MCP work.
        timeout(DEFAULT_READ_TIMEOUT, blocked_resume.entered.notified())
            .await
            .context("unrelated resume did not enter config loading")?;
        let results = timeout(DEFAULT_READ_TIMEOUT, async {
            futures::try_join!(
                sender.request(ClientRequest::ThreadSetName {
                    request_id: RequestId::Integer(4),
                    params: ThreadSetNameParams {
                        thread_id: thread.id.clone(),
                        name: "Viewer thread".to_string(),
                    },
                }),
                sender.request(ClientRequest::ThreadMetadataUpdate {
                    request_id: RequestId::Integer(5),
                    params: ThreadMetadataUpdateParams {
                        thread_id: thread.id.clone(),
                        project_id: None,
                        daybreak_enabled: None,
                        git_info: Some(ThreadMetadataGitInfoUpdateParams {
                            sha: None,
                            branch: Some(Some("feature/viewer".to_string())),
                            origin_url: None,
                        }),
                    },
                }),
                sender.request(ClientRequest::ThreadSectionMove {
                    request_id: RequestId::Integer(6),
                    params: ThreadSectionMoveParams {
                        thread_id: thread.id.clone(),
                        section_id: Some(PINNED_THREAD_SECTION_ID.to_string()),
                        before_thread_id: None,
                    },
                }),
                sender.request(ClientRequest::McpResourceRead {
                    request_id: RequestId::Integer(7),
                    params: McpResourceReadParams {
                        thread_id: Some(thread.id.clone()),
                        origin_call_id: None,
                        server: "resource_server".to_string(),
                        uri: TEST_RESOURCE_URI.to_string(),
                        connector_id: None,
                        target: None,
                    },
                }),
                sender.request(ClientRequest::McpServerToolCall {
                    request_id: RequestId::Integer(8),
                    params: McpServerToolCallParams {
                        thread_id: thread.id.clone(),
                        server: "resource_server".to_string(),
                        tool: "best_buy_product_search".to_string(),
                        arguments: Some(json!({ "query": "lamps" })),
                        meta: None,
                    },
                }),
            )
        })
        .await
        .context("metadata and MCP requests did not finish while unrelated resume was blocked")??;
        anyhow::Ok(results)
    }
    .await;
    if early_resume.is_pending() {
        early_resume = poll!(&mut resume);
    }
    let resume_pending = early_resume.is_pending();
    // Always release and drain the blocked resume, including on regression.
    blocked_resume.release.notify_one();
    let resume_result = match early_resume {
        Poll::Pending => timeout(DEFAULT_READ_TIMEOUT, &mut resume).await,
        Poll::Ready(result) => Ok(result),
    };
    let read_result = timeout(
        DEFAULT_READ_TIMEOUT,
        sender.request(ClientRequest::ThreadRead {
            request_id: RequestId::Integer(9),
            params: ThreadReadParams {
                thread_id: thread.id,
                include_turns: false,
            },
        }),
    )
    .await;
    client.shutdown().await?;
    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    let (rename, metadata, section, resource, tool) = request_results?;
    assert!(resume_pending, "resume completed early");
    assert_eq!(
        serde_json::from_value::<ThreadSetNameResponse>(rename.expect("set thread name"))?,
        ThreadSetNameResponse {},
    );
    let updated: ThreadMetadataUpdateResponse =
        serde_json::from_value(metadata.expect("update thread metadata"))?;
    let expected_git_info = Some(GitInfo {
        sha: None,
        branch: Some("feature/viewer".to_string()),
        origin_url: None,
    });
    assert_eq!(updated.thread.git_info, expected_git_info);
    assert_eq!(
        serde_json::from_value::<ThreadSectionMoveResponse>(section.expect("pin thread"))?,
        ThreadSectionMoveResponse {},
    );
    assert_eq!(
        serde_json::from_value::<McpResourceReadResponse>(resource.expect("read resource"))?,
        expected_resource_read_response(),
    );
    assert_eq!(
        serde_json::from_value::<McpServerToolCallResponse>(tool.expect("call tool"))?,
        serde_json::from_value::<McpServerToolCallResponse>(serde_json::to_value(
            CallToolResult::structured(json!({ "products": [] })),
        )?)?,
    );
    resume_result??.expect("finish unrelated resume");
    let read: ThreadReadResponse = serde_json::from_value(read_result??.expect("read thread"))?;
    assert_eq!(
        (read.thread.name, read.thread.git_info, read.thread.section),
        (
            Some("Viewer thread".to_string()),
            expected_git_info,
            Some(ThreadSection {
                id: PINNED_THREAD_SECTION_ID.to_string(),
                name: PINNED_THREAD_SECTION_NAME.to_string(),
                appearance: None,
            }),
        ),
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum ResumeMutation {
    GitMetadata,
    ProjectDeletion,
    Archive,
    Delete,
    DuplicateResume,
}

#[test_case(ResumeMutation::GitMetadata; "git_metadata_and_project_assignment")]
#[test_case(ResumeMutation::ProjectDeletion; "project_deletion")]
#[test_case(ResumeMutation::Archive; "archive")]
#[test_case(ResumeMutation::Delete; "delete")]
#[test_case(ResumeMutation::DuplicateResume; "duplicate_resume")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_revalidates_persisted_thread_after_config_load(
    mutation: ResumeMutation,
) -> Result<()> {
    let responses_server = create_mock_responses_server_repeating_assistant("Ready").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri()).write(codex_home.path())?;
    let filename_timestamp = "2025-01-05T12-00-00";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        filename_timestamp,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        matches!(mutation, ResumeMutation::GitMetadata).then(|| {
            codex_protocol::protocol::GitInfo {
                commit_hash: None,
                branch: Some("feature/before-resume".to_string()),
                repository_url: None,
            }
        }),
    )?;
    let path = rollout_path(codex_home.path(), filename_timestamp, &thread_id);
    let resume_cwd = TempDir::new()?;
    let blocked_resume = Arc::new(BlockedResumeConfig {
        cwd: AbsolutePathBuf::from_absolute_path(resume_cwd.path())?,
        matching_loads: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let mut client =
        start_resource_in_process_client(codex_home.path(), blocked_resume.clone()).await?;
    let sender = client.sender();
    let project_id = if matches!(
        mutation,
        ResumeMutation::GitMetadata | ResumeMutation::ProjectDeletion
    ) {
        let created: ProjectCreateResponse = serde_json::from_value(
            sender
                .request(ClientRequest::ProjectCreate {
                    request_id: RequestId::Integer(100),
                    params: ProjectCreateParams {
                        name: "Resume project".to_string(),
                        roots: Vec::new(),
                        metadata: None,
                        idempotency_key: "resume-project".to_string(),
                    },
                })
                .await?
                .expect("create project"),
        )?;
        Some(created.project.id)
    } else {
        None
    };
    if matches!(mutation, ResumeMutation::ProjectDeletion) {
        sender
            .request(ClientRequest::ThreadMetadataUpdate {
                request_id: RequestId::Integer(101),
                params: ThreadMetadataUpdateParams {
                    thread_id: thread_id.clone(),
                    project_id: project_id.clone(),
                    daybreak_enabled: None,
                    git_info: None,
                },
            })
            .await?
            .expect("assign project before resume");
    }
    let mut resume = pin!(sender.request(ClientRequest::ThreadResume {
        request_id: RequestId::Integer(1),
        params: ThreadResumeParams {
            // A path alias reaches the same persisted thread through a different
            // request queue, so its canonical-ID mutation can run during config loading.
            thread_id: "resume-by-path".to_string(),
            path: Some(path.canonicalize()?),
            cwd: Some(resume_cwd.path().to_string_lossy().into_owned()),
            ..Default::default()
        },
    }));
    let mut early_resume = poll!(&mut resume);
    let mutation_request = match mutation {
        ResumeMutation::GitMetadata => ClientRequest::ThreadMetadataUpdate {
            request_id: RequestId::Integer(2),
            params: ThreadMetadataUpdateParams {
                thread_id: thread_id.clone(),
                project_id: project_id.clone(),
                daybreak_enabled: None,
                git_info: Some(ThreadMetadataGitInfoUpdateParams {
                    sha: None,
                    branch: Some(Some("feature/updated-during-resume".to_string())),
                    origin_url: None,
                }),
            },
        },
        ResumeMutation::ProjectDeletion => ClientRequest::ProjectDelete {
            request_id: RequestId::Integer(2),
            params: ProjectDeleteParams {
                project_id: project_id.clone().expect("project to delete"),
            },
        },
        ResumeMutation::Archive => ClientRequest::ThreadArchive {
            request_id: RequestId::Integer(2),
            params: ThreadArchiveParams {
                thread_id: thread_id.clone(),
            },
        },
        ResumeMutation::Delete => ClientRequest::ThreadDelete {
            request_id: RequestId::Integer(2),
            params: ThreadDeleteParams {
                thread_id: thread_id.clone(),
            },
        },
        ResumeMutation::DuplicateResume => ClientRequest::ThreadResume {
            request_id: RequestId::Integer(2),
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                cwd: Some(resume_cwd.path().to_string_lossy().into_owned()),
                ..Default::default()
            },
        },
    };
    let mutation_result = async {
        timeout(DEFAULT_READ_TIMEOUT, blocked_resume.entered.notified())
            .await
            .context("resume did not enter config loading")?;
        let result = timeout(DEFAULT_READ_TIMEOUT, sender.request(mutation_request))
            .await
            .context("mutation must complete while resume config loading is blocked")??;
        anyhow::Ok(result)
    }
    .await;
    if early_resume.is_pending() {
        early_resume = poll!(&mut resume);
    }
    let resume_pending = early_resume.is_pending();
    blocked_resume.release.notify_one();
    let resume_result = match early_resume {
        Poll::Pending => timeout(DEFAULT_READ_TIMEOUT, &mut resume).await,
        Poll::Ready(result) => Ok(result),
    };
    let resume_config_loads = blocked_resume.matching_loads.load(Ordering::Relaxed);
    let turn_result = match mutation {
        ResumeMutation::GitMetadata | ResumeMutation::ProjectDeletion => {
            complete_in_process_turn(&mut client, RequestId::Integer(3), &thread_id).await
        }
        ResumeMutation::Archive | ResumeMutation::Delete | ResumeMutation::DuplicateResume => {
            Ok(())
        }
    };
    let loaded_result = timeout(
        DEFAULT_READ_TIMEOUT,
        sender.request(ClientRequest::ThreadLoadedList {
            request_id: RequestId::Integer(4),
            params: ThreadLoadedListParams::default(),
        }),
    )
    .await;
    client.shutdown().await?;

    let mutation_response = mutation_result?.expect("mutate persisted thread during resume");
    assert!(resume_pending, "resume must still be loading config");
    let resumed = resume_result??;
    let loaded: ThreadLoadedListResponse =
        serde_json::from_value(loaded_result??.expect("list loaded threads"))?;
    assert_eq!(
        loaded,
        ThreadLoadedListResponse {
            data: match mutation {
                ResumeMutation::Archive | ResumeMutation::Delete => Vec::new(),
                ResumeMutation::GitMetadata
                | ResumeMutation::ProjectDeletion
                | ResumeMutation::DuplicateResume => vec![thread_id.clone()],
            },
            next_cursor: None,
        }
    );
    match mutation {
        ResumeMutation::GitMetadata | ResumeMutation::ProjectDeletion => {
            let resumed: ThreadResumeResponse =
                serde_json::from_value(resumed.expect("resume updated thread"))?;
            let expected_git_info =
                matches!(mutation, ResumeMutation::GitMetadata).then(|| GitInfo {
                    sha: None,
                    branch: Some("feature/updated-during-resume".to_string()),
                    origin_url: None,
                });
            let expected_project_id = if matches!(mutation, ResumeMutation::GitMetadata) {
                project_id
            } else {
                None
            };
            assert_eq!(
                (resumed.thread.git_info, resumed.thread.project_id),
                (expected_git_info.clone(), expected_project_id.clone())
            );
            assert!(
                resume_config_loads >= 2,
                "changed persisted metadata must reload config"
            );
            turn_result?;

            // Check persisted metadata after the resumed writer appends a turn
            // and shuts down; an in-memory response alone can hide stale writes.
            let reader = start_resource_in_process_client(
                codex_home.path(),
                Arc::new(codex_config::NoopThreadConfigLoader),
            )
            .await?;
            let read_result = timeout(
                DEFAULT_READ_TIMEOUT,
                reader.request(ClientRequest::ThreadRead {
                    request_id: RequestId::Integer(1),
                    params: ThreadReadParams {
                        thread_id,
                        include_turns: false,
                    },
                }),
            )
            .await;
            reader.shutdown().await?;
            let read: ThreadReadResponse =
                serde_json::from_value(read_result??.expect("read persisted thread metadata"))?;
            assert_eq!(
                (read.thread.git_info, read.thread.project_id),
                (expected_git_info, expected_project_id)
            );
        }
        ResumeMutation::Archive | ResumeMutation::Delete => {
            assert!(resumed.is_err(), "resume must not reopen a removed rollout");
            assert!(
                !path.exists(),
                "resume must not recreate the active rollout"
            );
            let archived_path = codex_home
                .path()
                .join(ARCHIVED_SESSIONS_SUBDIR)
                .join(path.file_name().expect("rollout filename"));
            assert_eq!(
                archived_path.exists(),
                matches!(mutation, ResumeMutation::Archive)
            );
        }
        ResumeMutation::DuplicateResume => {
            let resumed: ThreadResumeResponse =
                serde_json::from_value(resumed.expect("rejoin thread resumed during config load"))?;
            let concurrent: ThreadResumeResponse = serde_json::from_value(mutation_response)?;
            assert_eq!(
                (resumed.thread.id, concurrent.thread.id),
                (thread_id.clone(), thread_id)
            );
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_reloads_config_when_saved_workspace_roots_change() -> Result<()> {
    let responses_server = create_mock_responses_server_repeating_assistant("Ready").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri()).write(codex_home.path())?;
    let filename_timestamp = "2025-01-05T12-00-00";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        filename_timestamp,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), filename_timestamp, &thread_id);
    let resume_cwd = TempDir::new()?;
    let saved_workspace = TempDir::new()?;
    let cwd = AbsolutePathBuf::from_absolute_path(resume_cwd.path())?;
    let saved_root = AbsolutePathBuf::from_absolute_path(saved_workspace.path())?;
    let settings: ThreadSettingsAppliedEvent = serde_json::from_value(json!({
        "thread_id": thread_id,
        "thread_settings": {
            "model": "gpt-5.4",
            "model_provider_id": "mock_provider",
            "cwd": cwd,
            "runtime_workspace_roots": [saved_root],
            "approval_policy": "never",
            "approvals_reviewer": "user",
            "permission_profile": PermissionProfile::read_only(),
            "collaboration_mode": { "mode": "default", "settings": { "model": "gpt-5.4" } },
        },
    }))?;
    append_rollout_item_to_path(
        &path,
        &RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(settings.clone())),
    )
    .await?;

    let blocked_resume = Arc::new(BlockedResumeConfig {
        cwd: cwd.clone(),
        matching_loads: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let client =
        start_resource_in_process_client(codex_home.path(), blocked_resume.clone()).await?;
    let sender = client.sender();
    let mut resume = pin!(sender.request(ClientRequest::ThreadResume {
        request_id: RequestId::Integer(1),
        params: ThreadResumeParams {
            thread_id: thread_id.clone(),
            cwd: Some(cwd.to_string_lossy().into_owned()),
            ..Default::default()
        },
    }));
    assert!(poll!(&mut resume).is_pending());
    let update_result = async {
        timeout(DEFAULT_READ_TIMEOUT, blocked_resume.entered.notified())
            .await
            .context("resume did not enter config loading")?;
        let mut cleared = settings;
        cleared.thread_settings.runtime_workspace_roots = Some(Vec::new());
        append_rollout_item_to_path(
            &path,
            &RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(cleared)),
        )
        .await?;
        anyhow::Ok(())
    }
    .await;
    blocked_resume.release.notify_one();
    update_result?;
    let resumed: ThreadResumeResponse = serde_json::from_value(
        timeout(DEFAULT_READ_TIMEOUT, &mut resume)
            .await??
            .expect("resume updated thread"),
    )?;
    let (items, _, _) = RolloutRecorder::load_rollout_items(&path).await?;
    client.shutdown().await?;

    assert_eq!(resumed.runtime_workspace_roots, Vec::new());
    assert!(
        blocked_resume.matching_loads.load(Ordering::Relaxed) >= 2,
        "roots-only updates must reload config"
    );
    let expected_thread_id = ThreadId::from_string(&thread_id)?;
    let latest_roots = items.into_iter().rev().find_map(|item| match item {
        RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event))
            if event.thread_id == Some(expected_thread_id) =>
        {
            Some(event.thread_settings.runtime_workspace_roots)
        }
        _ => None,
    });
    assert_eq!(latest_roots, Some(Some(Vec::new())));
    Ok(())
}

async fn complete_in_process_turn(
    client: &mut in_process::InProcessClientHandle,
    request_id: RequestId,
    thread_id: &str,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        client
            .request(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: vec![UserInput::Text {
                        text: "Persist the thread.".to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
            })
            .await?
            .expect("start persistence turn");
        loop {
            let event = client
                .next_event()
                .await
                .context("app-server stopped before turn/completed")?;
            if let InProcessServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                && completed.thread_id == thread_id
            {
                assert_eq!(completed.turn.status, TurnStatus::Completed);
                return anyhow::Ok(());
            }
        }
    })
    .await?
}

struct BlockedResumeConfig {
    cwd: AbsolutePathBuf,
    matching_loads: AtomicUsize,
    entered: Notify,
    release: Notify,
}

impl ThreadConfigLoader for BlockedResumeConfig {
    fn load(
        &self,
        context: ThreadConfigContext,
    ) -> ThreadConfigLoaderFuture<'_, Vec<ThreadConfigSource>> {
        Box::pin(async move {
            if context.cwd.as_ref() == Some(&self.cwd)
                && self.matching_loads.fetch_add(/*val*/ 1, Ordering::Relaxed) == 0
            {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(Vec::new())
        })
    }
}

async fn start_resource_in_process_client(
    codex_home: &Path,
    thread_config_loader: Arc<dyn ThreadConfigLoader>,
) -> Result<in_process::InProcessClientHandle> {
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    // Keep unrelated plugin repository syncs out of these in-process RPC tests.
    let cli_overrides = vec![("features.plugins".to_string(), toml::Value::Boolean(false))];
    let config = ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .fallback_cwd(Some(codex_home.to_path_buf()))
        .cli_overrides(cli_overrides.clone())
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;
    let state_db = codex_rollout::state_db::try_init(&config).await?;
    // Metadata, persistence, and HTTP MCP do not need local execution or shell startup.
    let environment_manager = Arc::new(EnvironmentManager::without_environments(
        config.http_client_factory(),
    ));
    Ok(in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config),
        cli_overrides,
        loader_overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        embedded_network_policy: Default::default(),
        thread_config_loader,
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: Some(state_db),
        environment_manager,
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                ..Default::default()
            }),
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?)
}

pub(super) async fn start_resource_test_app_server(
    apps_server_url: &str,
    responses_server_uri: &str,
    environment: ResourceTestEnvironment,
) -> Result<(TempDir, TestAppServer)> {
    start_resource_test_app_server_with_extra_config(
        apps_server_url,
        responses_server_uri,
        "[cloud.skills]\nenabled = true",
        environment,
    )
    .await
}

async fn start_resource_test_app_server_with_extra_config(
    apps_server_url: &str,
    responses_server_uri: &str,
    extra_config: &str,
    environment: ResourceTestEnvironment,
) -> Result<(TempDir, TestAppServer)> {
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(responses_server_uri)
        .with_approval_policy("on-request")
        .with_root_config(&format!(
            "chatgpt_base_url = \"{apps_server_url}\"\nmcp_oauth_credentials_store = \"file\""
        ))
        .enable_feature(Feature::Apps)
        .with_extra_config(&format!(
            "[skills]\ninclude_instructions = true\n{extra_config}"
        ))
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let builder = TestAppServer::builder().with_codex_home(codex_home.path());
    let builder = match environment {
        ResourceTestEnvironment::Auto => builder,
        // The Local caller explicitly exercises the implicit local executor.
        ResourceTestEnvironment::Local => builder.without_auto_env(),
    };
    let mcp = builder.build_initialized().await?;
    Ok((codex_home, mcp))
}

pub(super) enum ResourceTestEnvironment {
    Auto,
    Local,
}

pub(super) async fn start_resource_apps_mcp_server()
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
        origin_call_id: None,
    }
}

#[derive(Debug, Default)]
pub(super) struct ResourceAppsMcpCalls {
    read_metadata: Mutex<Vec<serde_json::Map<String, serde_json::Value>>>,
    list_resources: AtomicUsize,
    main_prompt_reads: AtomicUsize,
    reference_reads: AtomicUsize,
    pub(super) tools_enabled: AtomicBool,
    pub(super) best_buy_app_only: AtomicBool,
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
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_resources().build())
            .with_protocol_version(ProtocolVersion::V_2025_06_18);
        if self.calls.tools_enabled.load(Ordering::Relaxed) {
            info.capabilities.tools = Some(Default::default());
        }
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let tools = ["best_buy", "walmart"]
            .into_iter()
            .map(|connector_id| {
                let mut ui = json!({ "resourceUri": TEST_WIDGET_RESOURCE_URI });
                if connector_id == "best_buy"
                    && self.calls.best_buy_app_only.load(Ordering::Relaxed)
                {
                    ui["visibility"] = json!(["app"]);
                }
                serde_json::from_value(json!({
                    "name": format!("{connector_id}_product_search"),
                    "description": "Search products.",
                    "inputSchema": { "type": "object" },
                    "annotations": { "readOnlyHint": true },
                    "_meta": {
                        "connector_id": connector_id,
                        "connector_name": connector_id,
                        "link_id": format!("link_{connector_id}"),
                        "ui": ui,
                        "openai/outputTemplate": TEST_WIDGET_RESOURCE_URI,
                        "_codex_apps": {
                            "resource_uri": format!(
                                "/{connector_id}/link_{connector_id}/{connector_id}_product_search"
                            ),
                            "contains_mcp_source": true,
                        },
                    },
                }))
            })
            .collect::<serde_json::Result<Vec<_>>>()
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        if request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("query"))
            == Some(&json!("fail"))
        {
            return Ok(
                CallToolResult::structured_error(json!({ "error": "search failed" })).into(),
            );
        }

        Ok(CallToolResult::structured(json!({ "products": [] })).into())
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        self.calls.list_resources.fetch_add(1, Ordering::Relaxed);
        let cursor = request.and_then(|request| request.cursor);
        if cursor.is_none() {
            let mut result = ListResourcesResult::with_all_items(vec![skill_resource(
                "skill://plugin_ignored/ignored",
                "plugin_ignored/ignored",
                "Not an MCP skill resource.",
                "text/plain",
                "ignored-plugin",
                "ignored",
            )]);
            result.next_cursor = Some("skills-page".to_string());
            return Ok(result);
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

        let mut result = ListResourcesResult::with_all_items(vec![skill_resource(
            SKILL_RESOURCE_URI,
            "plugin_demo/deploy",
            RAW_SKILL_DESCRIPTION,
            "mcp/skill",
            "demo-plugin",
            "deploy",
        )]);
        result.next_cursor = Some("failing-page".to_string());
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, rmcp::ErrorData> {
        self.calls
            .read_metadata
            .lock()
            .expect("read metadata lock")
            .push(context.meta.0.0.clone());
        let uri = request.uri;
        if uri == TEST_ERROR_RESOURCE_URI {
            return Err(rmcp::ErrorData::new(
                rmcp::model::ErrorCode(-32042),
                "resource authorization required",
                Some(json!({
                    "uri": uri,
                    "protocolVersion": context.protocol_version(),
                    "_meta": {"_codex_apps": {"connector_auth_failure": {
                        "is_auth_failure": true,
                        "connector_id": "calendar",
                        "requested_scopes": ["calendar.read"],
                    }}},
                })),
            ));
        }
        if uri == TEST_WIDGET_RESOURCE_URI {
            let request_meta = context
                .meta
                .0
                .0
                .get("x-codex-turn-metadata")
                .and_then(|metadata| metadata.get("mcp_request_meta"));
            let connector_id = request_meta
                .and_then(|metadata| metadata.pointer("/selected_connector_ids/0"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| rmcp::ErrorData::invalid_params("missing app scope", None))?;
            let expected_link_id = format!("link_{connector_id}");
            if request_meta
                .and_then(|metadata| metadata.get("link_id"))
                .and_then(serde_json::Value::as_str)
                != Some(expected_link_id.as_str())
            {
                return Err(rmcp::ErrorData::invalid_params("wrong account scope", None));
            }

            return Ok(
                ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
                    uri,
                    mime_type: Some("text/html".to_string()),
                    text: format!("<html>{connector_id}</html>"),
                    meta: None,
                }])
                .into(),
            );
        }
        if uri == TEST_ELICITATION_RESOURCE_URI {
            let requested_schema = ElicitationSchema::builder()
                .build()
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            let result = context
                .peer
                .create_elicitation(ElicitRequestParams::FormElicitationParams {
                    meta: None,
                    message: "Confirm the resource read.".to_string(),
                    requested_schema,
                })
                .await
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
            assert_eq!(result, ElicitResult::new(ElicitationAction::Decline));

            return Ok(
                ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
                    uri: TEST_ELICITATION_RESOURCE_URI.to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: TEST_ELICITATION_RESOURCE_TEXT.to_string(),
                    meta: None,
                }])
                .into(),
            );
        }
        if uri == SKILL_MAIN_PROMPT_URI {
            self.calls.main_prompt_reads.fetch_add(1, Ordering::Relaxed);
            return Ok(
                ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
                    uri: SKILL_MAIN_PROMPT_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: SKILL_CONTENTS.to_string(),
                    meta: None,
                }])
                .into(),
            );
        }
        if uri == SKILL_REFERENCE_URI {
            self.calls.reference_reads.fetch_add(1, Ordering::Relaxed);
            return Ok(
                ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
                    uri: SKILL_REFERENCE_URI.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: SKILL_REFERENCE_CONTENTS.to_string(),
                    meta: None,
                }])
                .into(),
            );
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
        ])
        .into())
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
    Resource::new(uri, name)
        .with_description(description)
        .with_mime_type(mime_type)
        .with_meta(skill_resource_meta(plugin_name, skill_name))
}

fn skill_resource_meta(plugin_name: &str, skill_name: &str) -> MetaObject {
    MetaObject(serde_json::Map::from_iter([
        ("plugin_name".to_string(), json!(plugin_name)),
        ("skill_name".to_string(), json!(skill_name)),
    ]))
}
