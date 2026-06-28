use std::path::Path;

use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::save_oauth_tokens;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;

const SERVER_NAME: &str = "oauth-server";
const SERVER_URL: &str = "https://example.com/mcp";

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[tokio::test]
async fn mcp_logout_cli_removes_file_credentials() -> Result<()> {
    let codex_home = TempDir::new()?;
    let status = tokio::process::Command::new(std::env::current_exe()?)
        .args([
            "mcp_logout_cli_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .status()
        .await?;
    anyhow::ensure!(status.success(), "MCP logout child failed: {status}");
    Ok(())
}

#[tokio::test]
#[ignore = "spawned by mcp_logout_cli_removes_file_credentials"]
async fn mcp_logout_cli_child() -> Result<()> {
    let codex_home = std::env::var("CODEX_HOME")?;
    let codex_home = Path::new(&codex_home);
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            "mcp_oauth_credentials_store = \"file\"\n\n[mcp_servers.{SERVER_NAME}]\nurl = \"{SERVER_URL}\"\n"
        ),
    )?;

    let tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": SERVER_NAME,
        "url": SERVER_URL,
        "client_id": "test-client-id",
        "token_response": {
            "access_token": "access-token",
            "token_type": "bearer",
            "expires_in": 3600,
            "refresh_token": "refresh-token",
        },
        "expires_at": null,
    }))?;
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    assert!(codex_home.join(".credentials.json").exists());

    let mut logout = codex_command(codex_home)?;
    logout
        .args(["mcp", "logout", SERVER_NAME])
        .assert()
        .success()
        .stdout(contains(format!(
            "Removed OAuth credentials for '{SERVER_NAME}'."
        )));
    assert!(!codex_home.join(".credentials.json").exists());
    Ok(())
}
