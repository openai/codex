//! Windows process identity and file locks. Keep a process handle across shutdown
//! so PID reuse can never redirect forced termination to a different process.
//! Managed servers must not elevate ordinary clients sharing the account's socket.
//! Installer jobs contain extraction processes when an update is cancelled.
//! Detached launches stop the launcher's original stdio handles from propagating.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::SetHandleInformation;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::TOKEN_ELEVATION;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TokenElevation;
use windows_sys::Win32::Storage::FileSystem::LOCKFILE_EXCLUSIVE_LOCK;
use windows_sys::Win32::Storage::FileSystem::LOCKFILE_FAIL_IMMEDIATELY;
use windows_sys::Win32::Storage::FileSystem::LockFileEx;
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_BREAKAWAY_OK;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
use windows_sys::Win32::System::JobObjects::SetInformationJobObject;
use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
use windows_sys::Win32::System::Threading::DETACHED_PROCESS;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::System::Threading::GetProcessTimes;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
use windows_sys::Win32::System::Threading::PROCESS_ACCESS_RIGHTS;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
use windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE;
use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

pub(super) fn spawn_without_inheriting_stdio(
    command: &mut tokio::process::Command,
) -> Result<tokio::process::Child> {
    // The daemon must not inherit the launcher's output pipes: callers wait for
    // them to close after the launcher exits. Leave the flags cleared so concurrent
    // launches cannot inherit them either; Rust duplicates the child's chosen stdio.
    for (name, handle) in [
        ("stdin", io::stdin().as_raw_handle()),
        ("stdout", io::stdout().as_raw_handle()),
        ("stderr", io::stderr().as_raw_handle()),
    ] {
        if handle.is_null() || handle as isize == INVALID_HANDLE_VALUE {
            continue;
        }
        // SAFETY: these are borrowed standard handles; changing the inherit
        // flag neither closes them nor changes their read/write access.
        if unsafe {
            SetHandleInformation(handle as _, HANDLE_FLAG_INHERIT, /*dwflags*/ 0)
        } == 0
        {
            let error = io::Error::last_os_error();
            // An already-closed handle cannot be inherited.
            if error.raw_os_error() == Some(ERROR_INVALID_HANDLE as i32) {
                continue;
            }
            return Err(error)
                .with_context(|| format!("failed to clear launcher {name} inheritance"));
        }
    }
    Ok(command.spawn()?)
}

pub(crate) fn ensure_not_elevated() -> Result<()> {
    let mut token = 0;
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error()).context("failed to query daemon launcher token");
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token as _) };
    let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
    let mut returned = 0;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle() as _,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(io::Error::last_os_error())
            .context("failed to query daemon launcher elevation");
    }
    anyhow::ensure!(
        elevation.TokenIsElevated == 0,
        "start the Windows daemon from a non-elevated terminal; shared clients must not inherit administrator privileges"
    );
    Ok(())
}

// Check that breakaway launch is permitted before stopping an existing daemon.
// An outer system job may remain attached; membership alone does not establish
// whether it will terminate the daemon. Suspend the probe before cleanup.
pub(crate) fn ensure_detached_launch(executable: &Path) -> Result<()> {
    let mut child = Command::new(executable)
        .creation_flags(CREATE_SUSPENDED | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("cannot launch detached daemon; existing daemon was not stopped")?;
    child
        .kill()
        .context("failed to terminate suspended launch probe")?;
    child
        .wait()
        .context("failed to reap suspended launch probe")?;
    Ok(())
}

pub(super) struct Process(OwnedHandle);

impl Process {
    pub(super) fn open(pid: u32) -> Result<Option<Self>> {
        Self::open_with_access(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE)
    }

    fn open_with_access(pid: u32, access: PROCESS_ACCESS_RIGHTS) -> Result<Option<Self>> {
        let handle = unsafe {
            OpenProcess(access, /*binherithandle*/ 0, pid)
        };
        if handle == 0 {
            let err = io::Error::last_os_error();
            return if err.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                Ok(None)
            } else {
                Err(err).context("failed to open daemon process")
            };
        }
        Ok(Some(Self(unsafe {
            OwnedHandle::from_raw_handle(handle as _)
        })))
    }

    pub(super) fn start_time(&self) -> Result<String> {
        let mut created: FILETIME = unsafe { std::mem::zeroed() };
        let mut exited = created;
        let mut kernel = created;
        let mut user = created;
        if unsafe {
            GetProcessTimes(
                self.0.as_raw_handle() as _,
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        } == 0
        {
            return Err(io::Error::last_os_error()).context("failed to query daemon creation time");
        }
        Ok(
            ((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
                .to_string(),
        )
    }

    pub(super) fn is_running(&self) -> Result<bool> {
        match unsafe {
            WaitForSingleObject(self.0.as_raw_handle() as _, /*dwmilliseconds*/ 0)
        } {
            WAIT_TIMEOUT => Ok(true),
            WAIT_OBJECT_0 => Ok(false),
            _ => Err(io::Error::last_os_error()).context("failed to wait for daemon process"),
        }
    }

    pub(super) fn terminate(&self) -> Result<()> {
        if !self.is_running()? {
            return Ok(());
        }
        let pid = unsafe { GetProcessId(self.0.as_raw_handle() as _) };
        let Some(target) = Self::open_with_access(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
        )?
        else {
            return Ok(());
        };
        // Keep the original identity handle alive and validate the handle that
        // will actually be terminated, rather than trusting a second PID lookup.
        if target.start_time()? != self.start_time()? || !target.is_running()? {
            return Ok(());
        }
        if unsafe {
            TerminateProcess(target.0.as_raw_handle() as _, /*uexitcode*/ 1)
        } == 0
        {
            return Err(io::Error::last_os_error()).context("failed to terminate daemon process");
        }
        Ok(())
    }
}

pub(crate) fn try_lock_file(file: &tokio::fs::File) -> Result<bool> {
    let mut overlapped = unsafe { std::mem::zeroed() };
    if unsafe {
        LockFileEx(
            file.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            /*dwreserved*/ 0,
            /*nnumberofbytestolocklow*/ 1,
            /*nnumberofbytestolockhigh*/ 0,
            &mut overlapped,
        )
    } != 0
    {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        return Ok(false);
    }
    Err(err).context("failed to lock daemon state")
}

// Keep installer descendants bounded by the updater's lifetime, while allowing
// app-server launches and successor updaters to break away from this job.
pub(crate) fn updater_job() -> Result<OwnedHandle> {
    process_job(unsafe { GetCurrentProcess() })
}

/// Called before writing the installer script, while PowerShell is waiting on stdin.
pub(crate) fn installer_job(child: &tokio::process::Child) -> Result<OwnedHandle> {
    let process = child
        .raw_handle()
        .context("installer process handle is unavailable")?;
    process_job(process as isize)
}

fn process_job(process: isize) -> Result<OwnedHandle> {
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job == 0 {
        return Err(io::Error::last_os_error()).context("failed to create updater job");
    }
    let owned = unsafe { OwnedHandle::from_raw_handle(job as _) };
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags =
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
    if unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
        || unsafe { AssignProcessToJobObject(job, process) } == 0
    {
        return Err(io::Error::last_os_error())
            .context("failed to contain updater installer processes");
    }
    Ok(owned)
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
