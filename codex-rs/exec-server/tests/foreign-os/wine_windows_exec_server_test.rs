#[cfg(not(target_os = "linux"))]
compile_error!("the Wine exec-server test can only run on Linux");

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadParams;
use codex_exec_server::RemoteExecServerConnectArgs;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::ChildStdout;
use wine_test_support::WineTestCommand;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_exec_server_runs_non_tty_command_under_wine() -> Result<()> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-windows-exec-server")?;
    let mut server = WineTestCommand::new(executable)
        .env("CODEX_HOME", r"C:\codex-home")
        .spawn()?;
    let stdout = server.take_stdout();

    server.scope(exercise_exec_server(stdout)).await
}

async fn exercise_exec_server(stdout: ChildStdout) -> Result<()> {
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

    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        websocket_url,
        "wine-windows-bazel-test".to_string(),
    ))
    .await?;

    let info = client.environment_info().await?;
    // TODO(anp): Require PowerShell once it is available in the Wine test environment.
    assert!(
        matches!(info.shell.name.as_str(), "powershell" | "cmd"),
        "expected a Windows shell, got {info:?}",
    );

    let process_id = ProcessId::from("wine-cmd-smoke");
    let response = client
        .exec(ExecParams {
            process_id: process_id.clone(),
            argv: vec![
                r"C:\windows\system32\cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "echo WINE_BAZEL_OK&&cd".to_string(),
            ],
            cwd: PathBuf::from(r"C:\"),
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(response.process_id, process_id);

    let mut after_seq = None;
    let mut output = Vec::new();
    let exit_code = loop {
        let response = client
            .read(ReadParams {
                process_id: process_id.clone(),
                after_seq,
                max_bytes: Some(1024 * 1024),
                wait_ms: Some(5_000),
            })
            .await?;
        for chunk in response.chunks {
            output.extend(chunk.chunk.into_inner());
        }
        if response.closed {
            break response.exit_code;
        }
        after_seq = response.next_seq.checked_sub(1);
    };

    assert_eq!(exit_code, Some(0));
    let output = String::from_utf8(output)?;
    assert!(
        output.contains("WINE_BAZEL_OK"),
        "unexpected output: {output:?}"
    );
    assert!(output.contains(r"C:\"), "unexpected output: {output:?}");

    Ok(())
}
