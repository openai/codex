use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_fake_rollout;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ProjectCreateParams;
use codex_app_server_protocol::ProjectCreateResponse;
use codex_app_server_protocol::RawResponseCompletedNotification;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::SessionSource as ApiSessionSource;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_protocol::ThreadId as CoreThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::timeout;

use super::analytics::captured_analytics_events;
use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_analytics_event;
use super::analytics::wait_for_matching_analytics_event;

// Bazel CI can spend tens of seconds starting app-server subprocesses or
// processing turn RPCs under load.
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[tokio::test]
async fn tui_reports_the_current_project_assignment_on_each_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let model = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&model.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", model.uri()))
        .write(codex_home.path())?;
    mount_analytics_capture(&model, codex_home.path()).await?;
    let mut tui = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    tui.initialize_with_client_info(ClientInfo {
        name: "codex-tui".into(),
        version: "1".into(),
        title: None,
    })
    .await?;

    let ProjectCreateResponse { project } = tui
        .request(|request_id| ClientRequest::ProjectCreate {
            request_id,
            params: ProjectCreateParams {
                name: "Work".into(),
                roots: vec![],
                metadata: None,
                idempotency_key: "work".into(),
            },
        })
        .await?;
    let ThreadStartResponse { thread, .. } = tui
        .start_thread(ThreadStartParams {
            project_id: Some(project.id),
            ..Default::default()
        })
        .await?;
    let check_turn = async |tui: &mut TestAppServer, prompt: &str, expected: &str| -> Result<()> {
        let request = responses::mount_sse_once(&model, responses::sse_completed(prompt)).await;
        let completed = tui
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: Some(prompt.into()),
                input: vec![V2UserInput::Text {
                    text: prompt.into(),
                    text_elements: vec![],
                }],
                ..Default::default()
            })
            .await?;
        let header = request
            .single_request()
            .header("x-codex-turn-metadata")
            .expect("turn metadata");
        assert_eq!(parse_json_header(&header)["workspace_kind"], expected);
        let event = wait_for_matching_analytics_event(&model, DEFAULT_READ_TIMEOUT, |event| {
            event["event_type"] == "codex_turn_event"
                && event["event_params"]["turn_id"] == completed.turn.id
        })
        .await?;
        assert_eq!(event["event_params"]["workspace_kind"], expected);
        Ok(())
    };

    check_turn(&mut tui, "First", "project").await?;
    check_turn(&mut tui, "Continue", "project").await?;

    let update = tui
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id.clone(),
            project_id: Some(String::new()),
            git_info: None,
            daybreak_enabled: None,
        })
        .await?;
    let _: ThreadMetadataUpdateResponse = tui.read_response(update).await?;
    check_turn(&mut tui, "After move", "projectless").await
}

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
        (
            "parent_response_id".to_string(),
            "client-correlation".to_string(),
        ),
        ("fiber_run_id".to_string(), "fiber-start-123".to_string()),
        ("origin".to_string(), "gaas".to_string()),
        (
            "context_window_id".to_string(),
            "client-supplied".to_string(),
        ),
        ("turn_trigger".to_string(), "client-supplied".to_string()),
    ]);
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            turn_trigger: Some("user".to_string()),
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
    assert_eq!(
        request.header("x-codex-exclude").as_deref(),
        Some("tools,usage.attribution")
    );
    let metadata = request
        .header("x-codex-turn-metadata")
        .as_deref()
        .map(parse_json_header)
        .expect("x-codex-turn-metadata header should be present");
    let body = request.body_json();
    let body_turn_metadata = parse_json_header(
        body["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("turn metadata"),
    );
    assert_eq!(
        (
            metadata["parent_response_id"].as_str(),
            body_turn_metadata["parent_response_id"].as_str(),
            body["client_metadata"].get("parent_response_id"),
        ),
        (Some("client-correlation"), Some("client-correlation"), None),
    );
    assert_eq!(metadata["fiber_run_id"].as_str(), Some("fiber-start-123"));
    assert_eq!(metadata["origin"].as_str(), Some("gaas"));
    assert_eq!(metadata["thread_source"].as_str(), Some("automation"));
    assert_eq!(metadata["turn_trigger"].as_str(), Some("user"));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));
    assert!(metadata.get("installation_id").is_some());
    assert!(metadata.get("session_id").is_some());
    assert_eq!(
        metadata["window_id"].as_str(),
        request.header("x-codex-window-id").as_deref()
    );
    assert_eq!(metadata["window_number"].as_u64(), Some(0));
    assert!(
        metadata["context_window_id"]
            .as_str()
            .is_some_and(|window_id| uuid::Uuid::parse_str(window_id).is_ok())
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
    assert!(metadata.get("forked_from_ordinal_exclusive").is_none());
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
            responses::ev_web_search_call_added_partial("resumed-search", "in_progress"),
            responses::ev_web_search_call_done("resumed-search", "completed", "test query"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", server.uri()))
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;

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

    let turn_event =
        wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_event").await?;
    let params = &turn_event["event_params"];
    assert_eq!(
        (
            params["total_tool_call_count"].as_u64(),
            params["web_search_count"].as_u64()
        ),
        (Some(1), Some(1))
    );
    timeout(DEFAULT_READ_TIMEOUT, mcp.shutdown_gracefully()).await??;
    let events = captured_analytics_events(&server).await;
    let count = |event_type: &str| {
        events
            .iter()
            .filter(|event| {
                event["event_type"] == event_type
                    && event["event_params"]["thread_id"] == thread.id
                    && event["event_params"]["turn_id"] == turn.id
            })
            .count()
    };
    assert_eq!(
        (count("codex_turn_event"), count("codex_web_search_event")),
        (1, 1)
    );

    Ok(())
}

#[test_case::test_case(false; "replacement")]
#[test_case::test_case(true; "patch")]
#[tokio::test]
async fn turn_steer_updates_client_metadata_on_follow_up_responses_request_v2(
    patch_steer: bool,
) -> Result<()> {
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
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", server.uri()))
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let start_metadata = HashMap::from([
        ("fiber_run_id".to_string(), "fiber-start-123".to_string()),
        ("codex_app_used_dictation".to_string(), "false".to_string()),
        ("source".to_string(), "snapshot-source".to_string()),
        ("start_only".to_string(), "start-value".to_string()),
        (
            "misalignment_override".to_string(),
            r#"{"review_target":"RB._~:-opaque"}"#.to_string(),
        ),
    ]);
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Run sleep".to_string(),
                text_elements: Vec::new(),
            }],
            turn_trigger: Some("user".to_string()),
            responsesapi_client_metadata: Some(start_metadata),
            responsesapi_client_metadata_patch: Some(HashMap::from([(
                "source".to_string(),
                "initial-source".to_string(),
            )])),
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
        ("codex_app_used_dictation".to_string(), "true".to_string()),
        ("origin".to_string(), "gaas".to_string()),
        ("source".to_string(), "steer-source".to_string()),
        (
            "misalignment_override".to_string(),
            r#"{"review_target":"RC"}"#.to_string(),
        ),
    ]);
    let (responsesapi_client_metadata, responsesapi_client_metadata_patch) = if patch_steer {
        (None, Some(steer_metadata))
    } else {
        (Some(steer_metadata), None)
    };
    let steer_req = mcp
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Focus on the failure".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata,
            responsesapi_client_metadata_patch,
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
    assert_eq!(first_metadata["turn_trigger"].as_str(), Some("user"));
    assert_eq!(first_metadata["source"].as_str(), Some("initial-source"));
    assert!(first_metadata.get("codex_app_used_dictation").is_none());

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
    assert_eq!(second_metadata["turn_trigger"].as_str(), Some("user"));
    assert_eq!(second_metadata["source"].as_str(), Some("steer-source"));
    assert!(second_metadata.get("codex_app_used_dictation").is_none());
    assert_eq!(
        second_metadata["start_only"].as_str(),
        patch_steer.then_some("start-value")
    );
    assert_eq!(
        second_metadata["misalignment_override"],
        first_metadata["misalignment_override"],
    );

    let event = wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_event").await?;
    let params = &event["event_params"];
    assert_eq!(
        json!([
            params["thread_id"],
            params["turn_id"],
            params["turn_trigger"],
            params["codex_turn_source"],
            params["used_dictation"],
        ]),
        json!([thread.id, turn_id, "user", "steer-source", true]),
    );

    Ok(())
}

