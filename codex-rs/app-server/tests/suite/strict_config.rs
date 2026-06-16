use std::process::Command;

use anyhow::Result;
use tempfile::TempDir;

#[test]
fn strict_config_rejects_unknown_config_fields_for_standalone_app_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
foo = "bar"
"#,
    )?;

    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex-app-server")?)
        .env("CODEX_HOME", codex_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .args(["--strict-config", "--listen", "off"])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("unknown configuration field `foo`"),
        "expected strict config error in stderr, got: {stderr}"
    );

    Ok(())
}

#[cfg(target_os = "windows")]
#[test]
fn non_strict_config_rejects_unelevated_windows_sandbox_with_network_proxy() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = "networked"

[features]
network_proxy = true

[windows]
sandbox = "unelevated"

[permissions.networked.filesystem]
":minimal" = "read"

[permissions.networked.network]
enabled = true
"#,
    )?;

    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex-app-server")?)
        .env("CODEX_HOME", codex_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .args(["--listen", "off"])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("The network proxy requires the elevated Windows sandbox backend"),
        "expected Windows sandbox/network proxy config error in stderr, got: {stderr}"
    );

    Ok(())
}
