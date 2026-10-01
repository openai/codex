//! Read-only validation of Windows daemon rendezvous directories. Pin the
//! validated directory while connecting; never repair an untrusted listener's ACL.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::ACCESS_ALLOWED_ACE;
use windows_sys::Win32::Security::ACE_HEADER;
use windows_sys::Win32::Security::Authorization::GetSecurityInfo;
use windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows_sys::Win32::Security::CONTAINER_INHERIT_ACE;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::EqualSid;
use windows_sys::Win32::Security::GetAce;
use windows_sys::Win32::Security::GetSecurityDescriptorControl;
use windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
use windows_sys::Win32::Security::OWNER_SECURITY_INFORMATION;
use windows_sys::Win32::Security::SE_DACL_PROTECTED;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::FILE_LIST_DIRECTORY;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;

/// Validates an existing daemon socket directory without creating or changing it.
/// Connect using the returned path and retain the handle through connection setup
/// to prevent replacement of the validated directory. Ancestor junctions are
/// resolved before validation; the socket directory itself must not be a reparse point.
pub fn validate_private_socket_path(socket_path: &Path) -> io::Result<(PathBuf, OwnedHandle)> {
    let absolute = std::path::absolute(socket_path)?;
    let directory = absolute.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let socket_name = absolute.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let (directory, guard) = validate_private_directory(directory)?;
    Ok((directory.join(socket_name), guard))
}

pub(super) fn validate_private_directory(directory: &Path) -> io::Result<(PathBuf, OwnedHandle)> {
    let parent = directory.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let directory_name = directory.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let directory = std::fs::canonicalize(parent)?.join(directory_name);
    let path_wide: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    // Metadata-only access does not enforce sharing restrictions on Windows.
    // Directory listing access makes omission of FILE_SHARE_DELETE pin the path.
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            READ_CONTROL | FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let guard = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted socket directory",
        ));
    }
    let user = crate::windows_security::current_user()?;
    let user_sid = unsafe { (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    let mut owner = ptr::null_mut();
    let mut dacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // Accept exactly the protected, inheritable user-only ACL set at daemon bind.
    let trusted = unsafe {
        let mut control = 0;
        let mut revision = 0;
        let mut ace = ptr::null_mut();
        !owner.is_null()
            && EqualSid(owner, user_sid) != 0
            && GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) != 0
            && control & SE_DACL_PROTECTED != 0
            && !dacl.is_null()
            && (*dacl).AceCount == 1
            && GetAce(dacl, /*dwaceindex*/ 0, &mut ace) != 0
            && (*ace.cast::<ACE_HEADER>()).AceType == ACCESS_ALLOWED_ACE_TYPE as u8
            && (*ace.cast::<ACE_HEADER>()).AceFlags
                == (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE) as u8
            && (*ace.cast::<ACCESS_ALLOWED_ACE>()).Mask == FILE_ALL_ACCESS
            && EqualSid(
                ptr::addr_of_mut!((*ace.cast::<ACCESS_ALLOWED_ACE>()).SidStart).cast(),
                user_sid,
            ) != 0
    };
    unsafe { LocalFree(descriptor as _) };
    if !trusted {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket directory is not private to the current user",
        ));
    }
    Ok((directory, guard))
}
