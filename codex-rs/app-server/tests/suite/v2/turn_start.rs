use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_apply_patch_sse_response;
use app_test_support::create_exec_command_sse_response;
use app_test_support::create_fake_rollout;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::format_with_current_shell_display;
use app_test_support::to_response;
use codex_app_server_protocol::ByteRange;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeOutputDeltaNotification;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::TextElement;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::config::ConfigToml;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use codex_core::personality_migration::PERSONALITY_MIGRATION_FILENAME;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const TEST_ORIGINATOR: &str = "codex_vscode";
const LOCAL_PRAGMATIC_TEMPLATE: &str = "You are a deeply pragmatic, effective software engineer.";

#[tokio::test]
async fn turn_start_sends_originator_header() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: TEST_ORIGINATOR.to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = server
        .received_requests()
        .await
        .expect("failed to fetch received requests");
    assert!(!requests.is_empty());
    for request in requests {
        let originator = request
            .headers
            .get("originator")
            .expect("originator header missing");
        assert_eq!(originator.to_str()?, TEST_ORIGINATOR);
    }

    Ok(())
}

#[tokio::test]
async fn turn_start_emits_user_message_item_with_text_elements() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let text_elements = vec![TextElement::new(
        ByteRange { start: 0, end: 5 },
        Some("<note>".to_string()),
    )];
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: text_elements.clone(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;

    let user_message_item = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let params = notification.params.expect("item/started params");
            let item_started: ItemStartedNotification =
                serde_json::from_value(params).expect("deserialize item/started notification");
            if let ThreadItem::UserMessage { .. } = item_started.item {
                return Ok::<ThreadItem, anyhow::Error>(item_started.item);
            }
        }
    })
    .await??;

    match user_message_item {
        ThreadItem::UserMessage { content, .. } => {
            assert_eq!(
                content,
                vec![V2UserInput::Text {
                    text: "Hello".to_string(),
                    text_elements,
                }]
            );
        }
        other => panic!("expected user message item, got {other:?}"),
    }

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_emits_notifications_and_accepts_model_override() -> Result<()> {
    // Provide a mock server and config so model wiring is valid.
    // Three Codex turns hit the mock model (session start + two turn/start calls).
    let responses = vec![
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread (v2) and capture its id.
    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    // Start a turn with only input and thread_id set (no overrides).
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    assert!(!turn.id.is_empty());

    // Expect a turn/started notification.
    let notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    let started: TurnStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    assert_eq!(started.thread_id, thread.id);
    assert_eq!(
        started.turn.status,
        codex_app_server_protocol::TurnStatus::InProgress
    );

    // Send a second turn that exercises the overrides path: change the model.
    let turn_req2 = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Second".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model-override".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_resp2: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req2)),
    )
    .await??;
    let TurnStartResponse { turn: turn2 } = to_response::<TurnStartResponse>(turn_resp2)?;
    assert!(!turn2.id.is_empty());
    // Ensure the second turn has a different id than the first.
    assert_ne!(turn.id, turn2.id);

    // Expect a second turn/started notification as well.
    let _notif2: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    Ok(())
}

#[tokio::test]
async fn turn_start_accepts_collaboration_mode_override_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let collaboration_mode = CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "mock-model-collab".to_string(),
            reasoning_effort: Some(ReasoningEffort::High),
            developer_instructions: None,
        },
    };

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model-override".to_string()),
            effort: Some(ReasoningEffort::Low),
            summary: Some(ReasoningSummary::Auto),
            output_schema: None,
            collaboration_mode: Some(collaboration_mode),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let payload = request.body_json();
    assert_eq!(payload["model"].as_str(), Some("mock-model-collab"));
    let payload_text = payload.to_string();
    assert!(payload_text.contains("The `request_user_input` tool is unavailable in Default mode."));

    Ok(())
}

#[tokio::test]
async fn turn_start_accepts_personality_override_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("exp-codex-personality".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            personality: Some(Personality::Friendly),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let developer_texts = request.message_input_texts("developer");
    if developer_texts.is_empty() {
        eprintln!("request body: {}", request.body_json());
    }

    assert!(
        developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "expected personality update message in developer input, got {developer_texts:?}"
    );

    Ok(())
}

