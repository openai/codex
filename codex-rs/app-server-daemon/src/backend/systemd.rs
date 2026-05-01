use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use tokio::process::Command;

const UNIT_NAME: &str = "codex-app-server.service";

#[derive(Debug)]
pub(crate) struct SystemdBackend {
    codex_bin: PathBuf,
}

impl SystemdBackend {
    pub(crate) fn new(codex_bin: PathBuf) -> Self {
        Self { codex_bin }
    }

    pub(crate) async fn is_available() -> bool {
        command_succeeds("systemctl", &["--user", "show-environment"]).await
            && command_succeeds("systemd-run", &["--version"]).await
    }

    pub(crate) async fn is_running(&self) -> Result<bool> {
        let output = match Command::new("systemctl")
            .args(["--user", "is-active", UNIT_NAME])
            .output()
            .await
        {
            Ok(output) => output,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(err).context("failed to inspect user systemd app-server unit");
            }
        };
        Ok(output.status.success())
    }

    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        let output = Command::new("systemd-run")
            .args([
                "--user",
                "--unit",
                UNIT_NAME,
                "--collect",
                "--property",
                "Restart=on-failure",
            ])
            .arg(&self.codex_bin)
            .args(["app-server", "--listen", "unix://"])
            .output()
            .await
            .context("failed to invoke systemd-run for app-server")?;

        if output.status.success() {
            return Ok(None);
        }

        bail!(
            "failed to start app-server through user systemd: {}",
            stderr_or_status(&output)
        )
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["--user", "stop", UNIT_NAME])
            .output()
            .await
            .context("failed to stop user systemd app-server unit")?;
        if output.status.success() {
            return Ok(());
        }
        bail!(
            "failed to stop user systemd app-server unit: {}",
            stderr_or_status(&output)
        )
    }
}

async fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

fn stderr_or_status(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        output.status.to_string()
    } else {
        stderr
    }
}
