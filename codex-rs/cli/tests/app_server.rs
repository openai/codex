use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use tempfile::TempDir;

#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use anyhow::bail;
#[cfg(unix)]
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use tokio::process::Child;
#[cfg(unix)]
use tokio::process::Command;
#[cfg(unix)]
use tokio::time::sleep;
#[cfg(unix)]
use tokio::time::timeout;

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
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_agent_proxy_repairs_missing_bootstrap_and_rejects_overlap() -> Result<()> {
    let root = TempDir::new()?;
    let codex_home = root.path().join("home");
    std::fs::create_dir(&codex_home)?;
    let control_socket_path = root.path().join("desktop.agent");
    let stable_agent_path = root.path().join("desktop.agent.agent");
    let missing_bootstrap_agent_path = root.path().join("missing-bootstrap.sock");
    let first_agent_path = root.path().join("first-agent.sock");
    let second_agent_path = root.path().join("second-agent.sock");
    let _first_agent = UnixListener::bind(&first_agent_path)?;
    let _second_agent = UnixListener::bind(&second_agent_path)?;
    let codex_bin = codex_utils_cargo_bin::cargo_bin("codex")?;

    let mut app_server = Command::new(&codex_bin)
        .args([
            "app-server",
            "--listen",
            &format!("unix://{}", control_socket_path.display()),
        ])
        .current_dir(root.path())
        .env("CODEX_HOME", &codex_home)
        .env("SSH_AUTH_SOCK", &missing_bootstrap_agent_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("spawn app server")?;
    wait_for_child_state(&mut app_server, "app server startup", || {
        control_socket_path.exists()
            && std::fs::read_link(&stable_agent_path)
                .is_ok_and(|target| target == missing_bootstrap_agent_path)
    })
    .await?;

    let mut first_proxy = Command::new(&codex_bin)
        .args(["app-server", "proxy", "--sock"])
        .arg(&control_socket_path)
        .current_dir(root.path())
        .env("CODEX_HOME", &codex_home)
        .env("SSH_AUTH_SOCK", &first_agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("spawn first proxy")?;
    wait_for_child_state(&mut first_proxy, "first proxy agent handoff", || {
        std::fs::read_link(&stable_agent_path).is_ok_and(|target| target == first_agent_path)
    })
    .await?;

    let second_output = timeout(
        Duration::from_secs(5),
        Command::new(&codex_bin)
            .args(["app-server", "proxy", "--sock"])
            .arg(&control_socket_path)
            .current_dir(root.path())
            .env("CODEX_HOME", &codex_home)
            .env("SSH_AUTH_SOCK", &second_agent_path)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .context("overlapping proxy did not exit")??;
    assert!(!second_output.status.success());
    assert!(
        String::from_utf8_lossy(&second_output.stderr)
            .contains("another app-server proxy already owns SSH agent forwarding")
    );

    assert!(first_proxy.try_wait()?.is_none());
    assert_eq!(std::fs::read_link(&stable_agent_path)?, first_agent_path);

    drop(first_proxy.stdin.take());
    timeout(Duration::from_secs(5), first_proxy.wait())
        .await
        .context("first proxy did not exit")??;
    assert!(std::fs::symlink_metadata(&stable_agent_path).is_err());

    app_server.start_kill()?;
    app_server.wait().await?;

    Ok(())
}

#[cfg(unix)]
async fn wait_for_child_state(
    child: &mut Child,
    description: &str,
    mut ready: impl FnMut() -> bool,
) -> Result<()> {
    timeout(Duration::from_secs(20), async {
        loop {
            if ready() {
                return Ok(());
            }
            if let Some(status) = child.try_wait()? {
                bail!("child exited before {description}: {status}");
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .with_context(|| format!("timed out waiting for {description}"))?
}
