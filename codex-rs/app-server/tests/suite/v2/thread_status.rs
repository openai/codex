use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thread_status_changed_emits_runtime_updates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("untrusted")
        .enable_feature(Feature::CollaborationModes)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("RUST_LOG", Some("info"))])
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "collect status updates".to_string(),
                    text_elements: Vec::new(),
                }],
                model: Some("mock-model".to_string()),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_active_running = false;
    let mut saw_idle_after_turn = false;
    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = match timeout(remaining, mcp.read_next_message()).await {
            Ok(Ok(message)) => message,
            _ => break,
        };
        match message {
            JSONRPCMessage::Notification(JSONRPCNotification {
                method,
                params: Some(params),
                ..
            }) if method == "thread/status/changed" => {
                let notification: ThreadStatusChangedNotification = serde_json::from_value(params)?;
                if notification.thread_id != thread.id {
                    continue;
                }
                match notification.status {
                    ThreadStatus::Active { .. } => {
                        saw_active_running = true;
                    }
                    ThreadStatus::Idle => {
                        if saw_active_running {
                            saw_idle_after_turn = true;
                        }
                    }
                    ThreadStatus::SystemError => {
                        if saw_active_running {
                            saw_idle_after_turn = true;
                        }
                    }
                    ThreadStatus::NotLoaded => {
                        if saw_active_running {
                            saw_idle_after_turn = true;
                        }
                    }
                }
            }
            _ => {}
        }

        if saw_active_running && saw_idle_after_turn {
            break;
        }
    }

    assert!(
        saw_active_running,
        "expected running active flag in thread/status/changed notifications"
    );
    assert!(
        saw_idle_after_turn,
        "expected idle status after turn completion in thread/status/changed notifications"
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_status_changed_can_be_opted_out() -> Result<()> {
    let codex_home = TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("untrusted")
        .enable_feature(Feature::CollaborationModes)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
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
                request_attestation: false,
                opt_out_notification_methods: Some(vec!["thread/status/changed".to_string()]),
                mcp_server_openai_form_elicitation: false,
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(_) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "run once".to_string(),
                    text_elements: Vec::new(),
                }],
                model: Some("mock-model".to_string()),
                ..Default::default()
            },
        })
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let status_update = timeout(
        std::time::Duration::from_millis(500),
        mcp.read_stream_until_notification_message("thread/status/changed"),
    )
    .await;
    match status_update {
        Err(_) => {}
        Ok(Ok(notification)) => {
            anyhow::bail!(
                "thread/status/changed should be filtered by optOutNotificationMethods; got: {notification:?}"
            );
        }
        Ok(Err(err)) => {
            anyhow::bail!(
                "expected timeout waiting for filtered thread/status/changed, got: {err}"
            );
        }
    }

    Ok(())
}
