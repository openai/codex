#![cfg(target_os = "linux")]

use std::net::TcpListener;

use anyhow::Result;
use tempfile::TempDir;

#[test]
fn sandbox_with_network_proxy_blocks_direct_loopback_access() -> Result<()> {
    let codex_home = TempDir::new()?;
    let listener = TcpListener::bind("127.0.0.2:0")?;
    let port = listener.local_addr()?.port();
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = "network-test"

[features]
network_proxy = true

[permissions.network-test]
extends = ":workspace"

[permissions.network-test.network]
enabled = true
mode = "full"

[permissions.network-test.network.domains]
"example.com" = "allow"
"#,
    )?;

    let direct_connect_test =
        format!("if exec 3<>/dev/tcp/127.0.0.2/{port}; then exit 1; else exit 0; fi");
    let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args([
            "sandbox",
            "--permissions-profile",
            "network-test",
            "--",
            "bash",
            "-c",
            direct_connect_test.as_str(),
        ])
        .output()?;

    assert!(
        output.status.success(),
        "expected direct loopback access to be blocked; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(())
}