#[tokio::test]
async fn turn_start_change_personality_mid_thread_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let sse1 = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let sse2 = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]);
    let response_mock = responses::mount_sse_sequence(&server, vec![sse1, sse2]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("exp-codex-personality".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            personality: None,
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let turn_req2 = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello again".to_string(),
                text_elements: Vec::new(),
            }],
            personality: Some(Personality::Friendly),
            ..Default::default()
        })
        .await?;
    let turn_resp2: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req2)),
    )
    .await??;
    let _turn2: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp2)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2, "expected two requests");

    let first_developer_texts = requests[0].message_input_texts("developer");
    assert!(
        first_developer_texts
            .iter()
            .all(|text| !text.contains("<personality_spec>")),
        "expected no personality update message in first request, got {first_developer_texts:?}"
    );

    let second_developer_texts = requests[1].message_input_texts("developer");
    assert!(
        second_developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "expected personality update message in second request, got {second_developer_texts:?}"
    );

    Ok(())
}

#[tokio::test]
async fn turn_start_uses_migrated_pragmatic_personality_without_override_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::Personality, true)]),
    )?;
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-00-00",
        "2025-01-01T00:00:00Z",
        "history user message",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let persisted_toml: ConfigToml = toml::from_str(&std::fs::read_to_string(
        codex_home.path().join("config.toml"),
    )?)?;
    assert_eq!(persisted_toml.personality, Some(Personality::Pragmatic));
    assert!(
        codex_home
            .path()
            .join(PERSONALITY_MIGRATION_FILENAME)
            .exists(),
        "expected personality migration marker to be written on startup"
    );

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            personality: None,
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let instructions_text = request.instructions_text();
    assert!(
        instructions_text.contains(LOCAL_PRAGMATIC_TEMPLATE),
        "expected startup-migrated pragmatic personality in model instructions, got: {instructions_text:?}"
    );

    Ok(())
}

