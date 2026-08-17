use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use std::ffi::OsStr;
use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Component;
use std::path::Path;
use std::path::Prefix;
use std::ptr;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::NTSTATUS;
use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
use windows_sys::Win32::Foundation::UNICODE_STRING;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::WRITE_DAC;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK_0;
use windows_sys::Win32::System::Kernel::OBJ_CASE_INSENSITIVE;
use windows_sys::Win32::System::Kernel::OBJ_DONT_REPARSE;

const FILE_OPEN_IF: u32 = 3;
const FILE_DIRECTORY_FILE: u32 = 1;
const STATUS_REPARSE_POINT_ENCOUNTERED: NTSTATUS = 0xC000_050B_u32 as i32;

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: HANDLE,
    object_name: *const UNICODE_STRING,
    attributes: u32,
    security_descriptor: *const c_void,
    security_quality_of_service: *const c_void,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        file_handle: *mut HANDLE,
        desired_access: u32,
        object_attributes: *const ObjectAttributes,
        io_status_block: *mut IO_STATUS_BLOCK,
        allocation_size: *const i64,
        file_attributes: u32,
        share_access: u32,
        create_disposition: u32,
        create_options: u32,
        ea_buffer: *const c_void,
        ea_length: u32,
    ) -> NTSTATUS;
}

/// Opens or creates the final directory without following a reparse point in
/// any path component.
///
/// Parent directories must already exist; only the final directory is created.
///
/// The returned handle must remain open through any security mutation so the
/// mutation stays bound to the directory that passed this validation.
pub(super) fn open_or_create_no_reparse(path: &Path) -> Result<OwnedHandle> {
    ensure!(
        path.is_absolute(),
        "sandbox ACL path must be absolute: {}",
        path.display()
    );
    let source_offset = match path.components().next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)) => 0,
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::VerbatimDisk(_)) => 4,
        _ => bail!(
            "sandbox ACL path must have a local disk prefix: {}",
            path.display()
        ),
    };
    // NtCreateFile expects an NT namespace path. Convert only the local-disk
    // spellings accepted by provisioning, without canonicalizing and following
    // the reparse points that OBJ_DONT_REPARSE must reject.
    let source: Vec<u16> = path.as_os_str().encode_wide().collect();
    let mut nt_path: Vec<u16> = OsStr::new("\\??\\").encode_wide().collect();
    for unit in source.into_iter().skip(source_offset) {
        nt_path.push(if unit == b'/' as u16 {
            b'\\' as u16
        } else {
            unit
        });
    }
    nt_path.push(0);
    let name_length = u16::try_from((nt_path.len() - 1) * size_of::<u16>())
        .context("sandbox ACL path is too long")?;
    let maximum_length =
        u16::try_from(nt_path.len() * size_of::<u16>()).context("sandbox ACL path is too long")?;
    let object_name = UNICODE_STRING {
        Length: name_length,
        MaximumLength: maximum_length,
        Buffer: nt_path.as_mut_ptr(),
    };
    let object_attributes = ObjectAttributes {
        length: size_of::<ObjectAttributes>() as u32,
        root_directory: 0,
        object_name: &object_name,
        attributes: OBJ_CASE_INSENSITIVE as u32 | OBJ_DONT_REPARSE as u32,
        security_descriptor: ptr::null(),
        security_quality_of_service: ptr::null(),
    };
    let mut io_status_block = IO_STATUS_BLOCK {
        Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
        Information: 0,
    };
    let mut handle = 0;
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            WRITE_DAC,
            &object_attributes,
            &mut io_status_block,
            ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN_IF,
            FILE_DIRECTORY_FILE,
            ptr::null(),
            /*ea_length*/ 0,
        )
    };
    if status < 0 {
        if status == STATUS_REPARSE_POINT_ENCOUNTERED {
            bail!(
                "sandbox ACL path contains a reparse point: {}",
                path.display()
            );
        }
        let error = unsafe { RtlNtStatusToDosError(status) };
        return Err(std::io::Error::from_raw_os_error(error as i32))
            .with_context(|| format!("open sandbox ACL directory {}", path.display()));
    }
    ensure!(
        handle != 0 && handle != INVALID_HANDLE_VALUE,
        "NtCreateFile returned an invalid sandbox ACL directory handle"
    );
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as *mut c_void) })
}

#[cfg(test)]
#[path = "no_reparse_dir_tests.rs"]
mod tests;
