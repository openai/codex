//! Creates replacement files with the destination DACL before any secret is written.
//! Protect the captured DACL from broader parent inheritance during creation.

use std::ffi::c_void;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::GetSecurityInfo;
use windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::SE_DACL_PROTECTED;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::SetSecurityDescriptorControl;
use windows_sys::Win32::Storage::FileSystem::CREATE_NEW;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;

struct Descriptor(*mut c_void);

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: GetSecurityInfo allocates this descriptor with LocalAlloc.
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(in crate::auth::storage) fn replacement(
    path: &Path,
) -> io::Result<tempfile::NamedTempFile<File>> {
    let descriptor = match OpenOptions::new().access_mode(READ_CONTROL).open(path) {
        Ok(file) => {
            // SAFETY: BY_HANDLE_FILE_INFORMATION is an all-integer output structure.
            let mut info = unsafe { std::mem::zeroed() };
            // SAFETY: file is open and info is a valid output structure.
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if info.nNumberOfLinks > 1 {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "atomic credential replacement does not support hard-linked files",
                ));
            }
            let mut descriptor = ptr::null_mut();
            // SAFETY: the file handle and output pointer remain valid for this call.
            let status = unsafe {
                GetSecurityInfo(
                    file.as_raw_handle(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut descriptor,
                )
            };
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status as i32));
            }
            let descriptor = Descriptor(descriptor);
            // SAFETY: this is the owned descriptor returned by GetSecurityInfo.
            if unsafe {
                SetSecurityDescriptorControl(descriptor.0, SE_DACL_PROTECTED, SE_DACL_PROTECTED)
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            Some(descriptor)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let parent = std::fs::canonicalize(
        path.parent()
            .ok_or_else(|| io::Error::other("credential file has no parent"))?,
    )?;
    tempfile::Builder::new()
        .prefix(".auth-")
        .make_in(parent, |temporary| {
            let name: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.as_ref().map_or(ptr::null_mut(), |value| value.0),
                bInheritHandle: 0,
            };
            // SAFETY: name is NUL-terminated; the owned descriptor outlives creation.
            // CREATE_NEW prevents opening an attacker-created temporary path.
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    &attributes,
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL,
                    ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: this is a newly opened handle owned exclusively by this File.
            Ok(unsafe { File::from_raw_handle(handle) })
        })
}

#[cfg(test)]
#[path = "storage_file_windows_tests.rs"]
mod tests;
