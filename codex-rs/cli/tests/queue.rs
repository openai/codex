use std::path::Path;
use std::process::Output;

use anyhow::Context;
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const THREAD_ID: &str = "123e4567-e89b-12d3-a456-426614174000";

enum QueueResponse {
    Success,
    MethodNotFound,
    #[cfg(unix)]
    UnknownMethodVariant,
}

#[cfg(unix)]
fn socket_test_home() -> Result<TempDir> {
    // macOS temporary paths can exceed the Unix socket path limit.
    #[cfg(target_os = "macos")]
    let home = tempfile::tempdir_in("/tmp")?;
    #[cfg(not(target_os = "macos"))]
    let home = TempDir::new()?;
    Ok(home)
}

async fn respond_to_queue_request<S>(
    stream: S,
    codex_home: &Path,
    response: QueueResponse,
) -> Result<Value>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut websocket = accept_async(stream).await?;
    let initialize = websocket
        .next()
        .await
        .context("missing initialize request")??;
    let initialize: Value = serde_json::from_str(initialize.to_text()?)?;
    assert_eq!(initialize["method"], "initialize");
    assert_eq!(
        initialize["params"]["capabilities"]["experimentalApi"],
        true
    );
    let initialized_response = json!({
        "id": initialize["id"],
        "result": {
            "userAgent": "codex_cli_rs/0.0.0-test",
            "codexHome": codex_home,
        },
    });
    websocket
        .send(Message::Text(initialized_response.to_string().into()))
        .await?;

    let initialized = websocket
        .next()
        .await
        .context("missing initialized notification")??;
    let initialized: Value = serde_json::from_str(initialized.to_text()?)?;
    assert_eq!(initialized["method"], "initialized");

    let request = websocket.next().await.context("missing queue request")??;
    let request: Value = serde_json::from_str(request.to_text()?)?;
    assert_eq!(request["method"], "thread/queue/add");

    let result = match response {
        QueueResponse::Success => json!({
            "id": request["id"],
            "result": {
                "queuedSubmission": {
                    "id": "queued-submission-id",
                    "input": request["params"]["input"],
                    "clientUserMessageId": request["params"]["clientUserMessageId"],
                },
            },
        }),
        QueueResponse::MethodNotFound => json!({
            "id": request["id"],
            "error": { "code": -32601, "message": "Method not found" },
        }),
        #[cfg(unix)]
        QueueResponse::UnknownMethodVariant => json!({
            "id": request["id"],
            "error": {
                "code": -32600,
                "message": "Invalid request: unknown variant `thread/queue/add`, expected `thread/list`",
            },
        }),
    };
    websocket
        .send(Message::Text(result.to_string().into()))
        .await?;
    Ok(request)
}

#[tokio::test]
async fn queue_submits_message_to_remote_app_server() -> Result<()> {
    let (output, request) = run_remote_queue_command(QueueResponse::Success).await?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request["params"]["threadId"], THREAD_ID);
    assert_eq!(request["params"]["input"][0]["text"], "do the thing");
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("Queued message queued-submission-id for thread {THREAD_ID}.\n")
    );
    Ok(())
}

#[tokio::test]
async fn queue_does_not_fallback_from_unsupported_explicit_remote() -> Result<()> {
    let (output, _) = run_remote_queue_command(QueueResponse::MethodNotFound).await?;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("remote app server does not support thread/queue/add")
    );
    Ok(())
}

#[test]
fn queue_rejects_empty_message() -> Result<()> {
    let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["queue", "--thread", THREAD_ID, "--message", ""])
        .output()?;
    assert!(!output.status.success());
    Ok(())
}

#[test]
fn queue_rejects_image_attachments() -> Result<()> {
    let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args([
            "queue",
            "--thread",
            THREAD_ID,
            "--message",
            "do the thing",
            "--image",
            "screenshot.png",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("does not support image attachments"));
    Ok(())
}

async fn run_remote_queue_command(response: QueueResponse) -> Result<(Output, Value)> {
    let codex_home = TempDir::new()?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("ws://{}", listener.local_addr()?);
    let remote_first = matches!(response, QueueResponse::MethodNotFound);
    let server_home = codex_home.path().to_path_buf();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        respond_to_queue_request(stream, server_home.as_path(), response).await
    });

    let remote_args = if remote_first {
        [
            "--remote",
            endpoint.as_str(),
            "--remote-auth-token-env",
            "CODEX_REMOTE_TOKEN",
            "queue",
        ]
    } else {
        [
            "queue",
            "--remote",
            endpoint.as_str(),
            "--remote-auth-token-env",
            "CODEX_REMOTE_TOKEN",
        ]
    };
    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_REMOTE_TOKEN", "test-token")
        .args(remote_args)
        .args(["--thread", THREAD_ID, "--message", "do the thing"])
        .output()
        .await?;
    Ok((output, server.await??))
}

