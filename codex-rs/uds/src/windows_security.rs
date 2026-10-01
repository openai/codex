//! Protect Windows socket rendezvous directories with an inheritable user-only
//! DACL at creation time. Reject unsafe existing directories instead of repairing them.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenUser;
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;

pub(super) fn prepare_private_directory(path: &Path) -> io::Result<()> {
    let user = current_user()?;
    let user_sid = unsafe { (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    let mut sid_string = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user_sid, &mut sid_string) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut sid_length = 0;
    while unsafe { *sid_string.add(sid_length) } != 0 {
        sid_length += 1;
    }
    let sid =
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid_string, sid_length) });
    unsafe { LocalFree(sid_string as _) };
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;OICI;FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            /*stringsdrevision*/ 1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        // Canonicalize only the parent: this supplies Win32's extended-length
        // prefix without following a reparse point at the directory being secured.
        let absolute = std::path::absolute(path)?;
        let path = match (absolute.parent(), absolute.file_name()) {
            (Some(parent), Some(name)) => {
                std::fs::create_dir_all(parent)?;
                std::fs::canonicalize(parent)?.join(name)
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "private directory must not be a filesystem root",
                ));
            }
        };
        let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        if unsafe { CreateDirectoryW(path_wide.as_ptr(), &attributes) } != 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::AlreadyExists {
            return Err(err);
        }
        // Changing a DACL cannot revoke handles already held by another user.
        // Existing directories must already meet our private-directory contract.
        let _guard = crate::windows_socket_validation::validate_private_directory(&path)?;
        Ok(())
    })();
    unsafe { LocalFree(descriptor as _) };
    result
}

// Keep the pointer-aligned buffer alive while using its TOKEN_USER SID.
pub(super) fn current_user() -> io::Result<Vec<usize>> {
    // Token information contains pointers, so allocate pointer-aligned storage.
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _token = unsafe { OwnedHandle::from_raw_handle(token) };
    token_user(token)
}

pub(super) fn token_user(token: HANDLE) -> io::Result<Vec<usize>> {
    let mut length = 0;
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            ptr::null_mut(),
            /*tokeninformationlength*/ 0,
            &mut length,
        )
    };
    let mut user = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            user.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(user)
}
