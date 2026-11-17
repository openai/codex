//! Launch a real app server for CLI tests that connect over a Unix socket.

use anyhow::Result;
use anyhow::ensure;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Child;

pub(super) async fn start(home: &Path, extra_args: &[&str]) -> Result<(Child, String)> {
    let socket = codex_app_server_client::app_server_control_socket_path(home)?;
    let mut daemon = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["app-server", "--listen", "unix://"])
        .args(extra_args)
        .env("CODEX_HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    tokio::time::timeout(Duration::from_secs(30), async {
        while tokio::net::UnixStream::connect(socket.as_path())
            .await
            .is_err()
        {
            ensure!(
                daemon.try_wait()?.is_none(),
                "app server exited before opening its socket"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok((daemon, format!("unix://{}", socket.as_path().display())))
}
