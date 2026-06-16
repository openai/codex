//! Test support for running the Windows exec-server under Wine.

use std::future::Future;

use anyhow::Context;
use anyhow::Result;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use wine_test_support::WineTestCommand;

/// Runs the Windows exec-server under Wine for the duration of a scoped operation.
pub struct WineExecServer;

impl WineExecServer {
    /// Starts the server, passes its WebSocket URL to `operation`, and tears it down afterward.
    pub async fn scope<T, F, Fut>(self, operation: F) -> Result<T>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let executable = codex_utils_cargo_bin::cargo_bin("wine-windows-exec-server")?;
        let mut exec_server = WineTestCommand::new(executable)
            .env("CODEX_HOME", r"C:\codex-home")
            .spawn()?;
        let stdout = exec_server.take_stdout();

        exec_server
            .scope(async move {
                let mut lines = BufReader::new(stdout).lines();
                let exec_server_url = loop {
                    let line = lines
                        .next_line()
                        .await?
                        .context("Wine exec-server exited before reporting its URL")?;
                    if line.starts_with("ws://") {
                        break line;
                    }
                };
                operation(exec_server_url).await
            })
            .await
    }
}
