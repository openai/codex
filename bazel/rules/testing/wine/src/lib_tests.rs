use std::any::Any;
use std::fs;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_pty::TerminalSize;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use super::WineCommandOutput;
use super::WineRuntimePaths;
use super::WineTestCommand;
use super::WineTestProcess;
use super::collect_wine_command_output;
use super::install_powershell_runtime;
use super::wine_pty_environment;

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
async fn exported_command_context_runs_commands_in_active_prefix() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let prefix = prefix_path(&process);
    let prefix_after_scope = prefix.clone();
    let marker = prefix.join("drive_c").join("shared-prefix-marker");
    let powershell_marker = prefix
        .join("drive_c")
        .join("shared-powershell-prefix-marker");
    let context = process.command_context();
    let wrapper = codex_utils_cargo_bin::cargo_bin("wine-test-exec")?;
    let smoke = codex_utils_cargo_bin::cargo_bin("wine-smoke")?;

    process
        .scope(async move {
            let mut command = TokioCommand::new(&wrapper);
            context.apply_to_command(&mut command);
            let output = command
                .arg(smoke)
                .arg("--write-prefix-marker")
                .output()
                .await
                .context("run Windows process through exported Wine test context")?;
            anyhow::ensure!(
                output.status.success(),
                "shared-prefix command failed: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim(),
            );
            assert_eq!(fs::read(&marker)?, b"shared prefix");

            let mut command = TokioCommand::new(wrapper);
            context.apply_to_command(&mut command);
            let output = command
                .args([
                    "--powershell",
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"[System.IO.File]::WriteAllText('C:\shared-powershell-prefix-marker', 'shared powershell prefix')"#,
                ])
                .output()
                .await
                .context("run PowerShell through exported Wine test context")?;
            anyhow::ensure!(
                output.status.success(),
                "shared-prefix PowerShell command failed: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim(),
            );
            assert_eq!(
                fs::read(&powershell_marker).with_context(|| {
                    format!(
                        "read PowerShell marker; stdout={} stderr={}",
                        String::from_utf8_lossy(&output.stdout).trim(),
                        String::from_utf8_lossy(&output.stderr).trim(),
                    )
                })?,
                b"shared powershell prefix"
            );
            Ok(())
        })
        .await?;

    assert_prefix_removed(&prefix_after_scope);
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

#[test]
fn powershell_runtime_follows_runfiles_symlinks() -> Result<()> {
    let prefix = TempDir::new()?;
    let runtime = TempDir::new()?;
    let backing = TempDir::new()?;
    let backing_file = backing.path().join("pwsh.exe");
    fs::write(&backing_file, b"pwsh")?;
    std::os::unix::fs::symlink(&backing_file, runtime.path().join("pwsh.exe"))?;

    install_powershell_runtime(prefix.path(), runtime.path())?;

    let installed = prefix
        .path()
        .join("drive_c")
        .join("Program Files")
        .join("PowerShell")
        .join("7")
        .join("pwsh.exe");
    assert_eq!(fs::read(installed)?, b"pwsh");
    Ok(())
}

#[tokio::test]
async fn pinned_powershell_runs_under_wine_with_a_pty() -> Result<()> {
    // Keep this integration smoke test local to the Wine support crate. The
    // production-shaped PowerShell launch path belongs to exec-server tests.
    // The marker makes the assertion resilient to Wine or PTY startup chatter.
    const POWERSHELL_SMOKE_MARKER: &str = "WINE_PWSH_SMOKE";
    // Besides proving that the pinned runtime starts, report the properties
    // that shell detection and command construction rely on: PowerShell 7 Core
    // running with Windows semantics and a backslash path separator.
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
    let runtime = WineRuntimePaths::from_runfiles()?;
    let prefix = TempDir::new()?;
    let env = wine_pty_environment(&runtime, prefix.path());
    let args = [
        runtime.powershell_executable.to_string_lossy().into_owned(),
        "-NoLogo".to_string(),
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-Command".to_string(),
        POWERSHELL_SMOKE_SCRIPT.to_string(),
    ];
    let wine = runtime.wine.to_string_lossy().into_owned();
    let spawned = codex_utils_pty::spawn_pty_process(
        &wine,
        &args,
        prefix.path(),
        &env,
        /*arg0*/ &None,
        TerminalSize::default(),
    )
    .await?;
    let command_result = timeout(
        Duration::from_secs(30),
        collect_wine_command_output(spawned),
    )
    .await
    .context("PowerShell smoke test timed out")
    .and_then(std::convert::identity);
    let shutdown_result = timeout(Duration::from_secs(10), async {
        let mut command = TokioCommand::new(&runtime.wineserver);
        command
            .args(["-k", "-w"])
            .env("HOME", prefix.path())
            .env("WINEPREFIX", prefix.path())
            .env("XDG_RUNTIME_DIR", prefix.path())
            .kill_on_drop(true);
        let status = command.status().await.context("stop isolated wineserver")?;
        anyhow::ensure!(
            status.success() || status.code() == Some(1),
            "wineserver exited with {status}"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("stop isolated wineserver timed out")
    .and_then(std::convert::identity);
    let WineCommandOutput {
        stdout,
        stderr,
        exit_code,
    } = match (command_result, shutdown_result) {
        (Ok(output), Ok(())) => output,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(error),
        (Err(error), Err(shutdown_error)) => {
            return Err(error.context(format!("Wine teardown also failed: {shutdown_error:#}")));
        }
    };
    anyhow::ensure!(
        exit_code == 0,
        "PowerShell exited with {}; stderr: {}",
        exit_code,
        String::from_utf8_lossy(&stderr),
    );
    let output = String::from_utf8(stdout)?;
    let marker_start = output
        .find(POWERSHELL_SMOKE_MARKER)
        .with_context(|| format!("PowerShell smoke marker was missing from {output:?}"))?;
    let smoke = output[marker_start..]
        .lines()
        .next()
        .context("PowerShell smoke marker line was incomplete")?
        .trim_end_matches('\r');
    let fields = smoke.split('|').collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        5,
        "unexpected PowerShell smoke output: {smoke}"
    );
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