#[tokio::test]
async fn turn_start_accepts_local_image_input() -> Result<()> {
    // Two Codex turns hit the mock model (session start + turn/start).
    let responses = vec![
        create_final_assistant_message_sse_response("Done")?,
        create_final_assistant_message_sse_response("Done")?,
    ];
    // Use the unchecked variant because the request payload includes a LocalImage
    // which the strict matcher does not currently cover.
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let image_path = codex_home.path().join("image.png");
    // No need to actually write the file; we just exercise the input path.

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::LocalImage { path: image_path }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    assert!(!turn.id.is_empty());

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_exec_approval_toggle_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().to_path_buf();

    // Mock server: first turn requests a shell call (elicitation), then completes.
    // Second turn same, but we'll set approval_policy=never to avoid elicitation.
    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call1",
        )?,
        create_final_assistant_message_sse_response("done 1")?,
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call2",
        )?,
        create_final_assistant_message_sse_response("done 2")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    // Default approval is untrusted to force elicitation on first turn.
    create_config_toml(
        codex_home.as_path(),
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // thread/start
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    // turn/start — expect CommandExecutionRequestApproval request from server
    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    // Acknowledge RPC
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn_id)),
    )
    .await??;

    // Receive elicitation
    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "call1");

    // Approve and wait for task completion
    mcp.send_response(
        request_id,
        serde_json::json!({ "decision": codex_core::protocol::ReviewDecision::Approved }),
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    // Second turn with approval_policy=never should not elicit approval
    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python again".to_string(),
                text_elements: Vec::new(),
            }],
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn_id)),
    )
    .await??;

    // Ensure we do NOT receive a CommandExecutionRequestApproval request before task completes
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_exec_approval_decline_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().to_path_buf();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            None,
            Some(5000),
            "call-decline",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        codex_home.as_path(),
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let started_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::CommandExecution { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution { id, status, .. } = started_command_execution else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-decline");
    assert_eq!(status, CommandExecutionStatus::InProgress);

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req else {
        panic!("expected CommandExecutionRequestApproval request")
    };
    assert_eq!(params.item_id, "call-decline");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Decline,
        })?,
    )
    .await?;

    let completed_command_execution = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::CommandExecution { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        id,
        status,
        exit_code,
        aggregated_output,
        ..
    } = completed_command_execution
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "call-decline");
    assert_eq!(status, CommandExecutionStatus::Declined);
    assert!(exit_code.is_none());
    assert!(aggregated_output.is_none());

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_updates_sandbox_and_cwd_between_turns_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir(&workspace_root)?;
    let first_cwd = workspace_root.join("turn1");
    let second_cwd = workspace_root.join("turn2");
    std::fs::create_dir(&first_cwd)?;
    std::fs::create_dir(&second_cwd)?;

    let responses = vec![
        create_shell_command_sse_response(
            vec!["echo".to_string(), "first".to_string(), "turn".to_string()],
            None,
            Some(5000),
            "call-first",
        )?,
        create_final_assistant_message_sse_response("done first")?,
        create_shell_command_sse_response(
            vec!["echo".to_string(), "second".to_string(), "turn".to_string()],
            None,
            Some(5000),
            "call-second",
        )?,
        create_final_assistant_message_sse_response("done second")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // thread/start
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    // first turn with workspace-write sandbox and first_cwd
    let first_turn = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(first_cwd.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![first_cwd.try_into()?],
                read_only_access: codex_app_server_protocol::ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
            personality: None,
            output_schema: None,
            collaboration_mode: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    mcp.clear_message_buffer();

    // second turn with workspace-write and second_cwd, ensure exec begins in second_cwd
    let second_turn = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(second_cwd.clone()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            summary: Some(ReasoningSummary::Auto),
            personality: None,
            output_schema: None,
            collaboration_mode: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn)),
    )
    .await??;

    let command_exec_item = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let item_started_notification = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let params = item_started_notification
                .params
                .clone()
                .expect("item/started params");
            let item_started: ItemStartedNotification =
                serde_json::from_value(params).expect("deserialize item/started notification");
            if matches!(item_started.item, ThreadItem::CommandExecution { .. }) {
                return Ok::<ThreadItem, anyhow::Error>(item_started.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        cwd,
        command,
        status,
        ..
    } = command_exec_item
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(cwd, second_cwd);
    let expected_command = format_with_current_shell_display("echo second turn");
    assert_eq!(command, expected_command);
    assert_eq!(status, CommandExecutionStatus::InProgress);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn turn_start_file_change_approval_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let patch = r#"*** Begin Patch
*** Add File: README.md
+new line
*** End Patch
"#;
    let responses = vec![
        create_apply_patch_sse_response(patch, "patch-call")?,
        create_final_assistant_message_sse_response("patch applied")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "apply patch".into(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let started_file_change = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::FileChange { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange {
        ref id,
        status,
        ref changes,
    } = started_file_change
    else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call");
    assert_eq!(status, PatchApplyStatus::InProgress);
    let started_changes = changes.clone();

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::FileChangeRequestApproval { request_id, params } = server_req else {
        panic!("expected FileChangeRequestApproval request")
    };
    assert_eq!(params.item_id, "patch-call");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);
    let expected_readme_path = workspace.join("README.md");
    let expected_readme_path = expected_readme_path.to_string_lossy().into_owned();
    pretty_assertions::assert_eq!(
        started_changes,
        vec![codex_app_server_protocol::FileUpdateChange {
            path: expected_readme_path.clone(),
            kind: PatchChangeKind::Add,
            diff: "new line\n".to_string(),
        }]
    );

    mcp.send_response(
        request_id,
        serde_json::to_value(FileChangeRequestApprovalResponse {
            decision: FileChangeApprovalDecision::Accept,
        })?,
    )
    .await?;

    let output_delta_notif = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("item/fileChange/outputDelta"),
    )
    .await??;
    let output_delta: FileChangeOutputDeltaNotification = serde_json::from_value(
        output_delta_notif
            .params
            .clone()
            .expect("item/fileChange/outputDelta params"),
    )?;
    assert_eq!(output_delta.thread_id, thread.id);
    assert_eq!(output_delta.turn_id, turn.id);
    assert_eq!(output_delta.item_id, "patch-call");
    assert!(
        !output_delta.delta.is_empty(),
        "expected delta to be non-empty, got: {}",
        output_delta.delta
    );

    let completed_file_change = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::FileChange { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange { ref id, status, .. } = completed_file_change else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call");
    assert_eq!(status, PatchApplyStatus::Completed);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let readme_contents = std::fs::read_to_string(expected_readme_path)?;
    assert_eq!(readme_contents, "new line\n");

    Ok(())
}

#[tokio::test]
async fn turn_start_file_change_approval_accept_for_session_persists_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let patch_1 = r#"*** Begin Patch
*** Add File: README.md
+new line
*** End Patch
"#;
    let patch_2 = r#"*** Begin Patch
*** Update File: README.md
@@
-new line
+updated line
*** End Patch
"#;

    let responses = vec![
        create_apply_patch_sse_response(patch_1, "patch-call-1")?,
        create_final_assistant_message_sse_response("patch 1 applied")?,
        create_apply_patch_sse_response(patch_2, "patch-call-2")?,
        create_final_assistant_message_sse_response("patch 2 applied")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    // First turn: expect FileChangeRequestApproval, respond with AcceptForSession, and verify the file exists.
    let turn_1_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "apply patch 1".into(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    let turn_1_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_1_req)),
    )
    .await??;
    let TurnStartResponse { turn: turn_1 } = to_response::<TurnStartResponse>(turn_1_resp)?;

    let started_file_change_1 = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::FileChange { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange { id, status, .. } = started_file_change_1 else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call-1");
    assert_eq!(status, PatchApplyStatus::InProgress);

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::FileChangeRequestApproval { request_id, params } = server_req else {
        panic!("expected FileChangeRequestApproval request")
    };
    assert_eq!(params.item_id, "patch-call-1");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn_1.id);

    mcp.send_response(
        request_id,
        serde_json::to_value(FileChangeRequestApprovalResponse {
            decision: FileChangeApprovalDecision::AcceptForSession,
        })?,
    )
    .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("item/fileChange/outputDelta"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("item/completed"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let readme_path = workspace.join("README.md");
    assert_eq!(std::fs::read_to_string(&readme_path)?, "new line\n");

    // Second turn: apply a patch to the same file. Approval should be skipped due to AcceptForSession.
    let turn_2_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "apply patch 2".into(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_2_req)),
    )
    .await??;

    let started_file_change_2 = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::FileChange { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange { id, status, .. } = started_file_change_2 else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call-2");
    assert_eq!(status, PatchApplyStatus::InProgress);

    // If the server incorrectly emits FileChangeRequestApproval, the helper below will error
    // (it bails on unexpected JSONRPCMessage::Request), causing the test to fail.
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("item/fileChange/outputDelta"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("item/completed"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    assert_eq!(std::fs::read_to_string(readme_path)?, "updated line\n");

    Ok(())
}

#[tokio::test]
async fn turn_start_file_change_approval_decline_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let patch = r#"*** Begin Patch
*** Add File: README.md
+new line
*** End Patch
"#;
    let responses = vec![
        create_apply_patch_sse_response(patch, "patch-call")?,
        create_final_assistant_message_sse_response("patch declined")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        &codex_home,
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "apply patch".into(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let started_file_change = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let started_notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(started_notif.params.clone().expect("item/started params"))?;
            if let ThreadItem::FileChange { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange {
        ref id,
        status,
        ref changes,
    } = started_file_change
    else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call");
    assert_eq!(status, PatchApplyStatus::InProgress);
    let started_changes = changes.clone();

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::FileChangeRequestApproval { request_id, params } = server_req else {
        panic!("expected FileChangeRequestApproval request")
    };
    assert_eq!(params.item_id, "patch-call");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);
    let expected_readme_path = workspace.join("README.md");
    let expected_readme_path_str = expected_readme_path.to_string_lossy().into_owned();
    pretty_assertions::assert_eq!(
        started_changes,
        vec![codex_app_server_protocol::FileUpdateChange {
            path: expected_readme_path_str.clone(),
            kind: PatchChangeKind::Add,
            diff: "new line\n".to_string(),
        }]
    );

    mcp.send_response(
        request_id,
        serde_json::to_value(FileChangeRequestApprovalResponse {
            decision: FileChangeApprovalDecision::Decline,
        })?,
    )
    .await?;

    let completed_file_change = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let completed_notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                completed_notif
                    .params
                    .clone()
                    .expect("item/completed params"),
            )?;
            if let ThreadItem::FileChange { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::FileChange { ref id, status, .. } = completed_file_change else {
        unreachable!("loop ensures we break on file change items");
    };
    assert_eq!(id, "patch-call");
    assert_eq!(status, PatchApplyStatus::Declined);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    assert!(
        !expected_readme_path.exists(),
        "declined patch should not be applied"
    );

    Ok(())
}

#[tokio::test]
#[cfg_attr(windows, ignore = "process id reporting differs on Windows")]
async fn command_execution_notifications_include_process_id() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses = vec![
        create_exec_command_sse_response("uexec-1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml_with_sandbox(
        codex_home.path(),
        &server.uri(),
        "never",
        &BTreeMap::from([(Feature::UnifiedExec, true)]),
        "danger-full-access",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run a command".to_string(),
                text_elements: Vec::new(),
            }],
            sandbox_policy: Some(codex_app_server_protocol::SandboxPolicy::DangerFullAccess),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let TurnStartResponse { turn: _turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let started_command = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notif = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification = serde_json::from_value(
                notif
                    .params
                    .clone()
                    .expect("item/started should include params"),
            )?;
            if let ThreadItem::CommandExecution { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        id,
        process_id: started_process_id,
        status,
        ..
    } = started_command
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(id, "uexec-1");
    assert_eq!(status, CommandExecutionStatus::InProgress);
    let started_process_id = started_process_id.expect("process id should be present");

    let completed_command = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notif = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                notif
                    .params
                    .clone()
                    .expect("item/completed should include params"),
            )?;
            if let ThreadItem::CommandExecution { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        id: completed_id,
        process_id: completed_process_id,
        status: completed_status,
        exit_code,
        ..
    } = completed_command
    else {
        unreachable!("loop ensures we break on command execution items");
    };
    assert_eq!(completed_id, "uexec-1");
    assert!(
        matches!(
            completed_status,
            CommandExecutionStatus::Completed | CommandExecutionStatus::Failed
        ),
        "unexpected command execution status: {completed_status:?}"
    );
    if completed_status == CommandExecutionStatus::Completed {
        assert_eq!(exit_code, Some(0));
    } else {
        assert!(exit_code.is_some(), "expected exit_code for failed command");
    }
    assert_eq!(
        completed_process_id.as_deref(),
        Some(started_process_id.as_str())
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
    feature_flags: &BTreeMap<Feature, bool>,
) -> std::io::Result<()> {
    create_config_toml_with_sandbox(
        codex_home,
        server_uri,
        approval_policy,
        feature_flags,
        "read-only",
    )
}

fn create_config_toml_with_sandbox(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
    feature_flags: &BTreeMap<Feature, bool>,
    sandbox_mode: &str,
) -> std::io::Result<()> {
    let mut features = BTreeMap::new();
    for (feature, enabled) in feature_flags {
        features.insert(*feature, *enabled);
    }
    let feature_entries = features
        .into_iter()
        .map(|(feature, enabled)| {
            let key = FEATURES
                .iter()
                .find(|spec| spec.id == feature)
                .map(|spec| spec.key)
                .unwrap_or_else(|| panic!("missing feature key for {feature:?}"));
            format!("{key} = {enabled}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "{sandbox_mode}"

model_provider = "mock_provider"

[features]
{feature_entries}

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
