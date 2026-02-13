use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_request_user_input_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoadedThreadStatus;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadTerminalOutcome;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::ThreadWatchParams;
use codex_app_server_protocol::ThreadWatchResponse;
use codex_app_server_protocol::ThreadWatchUpdate;
use codex_app_server_protocol::ThreadWatchUpdatedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thread_watch_snapshot_and_runtime_updates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![
        create_request_user_input_sse_response("watch-call-1")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

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

    let watch_request_id = mcp.send_thread_watch_request(ThreadWatchParams {}).await?;
    let watch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
    )
    .await??;
    let ThreadWatchResponse {
        snapshot_version,
        data,
    } = to_response(watch_resp)?;
    assert!(
        snapshot_version >= 1,
        "snapshotVersion should include prior loaded-thread state changes"
    );
    assert_eq!(data.len(), 1);
    let entry = data.first().expect("snapshot should include one thread");
    assert_eq!(entry.thread.id, thread.id);
    assert_eq!(entry.status, LoadedThreadStatus::Idle);

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "collect watch updates".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Plan,
                settings: Settings {
                    model: "mock-model".to_string(),
                    reasoning_effort: Some(ReasoningEffort::Medium),
                    developer_instructions: None,
                },
            }),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_start_resp)?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = server_req else {
        panic!("expected ToolRequestUserInput request, got: {server_req:?}");
    };

    mcp.send_response(
        request_id,
        serde_json::json!({
            "answers": {
                "confirm_path": { "answers": ["yes"] }
            }
        }),
    )
    .await?;

    let mut saw_waiting_user_input = false;
    let mut saw_terminal_completed = false;
    for _ in 0..40 {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Notification(JSONRPCNotification {
                method,
                params: Some(params),
            }) if method == "thread/watch/updated" => {
                let notification: ThreadWatchUpdatedNotification = serde_json::from_value(params)?;
                if let ThreadWatchUpdate::Upsert { entry, .. } = notification.update {
                    if entry.thread.id != thread.id {
                        continue;
                    }
                    match entry.status {
                        LoadedThreadStatus::Active { active_flags } => {
                            saw_waiting_user_input |=
                                active_flags.contains(&ThreadActiveFlag::WaitingUserInput);
                        }
                        LoadedThreadStatus::Terminal { outcome } => {
                            if outcome == ThreadTerminalOutcome::Completed {
                                saw_terminal_completed = true;
                            }
                        }
                        LoadedThreadStatus::Idle => {}
                    }
                }
            }
            _ => {}
        }

        if saw_waiting_user_input && saw_terminal_completed {
            break;
        }
    }

    assert!(
        saw_waiting_user_input,
        "expected waitingUserInput active flag in thread/watch updates"
    );
    assert!(
        saw_terminal_completed,
        "expected terminal completed status in thread/watch updates"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thread_watch_snapshot_includes_midflight_waiting_user_input_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![
        create_request_user_input_sse_response("watch-call-2")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

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

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "collect watch snapshot after turn starts".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Plan,
                settings: Settings {
                    model: "mock-model".to_string(),
                    reasoning_effort: Some(ReasoningEffort::Medium),
                    developer_instructions: None,
                },
            }),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_start_resp)?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = server_req else {
        panic!("expected ToolRequestUserInput request, got: {server_req:?}");
    };

    let watch_request_id = mcp.send_thread_watch_request(ThreadWatchParams {}).await?;
    let watch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
    )
    .await??;
    let ThreadWatchResponse {
        snapshot_version,
        data,
    } = to_response(watch_resp)?;
    assert!(
        snapshot_version >= 1,
        "snapshotVersion should include prior loaded-thread state changes"
    );
    assert_eq!(data.len(), 1);
    let entry = data.first().expect("snapshot should include one thread");
    assert_eq!(entry.thread.id, thread.id);
    match &entry.status {
        LoadedThreadStatus::Active { active_flags } => {
            assert!(
                active_flags.contains(&ThreadActiveFlag::WaitingUserInput),
                "expected waitingUserInput in snapshot when watch attaches mid-flight"
            );
        }
        status => {
            panic!("expected Active status when turn is waiting for user input, got {status:?}")
        }
    }

    mcp.send_response(
        request_id,
        serde_json::json!({
            "answers": {
                "confirm_path": { "answers": ["yes"] }
            }
        }),
    )
    .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thread_watch_emits_remove_when_thread_is_archived() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

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

    let watch_request_id = mcp.send_thread_watch_request(ThreadWatchParams {}).await?;
    let watch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
    )
    .await??;
    let _: ThreadWatchResponse = to_response(watch_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "materialize rollout".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_start_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let archive_request_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let archive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_request_id)),
    )
    .await??;
    let _: ThreadArchiveResponse = to_response(archive_resp)?;

    let mut saw_remove = false;
    for _ in 0..40 {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Notification(JSONRPCNotification {
                method,
                params: Some(params),
            }) if method == "thread/watch/updated" => {
                let notification: ThreadWatchUpdatedNotification = serde_json::from_value(params)?;
                if let ThreadWatchUpdate::Remove { thread_id } = notification.update
                    && thread_id == thread.id
                {
                    saw_remove = true;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(
        saw_remove,
        "expected thread/watch remove update after thread/archive"
    );

    Ok(())
}

#[tokio::test]
async fn thread_watch_emits_upsert_after_archive_unarchive_resume() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

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

    let watch_request_id = mcp.send_thread_watch_request(ThreadWatchParams {}).await?;
    let watch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
    )
    .await??;
    let _: ThreadWatchResponse = to_response(watch_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "materialize rollout".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_start_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let archive_request_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let archive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_request_id)),
    )
    .await??;
    let _: ThreadArchiveResponse = to_response(archive_resp)?;

    let mut remove_version = None;
    for _ in 0..40 {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Notification(JSONRPCNotification {
                method,
                params: Some(params),
            }) if method == "thread/watch/updated" => {
                let notification: ThreadWatchUpdatedNotification = serde_json::from_value(params)?;
                if let ThreadWatchUpdate::Remove { thread_id } = notification.update
                    && thread_id == thread.id
                {
                    remove_version = Some(notification.version);
                    break;
                }
            }
            _ => {}
        }
    }
    let remove_version =
        remove_version.expect("expected thread/watch remove update after thread/archive");

    let unarchive_request_id = mcp
        .send_thread_unarchive_request(ThreadUnarchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let unarchive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unarchive_request_id)),
    )
    .await??;
    let ThreadUnarchiveResponse {
        thread: unarchived_thread,
    } = to_response(unarchive_resp)?;
    assert_eq!(unarchived_thread.id, thread.id);

    let resume_request_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_request_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = to_response(resume_resp)?;
    assert_eq!(resumed_thread.id, thread.id);

    let mut upsert_version = None;
    for _ in 0..40 {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        match message {
            JSONRPCMessage::Notification(JSONRPCNotification {
                method,
                params: Some(params),
            }) if method == "thread/watch/updated" => {
                let notification: ThreadWatchUpdatedNotification = serde_json::from_value(params)?;
                if let ThreadWatchUpdate::Upsert { entry } = notification.update
                    && entry.thread.id == thread.id
                {
                    upsert_version = Some(notification.version);
                    break;
                }
            }
            _ => {}
        }
    }
    let upsert_version = upsert_version
        .expect("expected thread/watch upsert update after thread/unarchive + thread/resume");
    assert!(
        upsert_version > remove_version,
        "upsert version should follow remove version, got upsert={upsert_version} remove={remove_version}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_watch_updated_can_be_opted_out() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_capabilities(
            ClientInfo {
                name: "codex_vscode".to_string(),
                title: Some("Codex VS Code Extension".to_string()),
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: Some(vec!["thread/watch/updated".to_string()]),
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(_) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };

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

    let watch_request_id = mcp.send_thread_watch_request(ThreadWatchParams {}).await?;
    let watch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
    )
    .await??;
    let _: ThreadWatchResponse = to_response(watch_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "run once".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_start_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let watch_update = timeout(
        std::time::Duration::from_millis(500),
        mcp.read_stream_until_notification_message("thread/watch/updated"),
    )
    .await;
    match watch_update {
        Err(_) => {}
        Ok(Ok(notification)) => {
            anyhow::bail!(
                "thread/watch/updated should be filtered by optOutNotificationMethods; got: {notification:?}"
            );
        }
        Ok(Err(err)) => {
            anyhow::bail!("expected timeout waiting for filtered thread/watch/updated, got: {err}");
        }
    }

    Ok(())
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
collaboration_modes = true

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