#[tokio::test]
async fn resumed_closed_subagent_retains_raw_usage_attribution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.6-luna")
        .with_root_config("thread_unload_delay_secs = 0")
        .with_extra_config("[features.multi_agent_v2]\nenabled = false")
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let parent = app
        .start_thread(ThreadStartParams {
            experimental_raw_events: true,
            ..Default::default()
        })
        .await?
        .thread;
    let turn = |thread_id: &str, prompt: &str| TurnStartParams {
        thread_id: thread_id.to_owned(),
        input: vec![V2UserInput::Text {
            text: prompt.to_owned(),
            text_elements: vec![],
        }],
        ..Default::default()
    };
    let tool = |name: &str, args: serde_json::Value| {
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                name,
                "multi_agent_v1",
                name,
                &args.to_string(),
            ),
            responses::ev_completed(name),
        ])
    };
    responses::mount_sse_sequence(
        &server,
        vec![
            tool("spawn_agent", json!({"message": "Inspect the example"})),
            responses::sse_completed("done-1"),
            responses::sse_completed("done-2"),
        ],
    )
    .await;
    app.start_turn_and_wait_for_completion(turn(&parent.id, "Spawn a child"))
        .await?;
    let child_turn: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_notification("turn/completed"),
    )
    .await??;
    let child_id = child_turn.thread_id;

    responses::mount_sse_sequence(
        &server,
        vec![
            tool("close_agent", json!({"target": child_id})),
            responses::sse_completed("closed"),
        ],
    )
    .await;
    app.start_turn_and_wait_for_completion(turn(&parent.id, "Close the child"))
        .await?;
    // Resume from the child's subscription after both original runtimes are gone.
    let id = app
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: parent.id.clone(),
        })
        .await?;
    let _: ThreadUnsubscribeResponse = app.read_response(id).await?;
    let closed: ThreadClosedNotification =
        timeout(DEFAULT_READ_TIMEOUT, app.read_notification("thread/closed")).await??;
    assert_eq!(closed.thread_id, parent.id);
    let id = app
        .send_thread_loaded_list_request(Default::default())
        .await?;
    let loaded: ThreadLoadedListResponse = app.read_response(id).await?;
    assert!(loaded.data.is_empty());

    let id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: child_id.clone(),
            ..Default::default()
        })
        .await?;
    let _: ThreadResumeResponse = app.read_response(id).await?;
    let request = responses::mount_sse_once(&server, responses::sse_completed("resumed")).await;
    app.clear_message_buffer();
    app.start_turn_and_wait_for_completion(turn(&child_id, "Continue"))
        .await?;
    assert_eq!(
        request
            .single_request()
            .header("x-codex-exclude")
            .as_deref(),
        Some("tools")
    );
    let raw: RawResponseCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_notification("rawResponse/completed"),
    )
    .await??;
    assert_eq!(raw.response_id, "resumed");
    Ok(())
}

