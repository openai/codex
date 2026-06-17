use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use tokio::process::Command;
use wine_exec_server_test_support::WineExecServer;

const TEST_BINARY_ENV_VAR: &str = "CODEX_WINE_EXEC_TEST_BINARY";
const TEST_ENVIRONMENT_ENV_VAR: &str = "CODEX_TEST_ENVIRONMENT";
const REMOTE_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_REMOTE_EXEC_SERVER_URL";
const LEGACY_REMOTE_ENV_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV";
const DOCKER_CONTAINER_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV_CONTAINER_NAME";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let test_binary = PathBuf::from(
        std::env::var_os(TEST_BINARY_ENV_VAR)
            .with_context(|| format!("{TEST_BINARY_ENV_VAR} must be set by the Bazel test rule"))?,
    );
    let forwarded_args = std::env::args_os().skip(1).collect::<Vec<_>>();

    if is_terse_list_request(&forwarded_args) {
        let status = Command::new(&test_binary)
            .args(&forwarded_args)
            .status()
            .await
            .context("list integration tests")?;
        anyhow::ensure!(status.success(), "listing integration tests exited with {status}");
        return Ok(());
    }

    WineExecServer
        .scope(|exec_server_url| async move {
            let mut command = Command::new(test_binary);
            command
                .env(TEST_ENVIRONMENT_ENV_VAR, "wine-exec")
                .env(REMOTE_EXEC_SERVER_URL_ENV_VAR, exec_server_url)
                .env_remove(LEGACY_REMOTE_ENV_ENV_VAR)
                .env_remove(DOCKER_CONTAINER_ENV_VAR)
                .args(forwarded_args)
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .kill_on_drop(true);

            let status = command.status().await.context("run integration tests")?;
            anyhow::ensure!(status.success(), "integration tests exited with {status}");
            Ok(())
        })
        .await
}

fn is_terse_list_request(args: &[OsString]) -> bool {
    args.iter().map(OsString::as_os_str).eq([
        OsStr::new("--list"),
        OsStr::new("--format"),
        OsStr::new("terse"),
    ])
}
