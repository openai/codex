use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;

const START_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(target_os = "linux")]
const CODEX_LINUX_SANDBOX_EXE_ENV_VAR: &str = "CODEX_TEST_LINUX_SANDBOX_EXE";

/// Host-local exec-server fixture that exposes a WebSocket URL.
///
/// This is distinct from the ordinary local stdio executor: callers use it
/// when they need a socket transport they can interpose.
pub(crate) struct LocalWebsocketExecServer {
    child: Child,
    websocket_url: String,
}

impl LocalWebsocketExecServer {
    pub(crate) async fn start(codex_home: &Path, exec_server_program: &Path) -> Result<Self> {
        let mut command = Command::new(exec_server_program);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::inherit());
        command.current_dir(codex_home);
        command.env("CODEX_HOME", codex_home);
        #[cfg(target_os = "linux")]
        command.env(
            CODEX_LINUX_SANDBOX_EXE_ENV_VAR,
            core_test_support::find_codex_linux_sandbox_exe()
                .context("should find binary for delayed exec-server Linux sandbox helper")?,
        );
        command.kill_on_drop(true);
        let child = command.spawn().context("start local exec-server fixture")?;
        let mut exec_server = Self {
            child,
            websocket_url: String::new(),
        };
        let stdout = exec_server
            .child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("local exec-server fixture stdout was not captured"))?;
        let mut lines = BufReader::new(stdout).lines();
        let deadline = tokio::time::Instant::now() + START_TIMEOUT;
        exec_server.websocket_url = loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| anyhow!("timed out waiting for local exec-server listen URL"))?;
            let line = tokio::time::timeout(remaining, lines.next_line())
                .await
                .map_err(|_| anyhow!("timed out waiting for local exec-server listen URL"))??
                .ok_or_else(|| {
                    anyhow!("local exec-server exited before emitting its listen URL")
                })?;
            let listen_url = line.trim();
            if listen_url.starts_with("ws://") {
                break listen_url.to_string();
            }
        };
        Ok(exec_server)
    }

    pub(crate) fn websocket_url(&self) -> &str {
        &self.websocket_url
    }
}

impl Drop for LocalWebsocketExecServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);
        while start.elapsed() < timeout {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => return,
            }
        }
    }
}
