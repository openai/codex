//! Starts provisioning helpers only after their retained directory handles are installed.
//! The child owns its duplicates until exit, independently of the service lifetime.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::process::CommandExt;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
use windows_sys::Win32::Foundation::DuplicateHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::NTSTATUS;
use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
use windows_sys::Win32::System::Threading::GetCurrentProcess;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtResumeProcess(process_handle: HANDLE) -> NTSTATUS;
}

pub(super) fn spawn_with_retained_handles(
    command: &mut Command,
    retained_handles: &[BorrowedHandle<'_>],
) -> io::Result<Child> {
    let mut child = command
        .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let initialized = (|| {
        for handle in retained_handles {
            let mut child_handle = std::ptr::null_mut();
            if unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    handle.as_raw_handle() as HANDLE,
                    child.as_raw_handle() as HANDLE,
                    &mut child_handle,
                    /*dwdesiredaccess*/ 0,
                    /*binherithandle*/ 0,
                    DUPLICATE_SAME_ACCESS,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // This value belongs to the child's handle table. Do not close it here
            // or make it inheritable by unrelated processes or helper descendants.
        }
        let status = unsafe { NtResumeProcess(child.as_raw_handle() as HANDLE) };
        if status < 0 {
            return Err(io::Error::from_raw_os_error(
                unsafe { RtlNtStatusToDosError(status) } as i32,
            ));
        }
        Ok(())
    })();
    if let Err(error) = initialized {
        // A partially initialized helper must never run after its parent drops
        // the directory protections. Reap it, including any installed duplicates.
        child.kill()?;
        child.wait()?;
        return Err(error);
    }
    Ok(child)
}

#[cfg(test)]
#[path = "setup_launch_tests.rs"]
mod tests;
