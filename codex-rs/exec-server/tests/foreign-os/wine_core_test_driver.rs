use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use wine_test_support::WineTestCommand;

const CORE_TEST: &str =
    "suite::remote_env_windows::windows_exec_server_records_host_shell_mismatch";
const WINDOWS_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_WINDOWS_EXEC_SERVER_URL";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn core_test_runs_against_windows_exec_server_under_wine() -> Result<()> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-windows-exec-server")?;
    let mut server = WineTestCommand::new(executable)
        .env("CODEX_HOME", r"C:\codex-home")
        .spawn()?;
    let stdout = server.take_stdout();

    server
        .scope(async move {
            let mut lines = BufReader::new(stdout).lines();
            let websocket_url = loop {
                let line = lines
                    .next_line()
                    .await?
                    .context("Wine exec-server exited before reporting its URL")?;
                if line.starts_with("ws://") {
                    break line;
                }
            };

            let core_tests = codex_utils_cargo_bin::cargo_bin("codex-core-tests")?;
            let mut command = Command::new(core_tests);
            command
                .args([CORE_TEST, "--exact", "--nocapture"])
                .env(WINDOWS_EXEC_SERVER_URL_ENV_VAR, websocket_url)
                .kill_on_drop(true);
            let status = command.status().await.context("run core test under Wine")?;
            ensure!(status.success(), "core test exited with {status}");
            Ok(())
        })
        .await
}
