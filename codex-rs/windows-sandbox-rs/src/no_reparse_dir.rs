//! Opens local directories without traversing reparse points. Callers choose
//! access, sharing, and creation behavior and retain handles for their operation.
//! Handle-relative guard files keep pinned directories from becoming reparses.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use rand::RngCore;
use std::ffi::OsStr;
use std::ffi::c_void;
use std::mem::size_of;
use std::mem::size_of_val;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
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
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::QueryDosDeviceW;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK_0;
use windows_sys::Win32::System::Kernel::OBJ_CASE_INSENSITIVE;
use windows_sys::Win32::System::Kernel::OBJ_DONT_REPARSE;

const FILE_OPEN: u32 = 1;
const FILE_CREATE: u32 = 2;
const FILE_OPEN_IF: u32 = 3;
const FILE_DIRECTORY_FILE: u32 = 1;
const FILE_NON_DIRECTORY_FILE: u32 = 0x40;
const FILE_DELETE_ON_CLOSE: u32 = 0x1000;
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

/// Whether a no-reparse open may create the final directory.
#[derive(Clone, Copy, Debug)]
pub enum DirectoryOpenDisposition {
    OpenExisting,
    OpenOrCreate,
}

/// Validates an absolute local-drive directory path without accessing the filesystem.
///
/// Relative components and alternate data streams are not directory targets.
pub fn validate_local_directory_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute(),
        "directory path must be absolute: {}",
        path.display()
    );
    let mut components = path.components();
    match components.next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {}
        _ => bail!(
            "directory path must have a local disk prefix: {}",
            path.display()
        ),
    }
    for component in components {
        match component {
            Component::RootDir => {}
            Component::Normal(name) if !name.to_string_lossy().contains(':') => {}
            Component::Normal(_) => bail!("directory path cannot contain alternate data streams"),
            Component::ParentDir | Component::CurDir | Component::Prefix(_) => {
                bail!("directory path cannot contain relative or nested prefix components");
            }
        }
    }
    Ok(())
}

/// Opens a directory without following a reparse point in any path component.
/// Parent directories must exist even when creating the final directory.
///
/// Retain the handle through handle-based security mutations. To also prevent
/// renaming or deleting the directory, omit `FILE_SHARE_DELETE` and request
/// access that participates in sharing checks, such as `FILE_TRAVERSE`.
pub fn open_directory_no_reparse(
    path: &Path,
    desired_access: u32,
    share_access: u32,
    disposition: DirectoryOpenDisposition,
) -> Result<OwnedHandle> {
    let mut nt_path = local_directory_nt_path(path)?;
    open_no_reparse(
        /*root_directory*/ 0,
        &mut nt_path,
        desired_access,
        share_access,
        match disposition {
            DirectoryOpenDisposition::OpenExisting => FILE_OPEN,
            DirectoryOpenDisposition::OpenOrCreate => FILE_OPEN_IF,
        },
        FILE_DIRECTORY_FILE,
    )
    .with_context(|| format!("open directory {}", path.display()))
}

fn local_directory_nt_path(path: &Path) -> Result<Vec<u16>> {
    validate_local_directory_path(path)?;
    let source_offset = if matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::VerbatimDisk(_))
    ) {
        4
    } else {
        0
    };
    // Windows 10 also rejects the Object Manager drive-letter link under
    // OBJ_DONT_REPARSE. Resolve that alias only; never canonicalize filesystem
    // components, which must still be checked by NtCreateFile.
    let source: Vec<u16> = path.as_os_str().encode_wide().collect();
    let drive = [source[source_offset], b':' as u16, 0];
    let mut nt_path = vec![0_u16; 32_768];
    let length =
        unsafe { QueryDosDeviceW(drive.as_ptr(), nt_path.as_mut_ptr(), nt_path.len() as u32) };
    if length == 0 {
        return Err(std::io::Error::last_os_error()).context("resolve directory drive");
    }
    nt_path.truncate(
        nt_path
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(length as usize),
    );
    ensure!(
        is_disk_volume(&nt_path),
        "directory drive must resolve directly to a disk volume"
    );
    for unit in source.into_iter().skip(source_offset + 2) {
        nt_path.push(if unit == b'/' as u16 {
            b'\\' as u16
        } else {
            unit
        });
    }
    nt_path.push(0);
    Ok(nt_path)
}

fn is_disk_volume(target: &[u16]) -> bool {
    let Some((prefix, number)) = target.split_at_checked(br"\Device\HarddiskVolume".len()) else {
        return false;
    };
    // Do not pass other devices or volume subpaths to NtCreateFile.
    prefix
        .iter()
        .zip(br"\Device\HarddiskVolume")
        .all(|(&unit, expected)| {
            u8::try_from(unit).is_ok_and(|unit| unit.eq_ignore_ascii_case(expected))
        })
        && !number.is_empty()
        && number
            .iter()
            .all(|unit| (u16::from(b'0')..=u16::from(b'9')).contains(unit))
}

/// Keeps a directory nonempty, preventing in-place reparse conversion while the
/// returned handle is held. The fresh file cannot be renamed or deleted by other
/// opens and uses delete-on-close cleanup after the guard and its duplicates close.
///
/// Creation is relative to the pinned directory, never to its mutable pathname.
/// The caller must still validate that pathname after installing the guard and
/// retain its directory pins alongside the guard throughout privileged work.
pub fn create_directory_guard(directory: BorrowedHandle<'_>) -> Result<OwnedHandle> {
    let mut random = [0_u8; 8];
    rand::rngs::OsRng
        .try_fill_bytes(&mut random)
        .context("generate sandbox directory guard name")?;
    let guard_id = u64::from_le_bytes(random);
    let name = format!(".codex-provisioning-{guard_id:016x}.guard");
    let mut name: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
    open_no_reparse(
        directory.as_raw_handle() as HANDLE,
        &mut name,
        FILE_READ_DATA | DELETE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE | FILE_DELETE_ON_CLOSE,
    )
    .context("create sandbox directory guard")
}

pub(crate) fn open_no_reparse(
    root_directory: HANDLE,
    name: &mut [u16],
    desired_access: u32,
    share_access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<OwnedHandle> {
    let name_length = u16::try_from((name.len() - 1) * size_of::<u16>())
        .context("no-reparse path is too long")?;
    let maximum_length = u16::try_from(size_of_val(name)).context("no-reparse path is too long")?;
    let object_name = UNICODE_STRING {
        Length: name_length,
        MaximumLength: maximum_length,
        Buffer: name.as_mut_ptr(),
    };
    let object_attributes = ObjectAttributes {
        length: size_of::<ObjectAttributes>() as u32,
        root_directory,
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
            desired_access,
            &object_attributes,
            &mut io_status_block,
            ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            share_access,
            disposition,
            create_options,
            ptr::null(),
            /*ea_length*/ 0,
        )
    };
    if status < 0 {
        if status == STATUS_REPARSE_POINT_ENCOUNTERED {
            bail!("path contains a reparse point");
        }
        let error = unsafe { RtlNtStatusToDosError(status) };
        return Err(std::io::Error::from_raw_os_error(error as i32)).context("NtCreateFile");
    }
    ensure!(
        handle != 0 && handle != INVALID_HANDLE_VALUE,
        "NtCreateFile returned an invalid handle"
    );
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as *mut c_void) })
}

#[cfg(test)]
#[path = "no_reparse_dir_tests.rs"]
mod tests;
