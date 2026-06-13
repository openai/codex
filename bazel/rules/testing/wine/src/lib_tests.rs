use std::any::Any;
use std::future::Future;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command as TokioCommand;

use super::WineTestCommand;
use super::WineTestProcess;
use super::install_powershell_runtime;
use super::run_powershell;
use super::run_powershell_with_pty;

// The marker makes the assertion resilient to Wine or PTY startup chatter.
const POWERSHELL_SMOKE_MARKER: &str = "WINE_PWSH_SMOKE";
// Besides proving that the pinned runtime starts, report the properties that
// shell detection and command construction rely on: PowerShell 7 Core running
// with Windows semantics and a backslash path separator.
const POWERSHELL_SMOKE_SCRIPT: &str = concat!(
    "$ErrorActionPreference = 'Stop'; ",
    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); ",
    "$separatorCode = [int]([System.IO.Path]::DirectorySeparatorChar); ",
    "if ($PSVersionTable.PSVersion.Major -ne 7) { throw 'expected PowerShell 7' }; ",
    "if ($PSVersionTable.PSEdition -ne 'Core') { throw 'expected PowerShell Core' }; ",
    "if (-not $IsWindows) { throw 'expected Windows semantics' }; ",
    "if ($separatorCode -ne 92) { throw 'expected backslash path separator' }; ",
    "Write-Output ('WINE_PWSH_SMOKE|' + ",
    "$PSVersionTable.PSVersion.ToString() + '|' + ",
    "$PSVersionTable.PSEdition + '|' + ",
    "$IsWindows.ToString().ToLowerInvariant() + '|' + $separatorCode)",
);

fn assert_powershell_smoke_output(output: &str) -> Result<()> {
    let marker_start = output
        .find(POWERSHELL_SMOKE_MARKER)
        .with_context(|| format!("PowerShell smoke marker was missing from {output:?}"))?;
    let smoke = output[marker_start..]
        .lines()
        .next()
        .context("PowerShell smoke marker line was incomplete")?
        .trim_end_matches('\r');
    let fields = smoke.split('|').collect::<Vec<_>>();
    assert_eq!(fields.len(), 5, "unexpected PowerShell smoke output: {smoke}");
    assert_eq!(fields[0], POWERSHELL_SMOKE_MARKER);
    assert_eq!(
        fields[1].split('.').next(),
        Some("7"),
        "expected PowerShell 7.x, got {}",
        fields[1],
    );
    assert_eq!(&fields[2..], &["Core", "true", "92"]);
    Ok(())
}

async fn waiting_smoke_process() -> Result<WineTestProcess> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-smoke")?;
    let mut process = WineTestCommand::new(executable).arg("--wait").spawn()?;
    let mut lines = BufReader::new(process.take_stdout()).lines();
    let ready_line = lines
        .next_line()
        .await?
        .context("Windows smoke process exited before becoming ready")?;
    assert_eq!(ready_line, "WINE_TEST_READY");
    Ok(process)
}

fn prefix_path(process: &WineTestProcess) -> PathBuf {
    process
        .processes
        .as_ref()
        .expect("Wine process guard")
        .prefix
        .path()
        .to_path_buf()
}

fn assert_prefix_removed(prefix: &Path) {
    assert!(
        !prefix.exists(),
        "Wine prefix remains: {}",
        prefix.display()
    );
}

fn assert_panic_message(panic: Box<dyn Any + Send>, expected: &str) {
    assert_eq!(panic.downcast_ref::<&str>(), Some(&expected));
}

async fn assert_future_panics<T>(future: impl Future<Output = T>, expected: &str) {
    let panic = match AssertUnwindSafe(future).catch_unwind().await {
        Ok(_) => panic!("future should panic"),
        Err(panic) => panic,
    };
    assert_panic_message(panic, expected);
}

async fn process_with_failing_wineserver_stop() -> Result<WineTestProcess> {
    let mut process = waiting_smoke_process().await?;
    let processes = process.processes.as_mut().expect("Wine process guard");

    let mut command = TokioCommand::from(processes.stop_wineserver_command());
    let status = command
        .status()
        .await
        .context("pre-stop isolated wineserver")?;
    assert!(status.success(), "wineserver exited with {status}");

    processes.runtime.wineserver = processes.prefix.path().join("missing-wineserver");
    Ok(process)
}

