//! Verifies console suppression through contained launches and both containment fallbacks.

use super::*;
use pretty_assertions::assert_eq;
use std::os::windows::process::CommandExt;
use std::process::Stdio;
use std::time::Duration;
use winapi::um::winnt::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;

#[tokio::test]
async fn background_launches_keep_consoles_hidden_when_containment_fails() -> anyhow::Result<()> {
    const TEST_NAME: &str =
        "win::job::tests::background_launches_keep_consoles_hidden_when_containment_fails";
    const PHASE: &str = "CODEX_TEST_BACKGROUND_JOB_PHASE";
    let executable = std::env::current_exe()?;
    match std::env::var(PHASE).as_deref() {
        Ok("probe") => {
            assert!(unsafe { winapi::um::wincon::GetConsoleWindow() }.is_null());
            println!("background probe completed");
            return Ok(());
        }
        Ok("hold") => {
            std::thread::sleep(Duration::from_secs(/*secs*/ 30));
            return Ok(());
        }
        Ok("parent") => {}
        _ => {
            let output = std::process::Command::new(executable)
                .args(["--exact", TEST_NAME, "--nocapture"])
                .env(PHASE, "parent")
                .creation_flags(winapi::um::winbase::DETACHED_PROCESS)
                .output()?;
            assert!(output.status.success(), "detached probe failed: {output:?}");
            return Ok(());
        }
    }

    // Fill a one-process job so assigning the next suspended child must fail.
    // The parent remains outside this job, allowing the uncontained retry to run.
    let full_job = JobObject::create()?;
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    let configured = unsafe {
        SetInformationJobObject(
            full_job.handle.as_raw_handle().cast(),
            JobObjectExtendedLimitInformation,
            std::ptr::addr_of_mut!(limits).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    anyhow::ensure!(configured != 0, "{}", io::Error::last_os_error());
    let mut holder_command = Command::new(&executable);
    holder_command
        .args(["--exact", TEST_NAME])
        .env(PHASE, "hold");
    let mut holder = full_job.spawn_contained(&mut holder_command)?;

    for (case, job, expected_containment) in [
        (
            "creation failure",
            Err(io::Error::other("forced creation failure")),
            false,
        ),
        ("assignment failure", Ok(full_job), false),
        ("contained", JobObject::create(), true),
    ] {
        let mut command = Command::new(&executable);
        command
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(PHASE, "probe")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (child, job) = JobObject::spawn_background_with_job(&mut command, job)?;
        assert_eq!(job.is_some(), expected_containment, "{case}");
        let output =
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), child.wait_with_output())
                .await??;
        assert!(output.status.success(), "{case}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("background probe completed"));
    }
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), holder.wait()).await??;
    Ok(())
}