#[test_case::test_case(false, "none", false; "legacy")]
#[test_case::test_case(true, "none", false; "v2 fresh")]
#[test_case::test_case(true, "all", false; "v2 fork")]
#[test_case::test_case(true, "none", true; "v2 fresh with raw events")]
#[tokio::test]
async fn subagents_inherit_spawn_metadata_without_live_parent_updates(
    multi_agent_v2: bool,
    fork_turns: &str,
    experimental_raw_events: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let spawn_args = if multi_agent_v2 {
        json!({"task_name": "worker", "message": "Inspect the example", "fork_turns": fork_turns})
    } else {
        json!({"message": "Inspect the example"})
    };
    let namespace = if multi_agent_v2 {
        "collaboration"
    } else {
        "multi_agent_v1"
    };
    let parent_wait = responses::ev_completed("parent-wait");
    let child_wait = responses::ev_response_created("child-spawn");
    let server = responses::start_websocket_server(vec![
        vec![
            vec![responses::ev_completed("parent-warm")],
            vec![
                responses::ev_function_call_with_namespace(
                    "spawn-worker",
                    namespace,
                    "spawn_agent",
                    &spawn_args.to_string(),
                ),
                responses::ev_completed("parent-spawn"),
            ],
            vec![parent_wait.clone()],
        ],
        vec![
            vec![responses::ev_completed("child-warm")],
            vec![
                child_wait.clone(),
                responses::ev_function_call_with_namespace(
                    "spawn-grandchild",
                    namespace,
                    "spawn_agent",
                    &spawn_args.to_string(),
                ),
                responses::ev_completed("child-spawn"),
            ],
            vec![responses::ev_completed("child-done")],
        ],
        vec![
            vec![responses::ev_completed("grandchild-warm")],
            vec![responses::ev_completed("grandchild-done")],
        ],
    ])
    .await;
    let _hold_parent = server.gate_response_event(&parent_wait);
    let release_child = server.gate_response_event(&child_wait);
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri().replacen("ws://", "http://", 1))
        .with_model(if multi_agent_v2 {
            "gpt-5.6-sol"
        } else {
            "gpt-5.6-luna"
        })
        .with_provider_config("supports_websockets = true")
        .with_provider_config("supports_standalone_web_search = true")
        .enable_feature(Feature::StandaloneWebSearch)
        .with_root_config("web_search = \"live\"")
        .with_extra_config(&format!(
            "[agents]\nmax_depth = 3\n[features.multi_agent_v2]\nenabled = {multi_agent_v2}"
        ))
        .write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            experimental_raw_events,
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(thread_req)).await??;
    let turn_req = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Delegate the example".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(HashMap::from([
                ("cost_center".to_string(), "before".to_string()),
                ("thread_id".to_string(), "spoofed-thread".to_string()),
                ("turn_id".to_string(), "spoofed-turn".to_string()),
                ("parent_response_id".to_string(), "root-only".to_string()),
                (
                    "mcp_request_meta".to_string(),
                    r#"{"selected_connector_ids":["parent-connector"]}"#.to_string(),
                ),
                (
                    "misalignment_override".to_string(),
                    r#"{"review_target":"root-only"}"#.to_string(),
                ),
            ])),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turn_req)).await??;
    let child_first = timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 1),
    )
    .await?;

    let steer_req = app_server
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            expected_turn_id: turn.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Update cost attribution".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(HashMap::from([(
                "cost_center".to_string(),
                "after".to_string(),
            )])),
            responsesapi_client_metadata_patch: None,
            additional_context: None,
        })
        .await?;
    let _: TurnSteerResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(steer_req)).await??;
    release_child.send(()).expect("child response is waiting");
    let child_next = timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 2),
    )
    .await?;
    let grandchild = timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request(/*connection_index*/ 2, /*request_index*/ 1),
    )
    .await?;

    let expected_exclusions = if experimental_raw_events {
        "tools"
    } else {
        "tools,usage.attribution"
    };
    for handshake in server.handshakes() {
        assert_eq!(
            handshake.header("x-codex-exclude").as_deref(),
            Some(expected_exclusions)
        );
    }
    for request in [&child_first, &grandchild] {
        let body = request.body_json();
        let tools = body["input"]
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .rev()
                    .find(|item| item["type"] == "additional_tools")
            })
            .unwrap_or(&body);
        assert!(tools.to_string().contains("### `web__run`"));
    }
    let metadata = [child_first, child_next, grandchild].map(|request| {
        let body = request.body_json();
        let metadata = parse_json_header(
            body["client_metadata"]["x-codex-turn-metadata"]
                .as_str()
                .expect("canonical turn metadata"),
        );
        assert_eq!(metadata["thread_id"], body["client_metadata"]["thread_id"]);
        assert_eq!(metadata["turn_id"], body["client_metadata"]["turn_id"]);
        assert_ne!(metadata["thread_id"], json!(thread.id));
        assert_ne!(metadata["turn_id"], json!(turn.id));
        assert_ne!(metadata["thread_id"], json!("spoofed-thread"));
        assert_ne!(metadata["turn_id"], json!("spoofed-turn"));
        metadata
    });
    assert_eq!(
        metadata.each_ref().map(|metadata| (
            metadata["cost_center"].as_str(),
            metadata["misalignment_override"].as_str(),
            metadata["parent_response_id"].as_str(),
            metadata["mcp_request_meta"].as_str(),
        )),
        [(Some("before"), None, None, None); 3],
    );
    assert_eq!(metadata[0]["turn_id"], metadata[1]["turn_id"]);
    assert_eq!(metadata[0]["parent_turn_id"], json!(turn.id));
    assert_eq!(metadata[2]["parent_turn_id"], metadata[0]["turn_id"]);
    assert_ne!(metadata[2]["thread_id"], metadata[0]["thread_id"]);
    assert_ne!(metadata[2]["turn_id"], metadata[0]["turn_id"]);
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
        (
            "parent_response_id".to_string(),
            "client-correlation".to_string(),
        ),
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
            turn_trigger: Some("user".to_string()),
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
    assert_eq!(
        (
            metadata["parent_response_id"].as_str(),
            request["client_metadata"].get("parent_response_id"),
        ),
        (Some("client-correlation"), None),
    );
    assert_eq!(metadata["fiber_run_id"].as_str(), Some("fiber-start-123"));
    assert_eq!(metadata["origin"].as_str(), Some("gaas"));
    assert_eq!(metadata["thread_source"].as_str(), Some("automation"));
    assert_eq!(metadata["turn_trigger"].as_str(), Some("user"));
    assert_eq!(metadata["turn_id"].as_str(), Some(turn.id.as_str()));
    assert!(metadata.get("session_id").is_some());
    assert_eq!(
        metadata["window_id"].as_str(),
        request["client_metadata"]["x-codex-window-id"].as_str()
    );
    assert!(
        metadata["context_window_id"]
            .as_str()
            .is_some_and(|window_id| uuid::Uuid::parse_str(window_id).is_ok())
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
