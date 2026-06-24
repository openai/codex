use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn strict_config_rejects_unknown_config_fields_for_app_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
foo = "bar"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["app-server", "--strict-config", "--listen", "off"])
        .assert()
        .failure()
        .stderr(contains("unknown configuration field"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn proxy_ensure_listener_recovers_a_stale_socket_and_reuses_the_listener() -> Result<()> {
    use std::os::unix::net::UnixListener;

    let codex_home = TempDir::new()?;
    let socket_dir = codex_home.path().join("app-server-control");
    let socket_path = socket_dir.join("app-server-control.sock");
    std::fs::create_dir_all(&socket_dir)?;

    let stale_listener = UnixListener::bind(&socket_path)?;
    drop(stale_listener);

    let mut first_proxy = codex_command(codex_home.path())?;
    first_proxy
        .args(["app-server", "proxy", "--ensure-listener"])
        .assert()
        .success();

    let pid_file = codex_home.path().join("app-server-daemon/app-server.pid");
    let first_pid_record = std::fs::read_to_string(&pid_file)?;

    let mut second_proxy = codex_command(codex_home.path())?;
    second_proxy
        .args(["app-server", "proxy", "--ensure-listener"])
        .assert()
        .success();

    let second_pid_record = std::fs::read_to_string(pid_file)?;

    let mut stop = codex_command(codex_home.path())?;
    stop.args(["app-server", "daemon", "stop"])
        .assert()
        .success();

    assert_eq!(first_pid_record, second_pid_record);
    Ok(())
}

#[cfg(unix)]
#[test]
fn proxy_ensure_listener_rejects_a_non_default_socket() -> Result<()> {
    let codex_home = TempDir::new()?;
    let other_socket = codex_home.path().join("other.sock");
    let mut proxy = codex_command(codex_home.path())?;
    let output = proxy
        .args([
            "app-server",
            "proxy",
            "--ensure-listener",
            "--sock",
            other_socket.to_str().expect("temp path should be UTF-8"),
        ])
        .output()?;

    let mut stop = codex_command(codex_home.path())?;
    stop.args(["app-server", "daemon", "stop"])
        .assert()
        .success();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--ensure-listener only supports the CODEX_HOME socket")
    );
    Ok(())
}
