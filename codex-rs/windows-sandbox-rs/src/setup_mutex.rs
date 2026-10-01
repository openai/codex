//! Serializes sandbox account and network changes across setup and uninstall.
//! The acquiring thread must hold the lock until those changes finish.

use std::io;
use std::marker::PhantomData;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::ptr::null_mut;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::WAIT_ABANDONED;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::System::Threading::ReleaseMutex;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use crate::winutil::to_wide;

/// Holds the machine-wide setup lock on the thread that acquired it.
#[must_use]
pub struct SandboxSetupLock {
    handle: OwnedHandle,
    _thread_bound: PhantomData<*mut ()>,
}

impl Drop for SandboxSetupLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.handle.as_raw_handle() as HANDLE);
        }
    }
}

/// Waits up to `timeout_ms` for exclusive access by LocalSystem or an elevated administrator.
pub fn acquire_sandbox_setup_lock(timeout_ms: u32) -> Result<SandboxSetupLock> {
    let sddl = to_wide("D:P(A;;GA;;;SY)(A;;GA;;;BA)");
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            /*stringsdrevision*/ 1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("create sandbox setup mutex security");
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let name = to_wide(r"Global\CodexSandboxSetup");
    let handle = unsafe {
        CreateMutexW(&attributes, /*binitialowner*/ 0, name.as_ptr())
    };
    let result = if handle.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle as _) })
    };
    unsafe { LocalFree(descriptor as HLOCAL) };
    let handle = result.context("open sandbox setup mutex")?;
    match unsafe { WaitForSingleObject(handle.as_raw_handle() as HANDLE, timeout_ms) } {
        // A crashed owner releases the lock; setup still repairs any incomplete account changes.
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(SandboxSetupLock {
            handle,
            _thread_bound: PhantomData,
        }),
        WAIT_TIMEOUT => bail!("timed out waiting for sandbox setup mutex after {timeout_ms} ms"),
        _ => Err(io::Error::last_os_error()).context("wait for sandbox setup mutex"),
    }
}
