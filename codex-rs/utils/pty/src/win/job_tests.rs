use std::cell::Cell;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::io::RawHandle;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use pretty_assertions::assert_eq;
use winapi::shared::minwindef::FALSE;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::CreateProcessW;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::processthreadsapi::PROCESS_INFORMATION;
use winapi::um::processthreadsapi::STARTUPINFOW;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::CREATE_NO_WINDOW;
use winapi::um::winbase::CREATE_SUSPENDED;
use winapi::um::winbase::WAIT_OBJECT_0;

use super::JobObjectApi;
use super::KillOnCloseJob;
use super::SuspendedProcess;
use super::Win32JobObjectApi;

const PROCESS_WAIT_MS: u32 = 5_000;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> io::Result<Self> {
        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "codex-utils-pty-job-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureStage {
    Create,
    Configure,
    Assign,
    Resume,
    ResumeCount(u32),
}

struct FailingJobObjectApi {
    stage: FailureStage,
}

impl FailingJobObjectApi {
    fn failure(&self, stage: FailureStage) -> io::Result<()> {
        if self.stage == stage {
            Err(io::Error::other(format!("forced {stage:?} failure")))
        } else {
            Ok(())
        }
    }
}

impl JobObjectApi for FailingJobObjectApi {
    fn create_job(&self) -> io::Result<OwnedHandle> {
        self.failure(FailureStage::Create)?;
        Win32JobObjectApi.create_job()
    }

    fn configure_kill_on_close(&self, job: RawHandle) -> io::Result<()> {
        self.failure(FailureStage::Configure)?;
        Win32JobObjectApi.configure_kill_on_close(job)
    }

    fn assign_process(&self, job: RawHandle, process: RawHandle) -> io::Result<()> {
        self.failure(FailureStage::Assign)?;
        Win32JobObjectApi.assign_process(job, process)
    }

    fn resume_thread(&self, primary_thread: RawHandle) -> io::Result<u32> {
        if self.stage == FailureStage::Resume {
            return Err(io::Error::other("forced Resume failure"));
        }
        if let FailureStage::ResumeCount(count) = self.stage {
            return Ok(count);
        }
        Win32JobObjectApi.resume_thread(primary_thread)
    }
}

fn spawn_suspended(command_line: &str) -> io::Result<SuspendedProcess> {
    let mut command_line = OsStr::new(command_line)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut startup_info: STARTUPINFOW = unsafe { mem::zeroed() };
    startup_info.cb = mem::size_of::<STARTUPINFOW>() as u32;
    let mut process_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    let result = unsafe {
        CreateProcessW(
            std::ptr::null(),
            command_line.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            FALSE,
            CREATE_NO_WINDOW | CREATE_SUSPENDED,
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut startup_info,
            &mut process_info,
        )
    };
    if result == FALSE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe {
        SuspendedProcess::from_raw_handles(
            process_info.hProcess.cast(),
            process_info.hThread.cast(),
            process_info.dwProcessId,
        )
    })
}

fn create_job_then_spawn<T>(
    api: &impl JobObjectApi,
    spawn: impl FnOnce(KillOnCloseJob) -> io::Result<T>,
) -> io::Result<T> {
    KillOnCloseJob::new_with_api(api).and_then(spawn)
}

fn batch_command(script: &Path) -> String {
    format!("cmd.exe /d /c call \"{}\"", script.display())
}

fn spawn_marker_process(directory: &TestDirectory) -> io::Result<(SuspendedProcess, PathBuf)> {
    let script = directory.join("marker.cmd");
    let marker = directory.join("marker");
    fs::write(&script, "@echo off\r\necho ran>\"%~dp0marker\"\r\n")?;
    Ok((spawn_suspended(&batch_command(&script))?, marker))
}

