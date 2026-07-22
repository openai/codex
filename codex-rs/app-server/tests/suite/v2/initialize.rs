use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::fs_wait;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn initialize_uses_client_info_name_as_originator() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let expected_codex_home = AbsolutePathBuf::try_from(codex_home.path().canonicalize()?)?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse {
        user_agent,
        codex_home: response_codex_home,
        platform_family,
        platform_os,
    } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_vscode/"));
    assert_eq!(response_codex_home, expected_codex_home);
    assert_eq!(platform_family, std::env::consts::FAMILY);
    assert_eq!(platform_os, std::env::consts::OS);
    Ok(())
}

#[tokio::test]
async fn initialize_probe_does_not_override_originator() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_app_server_daemon".to_string(),
            title: Some("Codex App Server Daemon".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent, .. } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_cli_rs/"));
    Ok(())
}

#[tokio::test]
async fn initialize_codex_backend_does_not_override_originator() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex-backend".to_string(),
            title: Some("Codex Backend".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent, .. } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_cli_rs/"));
    Ok(())
}

#[tokio::test]
async fn initialize_respects_originator_override_env_var() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let expected_codex_home = AbsolutePathBuf::try_from(codex_home.path().canonicalize()?)?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[(
            "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
            Some("codex_originator_via_env_var"),
        )])
        .build()
        .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse {
        user_agent,
        codex_home: response_codex_home,
        platform_family,
        platform_os,
    } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_originator_via_env_var/"));
    assert_eq!(response_codex_home, expected_codex_home);
    assert_eq!(platform_family, std::env::consts::FAMILY);
    assert_eq!(platform_os, std::env::consts::OS);
    Ok(())
}

#[tokio::test]
async fn initialize_rejects_invalid_client_name() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("CODEX_INTERNAL_ORIGINATOR_OVERRIDE", None)])
        .build()
        .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "bad\rname".to_string(),
            title: Some("Bad Client".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Error(error) = message else {
        anyhow::bail!("expected initialize error, got {message:?}");
    };

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "Invalid clientInfo.name: 'bad\rname'. Must be a valid HTTP header value."
    );
    assert_eq!(error.error.data, None);
    Ok(())
}

#[tokio::test]
async fn initialize_opt_out_notification_methods_filters_notifications() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
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
                opt_out_notification_methods: Some(vec!["thread/started".to_string()]),
                mcp_server_openai_form_elicitation: false,
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(_) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };

    let request_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let message = mcp.read_next_message().await?;
            match message {
                JSONRPCMessage::Response(response)
                    if response.id == RequestId::Integer(request_id) =>
                {
                    return Ok(response);
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "thread/started" =>
                {
                    anyhow::bail!("thread/started should be filtered by optOutNotificationMethods");
                }
                _ => {}
            }
        }
    })
    .await??;
    let _: ThreadStartResponse = to_response(response)?;

    let thread_started = timeout(
        std::time::Duration::from_millis(500),
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await;
    assert!(
        thread_started.is_err(),
        "thread/started should be filtered by optOutNotificationMethods"
    );
    Ok(())
}

#[tokio::test]
async fn turn_start_notify_payload_includes_initialize_client_name() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let notify_file = codex_home.path().join("notify.json");
    let notify_capture = cargo_bin("codex-app-server-test-notify-capture")?;
    let notify_capture = notify_capture
        .to_str()
        .expect("notify capture path should be valid UTF-8");
    let notify_file_str = notify_file
        .to_str()
        .expect("notify file path should be valid UTF-8");
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!(
            "notify = [{}, {}]",
            toml_basic_string(notify_capture),
            toml_basic_string(notify_file_str)
        ))
        .disable_feature(Feature::ShellSnapshot)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "xcode".to_string(),
            title: Some("Xcode".to_string()),
            version: "1.0.0".to_string(),
        }),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    let payload_raw = tokio::fs::read_to_string(&notify_file).await?;
    let payload: Value = serde_json::from_str(&payload_raw)?;
    assert_eq!(payload["client"], "xcode");

    Ok(())
}

fn toml_basic_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