#[tokio::test]
async fn dropping_without_teardown_panics() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    assert_future_panics(
        async move { drop(process) },
        "WineTestProcess dropped without async teardown",
    )
    .await;
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn dropping_while_panicking_does_not_panic_again() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    assert_future_panics(
        async move {
            let _process = process;
            panic!("sentinel panic");
        },
        "sentinel panic",
    )
    .await;
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn async_teardown_disarms_drop_bomb() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    process.shutdown().await?;
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn take_stdout_panics_when_called_twice() -> Result<()> {
    let mut process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    assert_future_panics(
        async {
            process.take_stdout();
        },
        "Wine process stdout has already been taken",
    )
    .await;
    process.shutdown().await?;
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn scope_returns_value_and_tears_down() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    let value = process
        .scope(async { Ok::<_, anyhow::Error>("scope value") })
        .await?;
    assert_eq!(value, "scope value");
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn scope_returns_body_error_and_tears_down() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);

    let error = process
        .scope(async { Err::<(), _>(anyhow!("scope body failed")) })
        .await
        .expect_err("scope body should fail");

    assert_eq!(error.to_string(), "scope body failed");
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn scope_panic_preserves_panic_and_tears_down() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);

    assert_future_panics(
        process.scope::<()>(async { panic!("scope panic") }),
        "scope panic",
    )
    .await;
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn shutdown_reports_nonzero_process_exit() -> Result<()> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-smoke")?;
    let mut process = WineTestCommand::new(executable).arg("--fail").spawn()?;
    let prefix = prefix_path(&process);
    let status = process
        .processes
        .as_mut()
        .expect("Wine process guard")
        .child
        .wait()
        .await?;
    assert!(
        !status.success(),
        "Windows smoke process unexpectedly passed"
    );

    let error = process.shutdown().await.expect_err("shutdown should fail");

    assert!(error.to_string().starts_with("Windows process exited with"));
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn scope_preserves_body_error_when_teardown_also_fails() -> Result<()> {
    let process = process_with_failing_wineserver_stop().await?;
    let prefix = prefix_path(&process);

    let error = process
        .scope(async { Err::<(), _>(anyhow!("scope body failed")) })
        .await
        .expect_err("scope body and teardown should fail");

    assert!(
        error
            .to_string()
            .starts_with("Wine teardown also failed: stop isolated wineserver"),
        "unexpected error: {error:#}"
    );
    assert_eq!(
        error.chain().last().map(ToString::to_string),
        Some("scope body failed".to_string())
    );
    assert_prefix_removed(&prefix);
    Ok(())
}

#[tokio::test]
async fn shutdown_returns_teardown_error() -> Result<()> {
    let process = process_with_failing_wineserver_stop().await?;
    let prefix = prefix_path(&process);

    let error = process
        .shutdown()
        .await
        .expect_err("shutdown should report a wineserver failure");

    assert_eq!(error.to_string(), "stop isolated wineserver");
    assert_prefix_removed(&prefix);
    Ok(())
}

#[test]
fn powershell_runtime_is_materialized_at_the_windows_fallback_path() -> Result<()> {
    let prefix = TempDir::new()?;
    let runtime = TempDir::new()?;
    fs::create_dir(runtime.path().join("Modules"))?;
    fs::write(runtime.path().join("pwsh.exe"), b"pwsh")?;
    fs::write(runtime.path().join("Modules").join("marker.txt"), b"module")?;

    install_powershell_runtime(prefix.path(), runtime.path())?;

    let installed = prefix
        .path()
        .join("drive_c")
        .join("Program Files")
        .join("PowerShell")
        .join("7");
    assert_eq!(fs::read(installed.join("pwsh.exe"))?, b"pwsh");
    assert_eq!(
        fs::read(installed.join("Modules").join("marker.txt"))?,
        b"module"
    );
    Ok(())
}

#[tokio::test]
async fn pinned_powershell_runs_without_a_pty_under_wine() -> Result<()> {
    run_powershell(POWERSHELL_SMOKE_SCRIPT).await
}

#[tokio::test]
async fn pinned_powershell_runs_with_a_pty_under_wine() -> Result<()> {
    let output = run_powershell_with_pty(POWERSHELL_SMOKE_SCRIPT).await?;
    assert_powershell_smoke_output(&output)
}
