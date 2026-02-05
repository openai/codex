use anyhow::Result;
use std::ffi::OsStr;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::System::Threading::OpenMutexW;
use windows_sys::Win32::System::Threading::ReleaseMutex;
use windows_sys::Win32::System::Threading::MUTEX_ALL_ACCESS;

use super::to_wide;

#[allow(dead_code)]
const READ_ACL_MUTEX_NAME: &str = "Local\\CodexSandboxReadAcl";
const SETUP_REFRESH_MUTEX_NAME: &str = "Local\\CodexSandboxSetupRefresh";

pub struct NamedMutexGuard {
    handle: HANDLE,
}

impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[allow(dead_code)]
fn mutex_exists(name: &str) -> Result<bool> {
    let name_w = to_wide(OsStr::new(name));
    let handle = unsafe { OpenMutexW(MUTEX_ALL_ACCESS, 0, name_w.as_ptr()) };
    if handle == 0 {
        let err = unsafe { GetLastError() };
        if err == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        return Err(anyhow::anyhow!("OpenMutexW failed for {name}: {}", err));
    }
    unsafe {
        CloseHandle(handle);
    }
    Ok(true)
}

fn acquire_mutex(name: &str) -> Result<Option<NamedMutexGuard>> {
    let name_w = to_wide(OsStr::new(name));
    let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, name_w.as_ptr()) };
    if handle == 0 {
        return Err(anyhow::anyhow!("CreateMutexW failed for {name}: {}", unsafe {
            GetLastError()
        }));
    }
    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return Ok(None);
    }
    Ok(Some(NamedMutexGuard { handle }))
}

#[allow(dead_code)]
pub fn read_acl_mutex_exists() -> Result<bool> {
    mutex_exists(READ_ACL_MUTEX_NAME)
}

#[allow(dead_code)]
pub fn acquire_read_acl_mutex() -> Result<Option<NamedMutexGuard>> {
    acquire_mutex(READ_ACL_MUTEX_NAME)
}

#[allow(dead_code)]
pub fn setup_refresh_mutex_exists() -> Result<bool> {
    mutex_exists(SETUP_REFRESH_MUTEX_NAME)
}

pub fn acquire_setup_refresh_mutex() -> Result<Option<NamedMutexGuard>> {
    acquire_mutex(SETUP_REFRESH_MUTEX_NAME)
}