#[cfg(unix)]
#[tokio::test]
async fn remote_session_commands_with_workload_identity_use_server_auth() -> Result<()> {
    use std::time::Duration;

    use app_test_support::MockResponsesConfig;
    use app_test_support::TestAppServer;
    use app_test_support::create_final_assistant_message_sse_response;
    use app_test_support::create_mock_responses_server_sequence_unchecked;
    use codex_app_server_client::AppServerEvent;
    use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
    use codex_app_server_client::RemoteAppServerClient;
    use codex_app_server_client::RemoteAppServerConnectArgs;
    use codex_app_server_client::RemoteAppServerEndpoint;
    use codex_app_server_protocol::ClientRequest;
    use codex_app_server_protocol::RequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ThreadListParams;
    use codex_app_server_protocol::ThreadListResponse;
    use codex_app_server_protocol::ThreadQueueListParams;
    use codex_app_server_protocol::ThreadQueueListResponse;
    use codex_app_server_protocol::ThreadSetNameParams;
    use codex_app_server_protocol::ThreadSetNameResponse;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_app_server_protocol::ThreadStartResponse;
    use codex_app_server_protocol::TurnStatus;
    use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
    use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
    use tokio::process::Command;
    use tokio::time::timeout;

    let codex_home = socket_test_home()?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let model = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("queued message consumed")?,
    ])
    .await;
    MockResponsesConfig::new(&model.uri())
        .with_root_config("features.plugins = false\nanalytics.enabled = false")
        .with_provider_config("env_key = \"CODEX_QUEUE_SERVER_API_KEY\"")
        .write(codex_home.path())?;
    let _server = TestAppServer::builder()
        .with_program(&codex)
        .with_codex_home(codex_home.path())
        .with_plugin_startup_tasks()
        .without_managed_config()
        .with_args(&["app-server", "--listen", "unix://"])
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            ("CODEX_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
            (OPENAI_FEDERATION_RULE_ID_ENV_VAR, None),
            (OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR, None),
            ("CODEX_QUEUE_SERVER_API_KEY", Some("server-only-test-key")),
        ])
        .build()
        .await?;
    let socket_path = codex_app_server::app_server_control_socket_path(codex_home.path())?;
    timeout(Duration::from_secs(30), async {
        while !socket_path.as_path().try_exists()? {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    let endpoint = format!("unix://{}", socket_path.as_path().display());
    let mut app = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
        client_name: "queue-e2e-test".to_string(),
        client_version: "0.1.0".to_string(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;
    let started: ThreadStartResponse = app
        .request_typed(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams::default(),
        })
        .await?;
    let thread_id = started.thread.id;
    let message = "synthetic WIF queue end-to-end message";
    let output = timeout(
        Duration::from_secs(30),
        Command::new(&codex)
            .env("CODEX_HOME", codex_home.path())
            .env(OPENAI_FEDERATION_RULE_ID_ENV_VAR, "rule-test")
            .env(
                OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
                codex_home.path().join("missing-identity-token"),
            )
            .env_remove("CODEX_QUEUE_SERVER_API_KEY")
            .kill_on_drop(true)
            .args(["queue", "--remote", "unix://"])
            .args(["--thread", &thread_id, "--message", message])
            .output(),
    )
    .await??;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let completed = timeout(Duration::from_secs(30), async {
        while let Some(event) = app.next_event().await {
            if let AppServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(completed) = *notification
            {
                return Ok::<_, anyhow::Error>(completed);
            }
        }
        anyhow::bail!("app server disconnected before the queued turn completed")
    })
    .await??;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let queue: ThreadQueueListResponse = app
        .request_typed(ClientRequest::ThreadQueueList {
            request_id: RequestId::Integer(2),
            params: ThreadQueueListParams {
                thread_id: thread_id.clone(),
                cursor: None,
                limit: None,
            },
        })
        .await?;
    assert!(queue.data.is_empty());

    let requests = model
        .received_requests()
        .await
        .context("model request capture unavailable")?;
    let request = requests
        .iter()
        .find(|request| request.url.path().ends_with("/responses"))
        .context("queued message did not reach the model")?;
    assert!(
        request.body_json::<Value>()?["input"]
            .to_string()
            .contains(message)
    );
    assert_eq!(
        request.headers["authorization"].to_str()?,
        "Bearer server-only-test-key"
    );
    let metadata: Value = serde_json::from_str(request.headers["x-codex-turn-metadata"].to_str()?)?;
    assert_eq!(metadata["turn_trigger"], "queue");

    let name = "remote-session-lifecycle";
    let _: ThreadSetNameResponse = app
        .request_typed(ClientRequest::ThreadSetName {
            request_id: RequestId::Integer(3),
            params: ThreadSetNameParams {
                thread_id: thread_id.clone(),
                name: name.to_string(),
            },
        })
        .await?;
    // A separate caller home makes name resolution rely on the selected server.
    let caller_home = TempDir::new()?;
    let remote_command = || {
        let mut command = Command::new(&codex);
        command
            .env("CODEX_HOME", caller_home.path())
            .current_dir(caller_home.path())
            .env(OPENAI_FEDERATION_RULE_ID_ENV_VAR, "rule-test")
            .env(
                OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
                caller_home.path().join("missing-identity-token"),
            )
            .env_remove("CODEX_QUEUE_SERVER_API_KEY")
            .kill_on_drop(true)
            .args(["--remote", &endpoint]);
        command
    };
    let output = timeout(
        Duration::from_secs(30),
        remote_command().args(["delete", &thread_id]).output(),
    )
    .await??;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("cannot confirm session deletion"));

    let cases: &[(&[&str], Option<bool>)] = &[
        (&["archive", name], Some(true)),
        (&["unarchive", name], Some(false)),
        (&["delete", "--force", &thread_id], None),
    ];
    for &(args, expected_archived) in cases {
        let output = timeout(
            Duration::from_secs(30),
            remote_command().args(args).output(),
        )
        .await??;
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for archived in [false, true] {
            let threads: ThreadListResponse = app
                .request_typed(ClientRequest::ThreadList {
                    request_id: RequestId::Integer(4),
                    params: ThreadListParams {
                        cursor: None,
                        limit: None,
                        sort_key: None,
                        sort_direction: None,
                        model_providers: None,
                        source_kinds: None,
                        originators: None,
                        archived: Some(archived),
                        section_id: None,
                        project_id: None,
                        cwd: None,
                        use_state_db_only: false,
                        search_term: None,
                        parent_thread_id: None,
                        ancestor_thread_id: None,
                    },
                })
                .await?;
            assert_eq!(
                threads.data.iter().any(|thread| thread.id == thread_id),
                expected_archived == Some(archived),
                "{args:?}: archived={archived}"
            );
        }
    }
    Ok(())
}

#[test]
fn remote_session_commands_validate_config() -> Result<()> {
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["--strict-config"],
            "config.toml",
            "unknown_queue_option = true\n",
        ),
        (&["--profile", "queue"], "queue.config.toml", "model = 42\n"),
        (
            &[],
            "config.toml",
            "experimental_thread_store_endpoint = \"https://example.com\"\n",
        ),
    ];
    let commands: &[&[&str]] = &[
        &["queue", "--thread", THREAD_ID, "--message", "do the thing"],
        &["archive", THREAD_ID],
        &["unarchive", THREAD_ID],
        &["delete", "--force", THREAD_ID],
    ];
    for &command in commands {
        for &(args, config_file, config) in cases {
            let codex_home = TempDir::new()?;
            std::fs::write(codex_home.path().join(config_file), config)?;
            let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
                .env("CODEX_HOME", codex_home.path())
                .current_dir(codex_home.path())
                .args(args)
                .args(["--remote", "ws://127.0.0.1:1"])
                .args(command)
                .output()?;
            let stderr = String::from_utf8(output.stderr)?;
            assert!(!output.status.success());
            assert!(
                stderr.contains("failed to load config.toml"),
                "{command:?}, {config:?}: {stderr}"
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn queue_rejects_local_daemon_that_does_not_support_queueing() -> Result<()> {
    let codex_home = socket_test_home()?;
    let socket_path = codex_app_server::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(
        socket_path
            .as_path()
            .parent()
            .context("missing socket parent")?,
    )?;
    let listener = tokio::net::UnixListener::bind(socket_path.as_path())?;
    let server_home = codex_home.path().to_path_buf();
    let server = tokio::spawn(async move {
        let (probe, _) = listener.accept().await?;
        drop(probe);
        let (stream, _) = listener.accept().await?;
        respond_to_queue_request(
            stream,
            server_home.as_path(),
            QueueResponse::UnknownMethodVariant,
        )
        .await
    });

    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args(["queue", "--thread", THREAD_ID, "--message", "do the thing"])
        .output()
        .await?;
    server.await??;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("local app-server daemon does not support thread/queue/add")
    );
    assert!(!codex_home.path().join("queue_1.sqlite").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn queue_rejects_overrides_that_bypass_local_daemon() -> Result<()> {
    let codex_home = socket_test_home()?;
    let socket_path = codex_app_server::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(
        socket_path
            .as_path()
            .parent()
            .context("missing socket parent")?,
    )?;
    let listener = tokio::net::UnixListener::bind(socket_path.as_path())?;
    let server = tokio::spawn(async move {
        let (probe, _) = listener.accept().await?;
        drop(probe);
        Ok::<_, std::io::Error>(())
    });

    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args([
            "queue",
            "-c",
            "model=\"test-model\"",
            "--thread",
            THREAD_ID,
            "--message",
            "do the thing",
        ])
        .output()
        .await?;
    server.await??;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("embedded app server while a local app-server daemon is running")
    );
    Ok(())
}