fn wait_for_file(path: &Path) -> io::Result<()> {
    let start = Instant::now();
    while !path.exists() {
        if start.elapsed() >= Duration::from_secs(5) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out waiting for {}", path.display()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn process_observer(process: &SuspendedProcess) -> io::Result<OwnedHandle> {
    let Some(handle) = process.process.as_ref() else {
        return Err(io::Error::other("suspended process has no process handle"));
    };
    handle.try_clone()
}

fn assert_process_exited(process: &OwnedHandle) {
    let result = unsafe {
        WaitForSingleObject(process.as_raw_handle().cast(), /*dwMilliseconds*/ 0)
    };
    assert_eq!(result, WAIT_OBJECT_0);
}

#[test]
fn direct_job_close_terminates_root_and_grandchild() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let root_script = directory.join("root.cmd");
    let grandchild_script = directory.join("grandchild.cmd");
    let grandchild_ready = directory.join("grandchild-ready");
    let grandchild_escaped = directory.join("grandchild-escaped");
    fs::write(
        &root_script,
        "@echo off\r\nstart \"\" /b cmd.exe /d /c call \"%~dp0grandchild.cmd\"\r\nping.exe -n 30 127.0.0.1 >NUL\r\n",
    )?;
    fs::write(
        &grandchild_script,
        "@echo off\r\necho ready>\"%~dp0grandchild-ready\"\r\nping.exe -n 3 127.0.0.1 >NUL\r\necho escaped>\"%~dp0grandchild-escaped\"\r\n",
    )?;

    let job = KillOnCloseJob::new()?;
    let process = spawn_suspended(&batch_command(&root_script))?.assign_and_resume(job)?;

    wait_for_file(&grandchild_ready)?;

    let mut exit_code = 0;
    let result = unsafe { GetExitCodeProcess(process.as_raw_handle().cast(), &mut exit_code) };
    assert_ne!(result, FALSE);
    assert_eq!(exit_code, STILL_ACTIVE);

    process.controller().close()?;
    let result = unsafe { WaitForSingleObject(process.as_raw_handle().cast(), PROCESS_WAIT_MS) };
    assert_eq!(result, WAIT_OBJECT_0);

    thread::sleep(Duration::from_secs(4));
    assert!(grandchild_ready.exists());
    assert!(!grandchild_escaped.exists());

    Ok(())
}

#[test]
fn closing_job_is_idempotent() -> anyhow::Result<()> {
    let job = KillOnCloseJob::new()?;
    job.close()?;
    job.close()?;
    Ok(())
}

#[test]
fn job_creation_failure_retains_stage_context() {
    let spawn_reached = Cell::new(false);
    let result = create_job_then_spawn(
        &FailingJobObjectApi {
            stage: FailureStage::Create,
        },
        |_| {
            spawn_reached.set(true);
            Ok(())
        },
    );
    let Err(err) = result else {
        panic!("forced job creation should fail");
    };
    assert!(err.to_string().contains("failed to create job object"));
    assert!(!spawn_reached.get());
}

#[test]
fn job_configuration_failure_retains_stage_context() {
    let spawn_reached = Cell::new(false);
    let result = create_job_then_spawn(
        &FailingJobObjectApi {
            stage: FailureStage::Configure,
        },
        |_| {
            spawn_reached.set(true);
            Ok(())
        },
    );
    let Err(err) = result else {
        panic!("forced job configuration should fail");
    };
    assert!(
        err.to_string()
            .contains("failed to configure kill-on-close job object")
    );
    assert!(!spawn_reached.get());
}

#[test]
fn assignment_failure_terminates_suspended_process() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let (process, marker) = spawn_marker_process(&directory)?;
    let observer = process_observer(&process)?;
    let job = KillOnCloseJob::new()?;
    let result = process.assign_and_resume_with_api(
        job,
        &FailingJobObjectApi {
            stage: FailureStage::Assign,
        },
    );
    let Err(err) = result else {
        panic!("forced job assignment should fail");
    };
    assert!(
        err.to_string()
            .contains("failed to assign suspended process to job")
    );
    assert_process_exited(&observer);
    assert!(!marker.exists());
    Ok(())
}

#[test]
fn resume_failure_terminates_suspended_process() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let (process, marker) = spawn_marker_process(&directory)?;
    let observer = process_observer(&process)?;
    let job = KillOnCloseJob::new()?;
    let result = process.assign_and_resume_with_api(
        job,
        &FailingJobObjectApi {
            stage: FailureStage::Resume,
        },
    );
    let Err(err) = result else {
        panic!("forced process resume should fail");
    };
    assert!(
        err.to_string()
            .contains("failed to resume suspended process")
    );
    assert_process_exited(&observer);
    assert!(!marker.exists());
    Ok(())
}

#[test]
fn unexpected_resume_count_terminates_suspended_process() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let (process, marker) = spawn_marker_process(&directory)?;
    let observer = process_observer(&process)?;
    let job = KillOnCloseJob::new()?;
    let result = process.assign_and_resume_with_api(
        job,
        &FailingJobObjectApi {
            stage: FailureStage::ResumeCount(0),
        },
    );
    let Err(err) = result else {
        panic!("unexpected resume count should fail");
    };
    assert_eq!(err.to_string(), "failed to resume suspended process");
    assert_eq!(
        err.root_cause().to_string(),
        "expected suspended process thread count 1, got 0"
    );
    assert_process_exited(&observer);
    assert!(!marker.exists());
    Ok(())
}
