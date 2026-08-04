use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::responses;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::mcp_tool::TEST_SERVER_NAME;
use super::mcp_tool::TEST_TOOL_NAME;
use super::mcp_tool::start_mcp_server;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Fresh-context children inherit disabled built-ins while configured MCP tools
/// remain visible or searchable and executable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_context_subagent_inherits_disabled_view_image_and_mcp_tools() -> Result<()> {
    const PARENT_PROMPT: &str = "spawn a worker to call the client-managed MCP tool";
    const CHILD_PROMPT: &str = "call the inherited client-managed MCP tool";
    const SPAWN_CALL_ID: &str = "spawn-client-mcp-worker";
    const MCP_CALL_ID: &str = "call-child-mcp-with-disabled-view-image";

    let responses_server = responses::start_mock_server().await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_model("gpt-5.4")
        .with_provider_config("supports_websockets = false")
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_server_url}/mcp\"\n\n[features.multi_agent_v2]\nenabled = true"
        ))
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            config: Some(
                [("features.view_image".to_string(), json!(false))]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        })
        .await?;

    let namespace = format!("mcp__{TEST_SERVER_NAME}");
    let message = "client-managed viewer remains available";
    responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("parent-spawn"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "collaboration",
                "spawn_agent",
                &serde_json::to_string(&json!({
                    "message": CHILD_PROMPT,
                    "task_name": "mcp_worker",
                    "fork_turns": "none",
                }))?,
            ),
            responses::ev_completed("parent-spawn"),
        ]),
    )
    .await;
    let child_requests = responses::mount_sse_once_match(
        &responses_server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(CHILD_PROMPT)
                && !body.contains(SPAWN_CALL_ID)
                && !body.contains(MCP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("child-mcp"),
            responses::ev_function_call_with_namespace(
                MCP_CALL_ID,
                &namespace,
                TEST_TOOL_NAME,
                &serde_json::to_string(&json!({ "message": message }))?,
            ),
            responses::ev_completed("child-mcp"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &responses_server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(SPAWN_CALL_ID)
        },
        create_final_assistant_message_sse_response("worker spawned")?,
    )
    .await;
    let child_followup = responses::mount_sse_once_match(
        &responses_server,
        |request: &wiremock::Request| String::from_utf8_lossy(&request.body).contains(MCP_CALL_ID),
        create_final_assistant_message_sse_response("MCP tool completed")?,
    )
    .await;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: PARENT_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    let child_thread_id = loop {
        let completed: ItemCompletedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("item/completed"),
        )
        .await??;
        if matches!(&completed.item, ThreadItem::McpToolCall { id, .. } if id == MCP_CALL_ID) {
            assert_ne!(completed.thread_id, thread.id);
            break completed.thread_id;
        }
    };
    loop {
        let completed: TurnCompletedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("turn/completed"),
        )
        .await??;
        if completed.thread_id == child_thread_id {
            break;
        }
    }

    let model_request = child_requests
        .last_request()
        .expect("expected fresh-context child model request")
        .body_json();
    let visible_tools = model_request["tools"]
        .as_array()
        .expect("expected model-visible tools");
    assert!(
        visible_tools
            .iter()
            .all(|tool| tool["name"] != "view_image"),
        "the native image viewer must not reach the model"
    );
    assert!(
        responses::namespace_child_tool(&model_request, &namespace, TEST_TOOL_NAME).is_some()
            || visible_tools
                .iter()
                .any(|tool| tool["type"] == "tool_search"),
        "the namespaced MCP tool must remain directly visible or searchable"
    );
    let tool_output = child_followup
        .last_request()
        .expect("expected child follow-up model request")
        .function_call_output(MCP_CALL_ID);
    assert!(
        tool_output.to_string().contains(message),
        "expected the child model to receive the MCP result: {tool_output}"
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;
    Ok(())
}

/// Guardian reviewer turns respect a disabled viewer while retaining execution tools.
#[tokio::test]
async fn guardian_reviewer_inherits_disabled_view_image() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let guardian_request = responses::mount_sse_once(
        &responses_server,
        create_final_assistant_message_sse_response("review complete")?,
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let guardian_thread_id = create_fake_parented_rollout_with_source(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "review a requested action",
        Some("mock_provider"),
        /*git_info*/ None,
        SessionSource::SubAgent(SubAgentSource::Other("guardian".to_string())),
        ThreadId::new().into(),
        ThreadId::new(),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: guardian_thread_id,
            config: Some(
                [("features.view_image".to_string(), json!(false))]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    let environment = mcp.auto_env_params()?;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                input: vec![UserInput::Text {
                    text: "review the requested action".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Some(vec![environment]),
                ..Default::default()
            },
        })
        .await?;
    let _: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;

    let request = guardian_request.single_request().body_json();
    let tools = request["tools"].as_array().expect("model-visible tools");
    for tool in ["exec_command", "write_stdin"] {
        assert!(
            tools.iter().any(|spec| spec["name"] == tool),
            "guardian reviewer must retain {tool}"
        );
    }
    assert!(
        tools.iter().all(|spec| spec["name"] != "view_image"),
        "guardian reviewer must not receive the disabled image viewer"
    );

    Ok(())
}
