use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use tempfile::TempDir;

#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use anyhow::bail;
#[cfg(unix)]
use codex_app_server_client::RemoteAppServerClient;
#[cfg(unix)]
use codex_app_server_client::RemoteAppServerConnectArgs;
#[cfg(unix)]
use codex_app_server_client::RemoteAppServerEndpoint;
#[cfg(unix)]
use codex_app_server_protocol::ClientRequest;
#[cfg(unix)]
use codex_app_server_protocol::CommandExecParams;
#[cfg(unix)]
use codex_app_server_protocol::CommandExecResponse;
#[cfg(unix)]
use codex_app_server_protocol::RequestId;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
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
async fn ssh_agent_handoff_survives_duplicate_startup_and_reaches_commands() -> Result<()> {
    let root = TempDir::new()?;
    let codex_home = root.path().join("home");
    std::fs::create_dir(&codex_home)?;
    let control_socket_path = root.path().join("desktop.agent");
    let stable_agent_path = root.path().join("desktop.agent.agent");
    let missing_bootstrap_agent_path = root.path().join("missing-bootstrap.sock");
    let current_agent_path = root.path().join("current-agent.sock");
    let _current_agent = UnixListener::bind(&current_agent_path)?;
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

    let duplicate_output = timeout(
        Duration::from_secs(10),
        Command::new(&codex_bin)
            .args([
                "app-server",
                "--listen",
                &format!("unix://{}", control_socket_path.display()),
            ])
            .current_dir(root.path())
            .env("CODEX_HOME", &codex_home)
            .env("SSH_AUTH_SOCK", root.path().join("duplicate-agent.sock"))
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .context("duplicate app server did not exit")??;
    assert!(!duplicate_output.status.success());
    assert_eq!(
        std::fs::read_link(&stable_agent_path)?,
        missing_bootstrap_agent_path
    );

    let mut proxy = Command::new(&codex_bin)
        .args(["app-server", "proxy", "--sock"])
        .arg(&control_socket_path)
        .current_dir(root.path())
        .env("CODEX_HOME", &codex_home)
        .env("SSH_AUTH_SOCK", &current_agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("spawn proxy")?;
    wait_for_child_state(&mut proxy, "proxy agent handoff", || {
        std::fs::read_link(&stable_agent_path).is_ok_and(|target| target == current_agent_path)
    })
    .await?;

    let app_server_client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        endpoint: RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::from_absolute_path(&control_socket_path)?,
        },
        client_name: "ssh-agent-handoff-test".to_string(),
        client_version: "0.0.0-test".to_string(),
        experimental_api: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: 8,
    })
    .await?;
    let response: CommandExecResponse = app_server_client
        .request_typed(ClientRequest::OneOffCommandExec {
            request_id: RequestId::Integer(1),
            params: CommandExecParams {
                command: vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "printf '%s|' \"$SSH_AUTH_SOCK\"; test -S \"$SSH_AUTH_SOCK\" \
                     && printf socket || printf missing"
                        .to_string(),
                ],
                process_id: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: false,
                output_bytes_cap: None,
                disable_output_cap: false,
                disable_timeout: false,
                timeout_ms: None,
                cwd: Some(root.path().to_path_buf()),
                env: None,
                size: None,
                sandbox_policy: None,
                permission_profile: None,
            },
        })
        .await?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: format!("{}|socket", stable_agent_path.display()),
            stderr: String::new(),
        }
    );
    app_server_client.shutdown().await?;

    drop(proxy.stdin.take());
    timeout(Duration::from_secs(5), proxy.wait())
        .await
        .context("proxy did not exit")??;

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
