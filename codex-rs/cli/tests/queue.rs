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
async fn queue_rejects_local_daemon_that_does_not_support_queueing() -> Result<()> {
    let codex_home = tempfile::tempdir_in("/tmp")?;
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
    let codex_home = tempfile::tempdir_in("/tmp")?;
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
