//! Exercises listener authentication through the standalone executor CLI and real WebSockets.

use anyhow::Context;
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use std::process::Stdio;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

enum AuthMode {
    TokenHash,
    TokenFile,
    SignedBearer,
}

#[tokio::test]
async fn exec_server_websocket_auth_gates_rpc_connections() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(/*secs*/ 60), async {
        let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
        for mode in [
            AuthMode::TokenHash,
            AuthMode::TokenFile,
            AuthMode::SignedBearer,
        ] {
            let home = TempDir::new()?;
            let secret = "0123456789abcdef0123456789abcdef";
            let secret_file = home.path().join("secret");
            std::fs::write(&secret_file, secret)?;
            let mut command = Command::new(&codex);
            command
                .args(["exec-server", "--listen", "ws://127.0.0.1:0", "--ws-auth"])
                .env("CODEX_HOME", home.path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .kill_on_drop(true);
            let token = match mode {
                AuthMode::TokenHash => {
                    command.args([
                        "capability-token",
                        "--ws-token-sha256",
                        &format!("{:x}", Sha256::digest(secret.as_bytes())),
                    ]);
                    secret.to_string()
                }
                AuthMode::TokenFile => {
                    command
                        .args(["capability-token", "--ws-token-file"])
                        .arg(&secret_file);
                    secret.to_string()
                }
                AuthMode::SignedBearer => {
                    command
                        .args(["signed-bearer-token", "--ws-shared-secret-file"])
                        .arg(&secret_file)
                        .args([
                            "--ws-issuer",
                            "test-issuer",
                            "--ws-audience",
                            "test-executor",
                        ]);
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                    jsonwebtoken::encode(
                        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
                        &json!({"exp": now + 300, "iss": "test-issuer", "aud": "test-executor"}),
                        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
                    )?
                }
            };
            let mut child = command.spawn()?;
            let mut lines = BufReader::new(child.stdout.take().context("stdout")?).lines();
            let url = loop {
                let line = lines
                    .next_line()
                    .await?
                    .context("listener exited before startup")?;
                if line.starts_with("ws://") {
                    break line;
                }
            };

            // A missing or incorrect credential must fail before any RPC session opens.
            let digest_as_bearer = format!("Bearer {:x}", Sha256::digest(secret.as_bytes()));
            for authorization in [
                None,
                Some("Bearer incorrect"),
                Some("Basic incorrect"),
                Some(digest_as_bearer.as_str()),
            ] {
                let mut request = url.as_str().into_client_request()?;
                if let Some(authorization) = authorization {
                    request
                        .headers_mut()
                        .insert("authorization", authorization.parse()?);
                }
                let error = connect_async(request)
                    .await
                    .expect_err("reject unauthorized upgrade");
                let WebSocketError::Http(response) = error else {
                    anyhow::bail!("expected HTTP rejection, got {error}");
                };
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            }

            // Authentication applies to every connection, including reconnects.
            for _ in 0..2 {
                let mut request = url.as_str().into_client_request()?;
                request
                    .headers_mut()
                    .insert("authorization", format!("Bearer {token}").parse()?);
                let (mut websocket, _) = connect_async(request).await?;
                websocket
                    .send(Message::Text(
                        json!({
                            "id": 1,
                            "method": "initialize",
                            "params": {"clientName": "auth-test"}
                        })
                        .to_string()
                        .into(),
                    ))
                    .await?;
                let response = websocket.next().await.context("initialize response")??;
                let response: serde_json::Value = serde_json::from_str(response.to_text()?)?;
                assert_eq!(response["id"], json!(1));
                assert!(response.get("error").is_none(), "{response}");
                assert!(response["result"]["sessionId"].is_string(), "{response}");
                websocket.close(/*msg*/ None).await?;
            }
            child.kill().await?;
        }
        Ok(())
    })
    .await?
}

#[test]
fn exec_server_websocket_auth_rejects_inapplicable_transports() -> Result<()> {
    let home = TempDir::new()?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    for (transport, expected) in [
        (vec!["--listen", "stdio"], "not stdio"),
        (vec!["--listen", "stdio://"], "not stdio"),
        (
            vec![
                "--remote",
                "https://registry.example",
                "--environment-id",
                "test",
            ],
            "cannot be used with --remote",
        ),
        (
            vec![
                "forward",
                "--connect",
                "ws://127.0.0.1:1234",
                "--remote",
                "https://registry.example",
                "--environment-id",
                "test",
            ],
            "cannot be used with --remote",
        ),
    ] {
        assert_cmd::Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .args([
                "exec-server",
                "--ws-auth",
                "capability-token",
                "--ws-token-sha256",
                &"ab".repeat(32),
            ])
            .args(transport)
            .assert()
            .failure()
            .stderr(predicates::str::contains(expected));
    }
    Ok(())
}

#[test]
fn exec_server_websocket_auth_rejects_invalid_configuration() -> Result<()> {
    let home = TempDir::new()?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    for (args, expected) in [
        (vec!["--ws-token-sha256", "invalid"], "require `--ws-auth"),
        (
            vec![
                "--ws-auth",
                "capability-token",
                "--ws-token-sha256",
                "invalid",
            ],
            "64-character hex",
        ),
        (vec!["--ws-auth", "capability-token"], "is required"),
    ] {
        assert_cmd::Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .args(["exec-server"])
            .args(args)
            .assert()
            .failure()
            .stderr(predicates::str::contains(expected));
    }
    Ok(())
}
