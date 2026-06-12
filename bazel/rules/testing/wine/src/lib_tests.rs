use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command as TokioCommand;

use super::WineTestCommand;
use super::WineTestProcess;

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
        "Wine prefix was not removed: {}",
        prefix.display()
    );
}

struct PrefixRemovedOnDrop(PathBuf);

impl Drop for PrefixRemovedOnDrop {
    fn drop(&mut self) {
        assert_prefix_removed(&self.0);
    }
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
#[should_panic(expected = "WineTestProcess dropped without async teardown")]
async fn dropping_without_teardown_panics() {
    let process = waiting_smoke_process()
        .await
        .expect("start Windows smoke process");
    drop(process);
}

#[tokio::test]
#[should_panic(expected = "sentinel panic")]
async fn dropping_while_panicking_does_not_panic_again() {
    let _process = waiting_smoke_process()
        .await
        .expect("start Windows smoke process");
    panic!("sentinel panic");
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
#[should_panic(expected = "Wine process stdout has already been taken")]
async fn take_stdout_panics_when_called_twice() {
    let mut process = waiting_smoke_process()
        .await
        .expect("start Windows smoke process");
    process.take_stdout();
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
#[should_panic(expected = "scope panic")]
async fn scope_panic_preserves_panic_and_tears_down() {
    let process = waiting_smoke_process()
        .await
        .expect("start Windows smoke process");
    let _prefix_removed = PrefixRemovedOnDrop(prefix_path(&process));
    let _ = process.scope::<()>(async { panic!("scope panic") }).await;
}

#[tokio::test]
async fn scope_returns_teardown_error() -> Result<()> {
    let process = process_with_failing_wineserver_stop().await?;
    let prefix = prefix_path(&process);

    let error = process
        .scope(async { Ok::<_, anyhow::Error>(()) })
        .await
        .expect_err("scope teardown should fail");

    assert_eq!(error.to_string(), "stop isolated wineserver");
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
