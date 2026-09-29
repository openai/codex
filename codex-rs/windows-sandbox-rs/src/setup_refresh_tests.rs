//! Verifies refresh helpers stay console-free when launched by a detached daemon.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use pretty_assertions::assert_eq;
use windows_sys::Win32::System::Console::GetConsoleWindow;
use windows_sys::Win32::System::Threading::DETACHED_PROCESS;

use super::run_setup_refresh_payload;

const DETACHED_TEST: &str = "setup::refresh_tests::refresh_helper_has_no_console";
const HELPER_TEST: &str = "setup::refresh_tests::refresh_console_probe";
const HOME_ENV: &str = "CODEX_TEST_REFRESH_CONSOLE_HOME";

#[test]
fn refresh_helper_has_no_console() -> Result<()> {
    if let Some(home) = std::env::var_os(HOME_ENV).map(PathBuf::from) {
        assert_eq!(unsafe { GetConsoleWindow() }, 0);
        run_setup_refresh_payload(HELPER_TEST, &home)?;
        assert_eq!(fs::read(home.join("probe"))?, b"no console");
        return Ok(());
    }

    // Re-execute detached with a disposable fake setup helper beside it. This
    // exercises the production refresh launch without provisioning sandbox users.
    let temporary = tempfile::tempdir()?;
    let executable = std::env::current_exe()?;
    let detached = temporary.path().join("detached.exe");
    fs::copy(&executable, &detached)?;
    fs::copy(
        &executable,
        temporary.path().join("codex-windows-sandbox-setup.exe"),
    )?;
    let output = Command::new(detached)
        .args(["--exact", DETACHED_TEST, "--nocapture"])
        .env(HOME_ENV, temporary.path())
        .creation_flags(DETACHED_PROCESS)
        .output()?;
    assert!(
        output.status.success(),
        "detached refresh probe failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(fs::read(temporary.path().join("probe"))?, b"no console");
    Ok(())
}

#[test]
fn refresh_console_probe() -> Result<()> {
    // The setup launcher supplies this test name as its single payload argument.
    // Ordinary test runs must not require their own parent to be console-free.
    if std::env::args().nth(1).as_deref() != Some(HELPER_TEST) {
        return Ok(());
    }
    let home = PathBuf::from(std::env::var_os(HOME_ENV).expect("detached probe home"));
    assert_eq!(unsafe { GetConsoleWindow() }, 0);
    fs::write(home.join("probe"), b"no console")?;
    Ok(())
}
