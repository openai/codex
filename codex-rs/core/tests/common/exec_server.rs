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
        Self::start_with_env(&[]).await
    }

    pub async fn start_with_env(env: &[(&str, &str)]) -> Result<Self> {
        let codex_home = TempDir::new()?;
        let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
        command.envs(env.iter().copied());
        Self::start_command(command, codex_home).await
    }

    pub async fn start_with_version(version: &str) -> Result<Self> {
        let codex_home = TempDir::new()?;
        let package = codex_home.path().join("package");
        let bin = package.join("bin");
        std::fs::create_dir_all(&bin)?;
        let executable = bin.join(format!("codex{}", std::env::consts::EXE_SUFFIX));
        codex_utils_cargo_bin::copy_executable(
            &codex_utils_cargo_bin::cargo_bin("codex")?,
            &executable,
        )?;
        std::fs::write(
            package.join("codex-package.json"),
            serde_json::to_vec(&serde_json::json!({"version": version}))?,
        )?;
        Self::start_command(Command::new(executable), codex_home).await
    }

    async fn start_command(mut command: Command, codex_home: TempDir) -> Result<Self> {
        let mut child = command
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
