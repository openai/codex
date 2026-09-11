//! Config errors and startup/project warnings through the public API.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_ne;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn ignored_config_fields_emit_startup_and_project_warnings() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "network_proxy = { nested = 'private_value' }",
    )?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "allowed_permissions = [':read-only']",
    )?;
    let read_timeout = Duration::from_secs(30);
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(read_timeout)
        .await?;
    let startup_warning: ConfigWarningNotification =
        timeout(read_timeout, server.read_notification("configWarning")).await??;
    let summary = &startup_warning.summary;
    assert!(summary.contains("`network_proxy` is ignored."));
    assert!(summary.contains("`allowed_permissions` is ignored."));
    assert!(!summary.contains("private_value"));

    server.start_thread(ThreadStartParams::default()).await?;
    // Config warnings are sent before the thread/start response. Other startup
    // warnings (e.g. missing bubblewrap on Linux) may still be buffered here.
    while server
        .pending_notification_methods()
        .iter()
        .any(|method| method == "configWarning")
    {
        let warning: ConfigWarningNotification =
            timeout(read_timeout, server.read_notification("configWarning")).await??;
        assert_ne!(
            warning, startup_warning,
            "thread/start repeated the startup warning"
        );
    }

    let project = TempDir::new()?;
    std::fs::create_dir(project.path().join(".git"))?;
    std::fs::create_dir(project.path().join(".codex"))?;
    std::fs::write(
        project.path().join(".codex/config.toml"),
        "project_setting = 'private_value'",
    )?;
    set_project_trust_level(home.path(), project.path(), TrustLevel::Trusted)?;
    server
        .start_thread(ThreadStartParams {
            cwd: Some(project.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let warning: ConfigWarningNotification =
        timeout(read_timeout, server.read_notification("configWarning")).await??;
    assert!(warning.summary.contains("`project_setting` is ignored."));
    assert!(!warning.summary.contains("private_value"));
    Ok(())
}

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

#[test]
fn managed_auth_requirements_fail_closed_for_standalone_app_server() -> Result<()> {
    for requirements in [
        "allowed_login_methods = []\n",
        "allowed_login_methods = [\"chatgpt\"]\nallowed_chatgpt_workspaces = []\n",
    ] {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join("requirements.toml"), requirements)?;

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
            stderr.contains("authentication requirements do not permit any usable login method"),
            "expected managed authentication error in stderr, got: {stderr}"
        );
        assert!(
            !stderr.contains("using defaults"),
            "managed authentication requirements must not fall back to defaults"
        );
    }

    Ok(())
}
