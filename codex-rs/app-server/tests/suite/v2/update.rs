#![cfg(unix)]

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UpdateApplyResponse;
use pretty_assertions::assert_eq;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn update_apply_runs_the_detected_npm_updater() -> Result<()> {
    let codex_home = TempDir::new()?;
    let bin_dir = codex_home.path().join("bin");
    fs::create_dir_all(&bin_dir)?;
    let npm_path = bin_dir.join("npm");
    let invocation_path = codex_home.path().join("npm-invocation.txt");
    fs::write(
        &npm_path,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CODEX_TEST_UPDATE_INVOCATION\"\n",
    )?;
    let mut permissions = fs::metadata(&npm_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&npm_path, permissions)?;

    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let invocation_path = invocation_path.to_string_lossy().into_owned();
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("CODEX_MANAGED_BY_NPM", Some("1")),
            ("CODEX_TEST_UPDATE_INVOCATION", Some(&invocation_path)),
            ("PATH", Some(&path)),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_update_apply_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response = to_response::<UpdateApplyResponse>(response)?;

    assert_eq!(response, UpdateApplyResponse {});
    assert_eq!(
        fs::read_to_string(invocation_path)?,
        "install\n-g\n@openai/codex\n"
    );
    Ok(())
}
