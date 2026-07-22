use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_fake_rollout;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::SessionSource as ApiSessionSource;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::ThreadId as CoreThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::timeout;

// Bazel CI can spend tens of seconds starting app-server subprocesses or
// processing turn RPCs under load.
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[tokio::test]
async fn turn_start_forwards_client_metadata_to_responses_request_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            thread_source: Some(ThreadSource::Feature("automation".to_string())),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let client_metadata = HashMap::from([
        ("fiber_run_id".to_string(), "fiber-start-123".to_string()),
        ("origin".to_string(), "gaas".to_string()),
    ]);
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(client_metadata.clone()),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let metadata = request
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("x-codex-turn-metadata header should be present");
    assert_eq!(metadata["fiber_run_id"].as_str(), Some("fiber-start-123"));
    assert_eq!(metadata["origin"].as_str(), Some("gaas"));
    assert_eq!(metadata["thread_source"].as_str(), Some("automation"));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));
    assert!(metadata.get("installation_id").is_some());
    assert!(metadata.get("session_id").is_some());
    assert_eq!(
        metadata["window_id"].as_str(),
        request.header("x-codex-window-id").as_deref()
    );

    Ok(())
}

#[tokio::test]
async fn turn_start_sends_fork_lineage_in_turn_metadata_for_thread_fork_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let source_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let ThreadForkResponse { thread, .. } =
        fork_fake_rollout_thread(&mut mcp, source_thread_id.clone()).await?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Continue".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let metadata = request
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("x-codex-turn-metadata header should be present");
    assert_eq!(
        metadata["forked_from_thread_id"].as_str(),
        Some(source_thread_id.as_str())
    );
    assert_eq!(metadata["thread_id"].as_str(), Some(thread.id.as_str()));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));

    Ok(())
}

#[tokio::test]
async fn review_start_sends_parent_lineage_in_turn_metadata_for_thread_fork_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let review_payload = serde_json::json!({
        "findings": [],
        "overall_correctness": "good",
        "overall_explanation": "Done",
        "overall_confidence_score": 0.5
    })
    .to_string();
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", &review_payload),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let source_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let ThreadForkResponse { thread, .. } =
        fork_fake_rollout_thread(&mut mcp, source_thread_id.clone()).await?;

    let review_req = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id: thread.id.clone(),
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::Custom {
                instructions: "Review the fork".to_string(),
            },
        })
        .await?;
    let ReviewStartResponse {
        review_thread_id, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(review_req)).await??;
    assert_eq!(review_thread_id, thread.id);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let metadata = request
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("x-codex-turn-metadata header should be present");
    assert_eq!(
        request.header("x-openai-subagent").as_deref(),
        Some("review")
    );
    assert!(metadata.get("forked_from_thread_id").is_none());
    assert_eq!(
        metadata["parent_thread_id"].as_str(),
        Some(review_thread_id.as_str())
    );
    let review_request_thread_id = metadata["thread_id"]
        .as_str()
        .expect("review request thread_id should be present");
    assert!(review_request_thread_id != review_thread_id.as_str());
    assert_eq!(
        request
            .header("x-codex-window-id")
            .as_deref()
            .and_then(|window_id| window_id.split_once(':').map(|(thread_id, _)| thread_id)),
        Some(review_request_thread_id)
    );
    assert!(metadata["turn_id"].as_str().is_some());

    Ok(())
}

#[tokio::test]
async fn turn_start_sends_nested_subagent_lineage_after_cold_thread_resume_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let root_thread_id = CoreThreadId::new();
    let root_thread_id_str = root_thread_id.to_string();
    let parent_thread_id = CoreThreadId::new();
    let parent_thread_id_str = parent_thread_id.to_string();
    let subagent_thread_id = create_fake_parented_rollout_with_source(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved subagent message",
        Some("mock_provider"),
        /*git_info*/ None,
        SessionSource::SubAgent(SubAgentSource::Other("guardian".to_string())),
        root_thread_id.into(),
        parent_thread_id,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let resume_req = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: subagent_thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_req)).await??;
    assert_eq!(thread.id, subagent_thread_id);
    assert_eq!(thread.session_id, root_thread_id_str);
    assert_eq!(thread.parent_thread_id, Some(parent_thread_id_str.clone()));
    assert_eq!(
        thread.source,
        ApiSessionSource::SubAgent(SubAgentSource::Other("guardian".to_string()))
    );

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Continue".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let metadata = request
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("x-codex-turn-metadata header should be present");
    assert_eq!(
        metadata["parent_thread_id"].as_str(),
        Some(parent_thread_id_str.as_str())
    );
    assert_eq!(metadata["subagent_kind"].as_str(), Some("guardian"));
    assert_eq!(
        metadata["session_id"].as_str(),
        Some(thread.session_id.as_str())
    );
    assert_eq!(metadata["thread_id"].as_str(), Some(thread.id.as_str()));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));
    assert!(metadata.get("forked_from_thread_id").is_none());

    Ok(())
}

