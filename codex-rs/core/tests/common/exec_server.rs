//! Shared native exec-server process fixture; dropping it stops the child process.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::timeout;

const EXEC_SERVER_START_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ExecServerProcess {
    _codex_home: TempDir,
    child: Child,
    _stdout: BufReader<ChildStdout>,
    pub websocket_url: String,
}

impl ExecServerProcess {
    pub async fn start() -> Result<Self> {
        let codex_home = TempDir::new()?;
        let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
            .env("CODEX_HOME", codex_home.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .context("exec-server stdout should be piped")?;
        let mut stdout = BufReader::new(stdout);
        let deadline = Instant::now() + EXEC_SERVER_START_TIMEOUT;
        let websocket_url = loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .context("timed out waiting for exec-server listen URL")?;
            let mut line = String::new();
            let bytes_read = timeout(remaining, stdout.read_line(&mut line))
                .await
                .context("timed out reading exec-server listen URL")??;
            if bytes_read == 0 {
                bail!("exec-server exited before printing its listen URL");
            }
            let line = line.trim();
            if line.starts_with("ws://") {
                break line.to_string();
            }
        };

        Ok(Self {
            _codex_home: codex_home,
            child,
            _stdout: stdout,
            websocket_url,
        })
    }
}

impl Drop for ExecServerProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
