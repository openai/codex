//! Test support for running the Linux exec-server inside bubblewrap.

use std::future::Future;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use bwrap_test_support::BwrapTestCommand;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::time::timeout;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

/// Runs the Linux exec-server in a fresh bubblewrap environment for a scoped operation.
pub struct BwrapExecServer;

impl BwrapExecServer {
    /// Starts the server, passes its WebSocket URL to `operation`, and tears it down afterward.
    pub async fn scope<T, F, Fut>(self, operation: F) -> Result<T>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let executable = codex_utils_cargo_bin::cargo_bin("codex")?;
        let bwrap = codex_utils_cargo_bin::cargo_bin("bwrap")?;
        let bwrap_dir = bwrap.parent().context("bwrap runfile has no parent")?;
        let mut path_entries = vec![bwrap_dir.to_path_buf()];
        if let Some(path) = std::env::var_os("PATH") {
            path_entries.extend(std::env::split_paths(&path));
        }
        let path = std::env::join_paths(path_entries).context("build bwrap exec-server PATH")?;
        let mut exec_server = BwrapTestCommand::new(executable)
            .arg("exec-server")
            .arg("--listen")
            .arg("ws://127.0.0.1:0")
            .env("PATH", path)
            .spawn()?;
        let stdout = exec_server.take_stdout();

        exec_server
            .scope(async move {
                let mut lines = BufReader::new(stdout).lines();
                let exec_server_url = timeout(STARTUP_TIMEOUT, async {
                    loop {
                        let line = lines
                            .next_line()
                            .await?
                            .context("bwrap exec-server exited before reporting its URL")?;
                        if line.starts_with("ws://") {
                            return Ok::<_, anyhow::Error>(line);
                        }
                    }
                })
                .await
                .context("timed out waiting for bwrap exec-server URL")??;
                operation(exec_server_url).await
            })
            .await
    }
}

#[cfg(test)]
#[path = "bwrap_exec_server_tests.rs"]
mod tests;
