//! Verify authentication updates across the running CLI executor process.

use std::process::Stdio;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_login::AuthCredentialsStoreMode;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::accept_hdr_async;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::path;

#[tokio::test]
async fn noise_executor_refreshes_on_401_and_reloads_rotated_credentials() -> Result<()> {
    let home = TempDir::new()?;
    let registry = MockServer::start().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    std::fs::write(
        home.path().join("config.toml"),
        "cli_auth_credentials_store = 'file'\n",
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("old-token").account_id("account"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(path("/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-token", "refresh_token": "new-refresh-token",
        })))
        .expect(1)
        .mount(&registry)
        .await;
    Mock::given(path("/cloud/environment/env/register"))
        .and(header("authorization", "Bearer old-token"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&registry)
        .await;
    for token in ["refreshed-token", "rotated-token"] {
        Mock::given(path("/cloud/environment/env/register"))
            .and(header("authorization", format!("Bearer {token}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "env", "url": format!("ws://{}", listener.local_addr()?),
                "executor_registration_id": "registration", "security_profile": "noise_hybrid_ik_v1",
            })))
            .with_priority(1)
            .up_to_n_times(1)
            .expect(1)
            .mount(&registry)
            .await;
    }
    // A later registry rejection reloads the rotated token from storage, without
    // another OAuth refresh or unconditional reloads on ordinary requests.
    Mock::given(path("/cloud/environment/env/register"))
        .and(header("authorization", "Bearer refreshed-token"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&registry)
        .await;
    let mut child = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .current_dir(home.path())
        .env("CODEX_HOME", home.path())
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env(
            "CODEX_REFRESH_TOKEN_URL_OVERRIDE",
            format!("{}/refresh", registry.uri()),
        )
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .args([
            "exec-server",
            "--remote",
            &registry.uri(),
            "--environment-id",
            "env",
            "--exit-on-stdin-close",
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdin = child.stdin.take();
    let (socket, _) = listener.accept().await?;
    let mut websocket = accept_async(socket).await?;
    // Update storage while the executor is alive, then force a new registration.
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("rotated-token").account_id("account"),
        AuthCredentialsStoreMode::File,
    )?;
    websocket.close(None).await?;
    drop(websocket);
    let (socket, _) = listener.accept().await?;
    let rejected = accept_hdr_async(socket, |_: &_, _: _| {
        Err(http::Response::builder().status(401).body(None).unwrap())
    })
    .await;
    assert!(rejected.is_err());
    let (socket, _) = listener.accept().await?;
    let _websocket = accept_async(socket).await?;
    drop(stdin);
    let output = child.wait_with_output().await?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
