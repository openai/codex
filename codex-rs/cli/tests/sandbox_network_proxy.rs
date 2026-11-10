#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::net::TcpListener;

use anyhow::Result;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[cfg(target_os = "macos")]
const LOOPBACK: &str = "127.0.0.1";
#[cfg(target_os = "linux")]
const LOOPBACK: &str = "127.0.0.2";

fn sandbox_unavailable(stderr: &str) -> Result<bool> {
    #[cfg(target_os = "linux")]
    if stderr.contains("bubblewrap is unavailable") {
        eprintln!("skipping network proxy sandbox test: bubblewrap is unavailable");
        return Ok(true);
    }
    #[cfg(target_os = "macos")]
    if stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted") {
        let control = std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
            .output()?;
        if String::from_utf8_lossy(&control.stderr)
            .contains("sandbox_apply: Operation not permitted")
        {
            eprintln!("skipping network proxy sandbox test: nested Seatbelt is unavailable");
            return Ok(true);
        }
    }
    Ok(false)
}

#[test]
fn sandbox_with_network_proxy_blocks_direct_loopback_access() -> Result<()> {
    let codex_home = TempDir::new()?;
    let listener = TcpListener::bind((LOOPBACK, 0))?;
    let port = listener.local_addr()?.port();
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = "network-test"

[features]
network_proxy = true
use_legacy_landlock = true

[permissions.network-test]
extends = ":workspace"

[permissions.network-test.network]
enabled = true
mode = "full"
"#,
    )?;

    let url = format!("http://{LOOPBACK}:{port}/");
    let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args([
            "sandbox",
            "--permission-profile",
            "network-test",
            "--",
            "curl",
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "1",
            "--max-time",
            "2",
            url.as_str(),
        ])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if sandbox_unavailable(&stderr)? {
        return Ok(());
    }

    assert_eq!(
        output.status.code(),
        Some(7),
        "expected direct loopback access to be blocked; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        stderr,
    );

    Ok(())
}

#[tokio::test]
async fn sandbox_with_network_proxy_allows_explicit_loopback_access() -> Result<()> {
    let codex_home = TempDir::new()?;
    let listener = TcpListener::bind((LOOPBACK, 0))?;
    let port = listener.local_addr()?.port();
    let server = MockServer::builder().listener(listener).start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
default_permissions = "network-test"

[features]
network_proxy = true
use_legacy_landlock = true

[permissions.network-test]
extends = ":workspace"

[permissions.network-test.network]
enabled = true
mode = "full"
allow_local_binding = false

[permissions.network-test.network.domains]
"{LOOPBACK}" = "allow"
"#
        ),
    )?;

    let url = format!("http://{LOOPBACK}:{port}/");
    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args([
            "sandbox",
            "--permission-profile",
            "network-test",
            "--",
            "curl",
            "--fail",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "2",
            "--max-time",
            "4",
            url.as_str(),
        ])
        .output()
        .await?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if sandbox_unavailable(&stderr)? {
        return Ok(());
    }

    assert!(
        output.status.success(),
        "expected allowlisted loopback access to succeed; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        stderr,
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    Ok(())
}
