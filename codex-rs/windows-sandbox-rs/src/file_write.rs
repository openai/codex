//! Writes helper outputs through fresh handles, replacing directory entries
//! without opening caller-supplied destination files or copying their ACLs.

use anyhow::Context;
use anyhow::Result;
use rand::RngCore;
use std::ffi::OsStr;
use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::mem::offset_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::NTSTATUS;
use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_DISPOSITION_INFO;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_TRAVERSE;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FileDispositionInfo;
use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
use windows_sys::Win32::Storage::FileSystem::SetFileInformationByHandle;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK_0;

use crate::no_reparse_dir::DirectoryOpenDisposition;
use crate::no_reparse_dir::open_directory_no_reparse;
use crate::no_reparse_dir::open_no_reparse;
use crate::no_reparse_dir::validate_local_directory_path;

const FILE_CREATE: u32 = 2;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
const FILE_NON_DIRECTORY_FILE: u32 = 0x40;
const FILE_RENAME_INFORMATION: i32 = 10;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetInformationFile(
        file_handle: HANDLE,
        io_status_block: *mut IO_STATUS_BLOCK,
        file_information: *const c_void,
        length: u32,
        file_information_class: i32,
    ) -> NTSTATUS;
}

/// Replaces an output with a fresh file that inherits its parent directory's ACL.
/// The destination is never opened for writing, even if replacement fails.
pub fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    validate_local_directory_path(path)?;
    let parent = path
        .parent()
        .context("output must have a parent directory")?;
    let name: Vec<u16> = path
        .file_name()
        .context("output must have a file name")?
        .encode_wide()
        .collect();
    let (mut file, directory) = create_temporary_file(parent, ".tmp")?;
    let result: Result<()> = (|| {
        file.write_all(contents)?;

        // FILE_RENAME_INFO ends in a variable-length UTF-16 name. Allocate
        // enough aligned storage for both the fixed header and that name.
        let size = size_of::<FILE_RENAME_INFO>() + size_of_val(name.as_slice());
        let mut buffer = vec![0_usize; size.div_ceil(size_of::<usize>())];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        let mut io_status = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        unsafe {
            (*info).Anonymous.ReplaceIfExists = true;
            (*info).RootDirectory = directory.as_raw_handle() as HANDLE;
            (*info).FileNameLength = u32::try_from(size_of_val(name.as_slice()))?;
            let filename = buffer
                .as_mut_ptr()
                .cast::<u8>()
                .add(offset_of!(FILE_RENAME_INFO, FileName))
                .cast::<u16>();
            std::ptr::copy_nonoverlapping(name.as_ptr(), filename, name.len());
            // The Win32 wrapper rejects RootDirectory on some Windows versions.
            // The native rename accepts the pinned parent and has the same layout.
            let status = NtSetInformationFile(
                file.as_raw_handle() as HANDLE,
                &mut io_status,
                info.cast(),
                u32::try_from(size)?,
                FILE_RENAME_INFORMATION,
            );
            if status < 0 {
                return Err(
                    std::io::Error::from_raw_os_error(RtlNtStatusToDosError(status) as i32).into(),
                );
            }
        }
        Ok(())
    })();
    if result.is_err() {
        // Clean up only the file we created, not a pathname the caller could
        // have replaced. Preserve the original write/rename error.
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle() as HANDLE,
                FileDispositionInfo,
                (&raw const disposition).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            );
        }
    }
    result.with_context(|| format!("replace output {}", path.display()))
}

pub(crate) fn create_temporary_file(parent: &Path, suffix: &str) -> Result<(File, OwnedHandle)> {
    let directory = open_directory_no_reparse(
        parent,
        FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        DirectoryOpenDisposition::OpenExisting,
    )?;
    let mut random = [0_u8; 8];
    rand::rngs::OsRng
        .try_fill_bytes(&mut random)
        .context("generate sandbox output name")?;
    let id = u64::from_le_bytes(random);
    let name = format!("sandbox-{id:016x}{suffix}");
    let mut name: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
    let file = open_no_reparse(
        directory.as_raw_handle() as HANDLE,
        &mut name,
        FILE_WRITE_DATA | DELETE | SYNCHRONIZE,
        FILE_SHARE_READ,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
    )?;
    Ok((File::from(file), directory))
}

#[cfg(test)]
#[path = "file_write_tests.rs"]
mod tests;
