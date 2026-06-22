use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ProcessId;
use core_test_support::TestEnvironment;
use core_test_support::TestExecutionEnvironment;
use core_test_support::test_environment;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

const EXPECTED_EXEC_OUTPUT: &str = "hello from the selected executor";

#[tokio::test]
async fn selected_execution_environment_runs_target_native_command() -> Result<()> {
    let execution = TestExecutionEnvironment::new().await?;
    let shell = execution.environment().info().await?.shell;
    let command = match test_environment() {
        TestEnvironment::WineExec => vec![
            "cmd.exe".to_string(),
            "/d".to_string(),
            "/c".to_string(),
            format!("echo {EXPECTED_EXEC_OUTPUT}"),
        ],
        TestEnvironment::Local if cfg!(windows) => vec![
            "cmd.exe".to_string(),
            "/d".to_string(),
            "/c".to_string(),
            format!("echo {EXPECTED_EXEC_OUTPUT}"),
        ],
        TestEnvironment::Local | TestEnvironment::Docker { .. } => vec![
            shell.path,
            "-lc".to_string(),
            format!("printf '{EXPECTED_EXEC_OUTPUT}'"),
        ],
    };
    let cwd = execution
        .environment_cwd()
        .to_inferred_path_uri()
        .context("test environment cwd should be absolute")?;
    let started = execution
        .environment()
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from("test-execution-environment-smoke"),
            argv: command,
            cwd,
            env_policy: None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    let mut events = started.process.subscribe_events();
    let mut output = Vec::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(10), events.recv()).await?? {
            ExecProcessEvent::Output(chunk)
                if matches!(
                    chunk.stream,
                    ExecOutputStream::Stdout | ExecOutputStream::Pty
                ) =>
            {
                output.extend(chunk.chunk.0);
            }
            ExecProcessEvent::Output(_) => {}
            ExecProcessEvent::Exited {
                exit_code: code, ..
            } => exit_code = Some(code),
            ExecProcessEvent::Closed { .. } => break,
            ExecProcessEvent::Failed(error) => bail!("test execution failed: {error}"),
        }
    }

    let output = String::from_utf8(output)?;
    assert_eq!(output.trim_end_matches(['\r', '\n']), EXPECTED_EXEC_OUTPUT);
    assert_eq!(exit_code, Some(0));
    Ok(())
}
