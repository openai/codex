//! Stops sandbox processes and drains logins except exact, pinned cleanup-owned tokens.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::ptr::null_mut;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Foundation::ERROR_NO_SUCH_LOGON_SESSION;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LUID;
use windows_sys::Win32::Foundation::STATUS_SUCCESS;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Security::Authentication::Identity::LsaEnumerateLogonSessions;
use windows_sys::Win32::Security::Authentication::Identity::LsaFreeReturnBuffer;
use windows_sys::Win32::Security::Authentication::Identity::LsaGetLogonSessionData;
use windows_sys::Win32::Security::Authentication::Identity::LsaNtStatusToWinError;
use windows_sys::Win32::Security::Authentication::Identity::SECURITY_LOGON_SESSION_DATA;
use windows_sys::Win32::Security::EqualSid;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::System::RemoteDesktop::WTS_CURRENT_SERVER_HANDLE;
use windows_sys::Win32::System::RemoteDesktop::WTS_PROCESS_INFOW;
use windows_sys::Win32::System::RemoteDesktop::WTSEnumerateProcessesW;
use windows_sys::Win32::System::RemoteDesktop::WTSFreeMemory;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
use windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE;
use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use super::principals::DisabledSandboxUsers;
use super::retained_logons::RetainedLogons;
use crate::token::get_user_sid_bytes;

pub(super) fn stop(users: &DisabledSandboxUsers, retained: &RetainedLogons) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let processes = sandbox_processes(users)?;
        for process in &processes {
            let handle = process.as_raw_handle() as HANDLE;
            if unsafe {
                TerminateProcess(handle, /*uexitcode*/ 1)
            } == 0
            {
                let error = io::Error::last_os_error();
                if unsafe {
                    WaitForSingleObject(handle, /*dwmilliseconds*/ 0)
                } != WAIT_OBJECT_0
                {
                    return Err(error).context("stop sandbox process before uninstall");
                }
            }
        }
        for process in &processes {
            let remaining = deadline.saturating_duration_since(Instant::now());
            // The five-second deadline keeps milliseconds within a DWORD.
            match unsafe {
                WaitForSingleObject(
                    process.as_raw_handle() as HANDLE,
                    remaining.as_millis() as u32,
                )
            } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => bail!("sandbox process did not exit before the uninstall deadline"),
                _ => {
                    return Err(io::Error::last_os_error())
                        .context("wait for sandbox process exit");
                }
            }
        }
        drop(processes);

        // A token can outlive its process or exist before a runner starts. Account disable does
        // not revoke it, so check for remaining logins before removing protections. Only
        // explicitly pinned, private SYSTEM-finalizer tokens may remain; no process is exempt.
        if !has_sandbox_logon_session(users, retained)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("sandbox processes or logon tokens remain after the uninstall deadline");
        }
        // Repeat to catch descendants created during the previous snapshot.
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn sandbox_processes(users: &DisabledSandboxUsers) -> Result<Vec<OwnedHandle>> {
    let mut process_info = null_mut();
    let mut count = 0;
    if unsafe {
        WTSEnumerateProcessesW(
            WTS_CURRENT_SERVER_HANDLE,
            /*reserved*/ 0,
            /*version*/ 1,
            &mut process_info,
            &mut count,
        )
    } == 0
    {
        return Err(io::Error::last_os_error())
            .context("enumerate sandbox processes for uninstall");
    }
    let process_info = ProcessList(process_info);
    let mut handles = Vec::new();
    if count == 0 {
        return Ok(handles);
    }
    for process in unsafe { std::slice::from_raw_parts(process_info.0, count as usize) } {
        if !is_sandbox_user(users, process.pUserSid) {
            continue;
        }
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                /*binherithandle*/ 0,
                process.ProcessId,
            )
        };
        if handle == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                continue;
            }
            return Err(error).context("open sandbox process for uninstall");
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle as *mut c_void) };
        let mut token = 0;
        if unsafe { OpenProcessToken(handle.as_raw_handle() as HANDLE, TOKEN_QUERY, &mut token) }
            == 0
        {
            let error = io::Error::last_os_error();
            if unsafe {
                WaitForSingleObject(handle.as_raw_handle() as HANDLE, /*dwmilliseconds*/ 0)
            } == WAIT_OBJECT_0
            {
                continue;
            }
            return Err(error).context("identify sandbox process before uninstall");
        }
        let token = unsafe { OwnedHandle::from_raw_handle(token as *mut c_void) };
        let sid = unsafe { get_user_sid_bytes(token.as_raw_handle() as HANDLE) }?;
        // A PID can be reused after enumeration. Check the opened process before terminating it.
        if users.sids().any(|user_sid| user_sid == sid.as_slice()) {
            handles.push(handle);
        }
    }
    Ok(handles)
}

fn has_sandbox_logon_session(
    users: &DisabledSandboxUsers,
    retained: &RetainedLogons,
) -> Result<bool> {
    let mut count = 0;
    let mut logons = null_mut();
    let status = unsafe { LsaEnumerateLogonSessions(&mut count, &mut logons) };
    if status != STATUS_SUCCESS {
        bail!("enumerate sandbox logon sessions for uninstall: {status:#x}");
    }
    let logons = LsaBuffer(logons.cast());
    if count == 0 {
        return Ok(false);
    }
    for logon in unsafe { std::slice::from_raw_parts(logons.0.cast::<LUID>(), count as usize) } {
        // LocalSystem uses the reserved logon ID 0:0x3e7 and has no normal logon data.
        if logon.LowPart == 0x3e7 && logon.HighPart == 0 {
            continue;
        }
        let mut data = null_mut();
        let status = unsafe { LsaGetLogonSessionData(logon, &mut data) };
        if status != STATUS_SUCCESS {
            if unsafe { LsaNtStatusToWinError(status) } == ERROR_NO_SUCH_LOGON_SESSION {
                continue;
            }
            bail!("read sandbox logon session for uninstall: {status:#x}");
        }
        // Keep protections when LSA returns no session data.
        if data.is_null() {
            return Ok(true);
        }
        let data = LsaBuffer(data.cast());
        let data = unsafe { &*data.0.cast::<SECURITY_LOGON_SESSION_DATA>() };
        // LSA also lists sessions without a user SID, even before sandbox accounts exist.
        // Only sessions identified as sandbox users are evidence of remaining sandbox tokens.
        if is_sandbox_user(users, data.Sid) && !retained.contains(*logon) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_sandbox_user(users: &DisabledSandboxUsers, sid: *mut c_void) -> bool {
    !sid.is_null()
        && users
            .sids()
            .any(|user_sid| unsafe { EqualSid(sid, user_sid.as_ptr() as *mut c_void) } != 0)
}

struct ProcessList(*mut WTS_PROCESS_INFOW);

impl Drop for ProcessList {
    fn drop(&mut self) {
        unsafe { WTSFreeMemory(self.0.cast()) };
    }
}

struct LsaBuffer(*mut c_void);

impl Drop for LsaBuffer {
    fn drop(&mut self) {
        unsafe { LsaFreeReturnBuffer(self.0) };
    }
}