#[tokio::test]
async fn turn_steer_updates_client_metadata_on_follow_up_responses_request_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let codex_home = TempDir::new()?;

    let server = responses::start_mock_server().await;
    let first_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Working"),
        responses::ev_completed("resp-1"),
    ]))
    .set_delay(std::time::Duration::from_secs(2));
    let second_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]));
    let request_log =
        responses::mount_response_sequence(&server, vec![first_response, second_response]).await;

    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let start_metadata =
        HashMap::from([("fiber_run_id".to_string(), "fiber-start-123".to_string())]);
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Run sleep".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(start_metadata.clone()),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;
    let turn_id = turn.id.clone();

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    wait_for_request_count(&request_log, /*expected*/ 1).await?;

    let steer_metadata = HashMap::from([
        ("fiber_run_id".to_string(), "fiber-steer-456".to_string()),
        ("origin".to_string(), "gaas".to_string()),
    ]);
    let steer_req = mcp
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Focus on the failure".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(steer_metadata.clone()),
            additional_context: None,
            expected_turn_id: turn_id.clone(),
        })
        .await?;
    let _turn: TurnSteerResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(steer_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 2);
    let first_metadata = requests[0]
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("first x-codex-turn-metadata header should be present");
    assert_eq!(
        first_metadata["fiber_run_id"].as_str(),
        Some("fiber-start-123")
    );
    assert_eq!(first_metadata["turn_id"].as_str(), Some(turn_id.as_str()));

    let second_metadata = requests[1]
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("second x-codex-turn-metadata header should be present");
    assert_eq!(
        second_metadata["fiber_run_id"].as_str(),
        Some("fiber-steer-456")
    );
    assert_eq!(second_metadata["origin"].as_str(), Some("gaas"));
    assert_eq!(second_metadata["turn_id"].as_str(), Some(turn_id.as_str()));

    Ok(())
}

#[tokio::test]
async fn turn_start_forwards_client_metadata_to_responses_websocket_request_body_v2() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let websocket_server = responses::start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warm-1"),
            responses::ev_completed("warm-1"),
        ],
        vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&websocket_server.uri().replacen("ws://", "http://", 1))
        .with_provider_config("supports_websockets = true")
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            thread_source: Some(ThreadSource::Feature("automation".to_string())),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let client_metadata = HashMap::from([
        ("fiber_run_id".to_string(), "fiber-start-123".to_string()),
        ("origin".to_string(), "gaas".to_string()),
    ]);
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(client_metadata),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let warmup = websocket_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await
        .body_json();
    let request = websocket_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 1)
        .await
        .body_json();

    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(request["type"].as_str(), Some("response.create"));
    assert_eq!(request["previous_response_id"].as_str(), Some("warm-1"));

    let metadata = request["client_metadata"]["x-codex-turn-metadata"]
        .as_str()
        .map(parse_json_header)
        .expect("websocket x-codex-turn-metadata client metadata should be present");
    assert_eq!(metadata["fiber_run_id"].as_str(), Some("fiber-start-123"));
    assert_eq!(metadata["origin"].as_str(), Some("gaas"));
    assert_eq!(metadata["thread_source"].as_str(), Some("automation"));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));
    assert!(metadata.get("session_id").is_some());
    assert_eq!(
        metadata["window_id"].as_str(),
        request["client_metadata"]["x-codex-window-id"].as_str()
    );

    websocket_server.shutdown().await;
    Ok(())
}

async fn fork_fake_rollout_thread(
    mcp: &mut TestAppServer,
    source_thread_id: String,
) -> Result<ThreadForkResponse> {
    let fork_req = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: source_thread_id,
            thread_source: Some(ThreadSource::User),
            ..Default::default()
        })
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_req)).await?
}

fn parse_json_header(value: &str) -> serde_json::Value {
    serde_json::from_str(value).expect("metadata header should contain valid JSON")
}

async fn wait_for_request_count(
    request_log: &core_test_support::responses::ResponseMock,
    expected: usize,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            if request_log.requests().len() >= expected {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}
