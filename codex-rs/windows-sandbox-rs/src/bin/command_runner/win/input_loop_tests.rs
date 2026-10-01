use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use codex_utils_pty::JobObject;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use super::NonOwningProcessHandle;
use super::spawn_input_loop;

#[tokio::test]
async fn disconnected_control_pipe_terminates_elevated_helper_and_descendants() -> anyhow::Result<()>
{
    let mut command = Command::new("python");
    command
        .args([
            "-u",
            "-c",
            "import subprocess,sys; child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); print(child.pid,flush=True); child.wait()",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let job = Arc::new(JobObject::create_without_breakaway()?);
    let mut helper = job.spawn_contained(&mut command)?;
    let stdout = helper.stdout.take().context("helper stdout")?;
    let mut stdout = BufReader::new(stdout);
    let mut descendant_pid = String::new();
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 10),
        stdout.read_line(&mut descendant_pid),
    )
    .await??;
    let descendant_pid = descendant_pid.trim().parse()?;
    let descendant = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE,
            /*bInheritHandle*/ 0,
            descendant_pid,
        )
    };
    anyhow::ensure!(!descendant.is_null(), "failed to open sandbox descendant");
    let descendant = unsafe { OwnedHandle::from_raw_handle(descendant as _) };
    let process = helper.raw_handle().context("helper process handle")?;

    let input_loop = spawn_input_loop(
        tempfile::tempfile()?,
        /*stdin_handle*/ None,
        Arc::new(Mutex::new(None)),
        Arc::clone(&job),
        NonOwningProcessHandle(process),
        /*log_dir*/ None,
    );
    input_loop
        .join()
        .expect("elevated runner input loop should stop after pipe disconnect");

    let status = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), helper.wait())
        .await
        .context("elevated helper survived control-pipe disconnect")??;
    anyhow::ensure!(!status.success(), "elevated helper unexpectedly succeeded");
    let descendant_status = unsafe {
        WaitForSingleObject(
            descendant.as_raw_handle() as _,
            /*dwMilliseconds*/ 2_000,
        )
    };
    anyhow::ensure!(
        descendant_status == WAIT_OBJECT_0,
        "elevated descendant survived control-pipe disconnect"
    );

    Ok(())
}
