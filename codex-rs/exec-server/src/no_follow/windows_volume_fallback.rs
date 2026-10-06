//! Retry drive-letter opens from a verified physical volume root when strict NT opens reject
//! the DOS alias. The root must be a local directory; all path components below it stay no-follow.

use super::*;
use std::os::windows::fs::MetadataExt;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::FILE_TRAVERSE;
use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
use windows_sys::Win32::Storage::FileSystem::VOLUME_NAME_NT;

const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const DOS_DRIVE_ROOT_LEN: usize = r"\??\C:\".len();

pub(super) fn open_relative_to_volume(
    name: &mut [u16],
    desired_access: u32,
    create_disposition: u32,
    create_options: u32,
) -> io::Result<OwnedHandle> {
    let mut root_name = name[..DOS_DRIVE_ROOT_LEN].to_vec();
    root_name.push(0);
    let root = open_volume_root(&mut root_name)?;
    open_nt_handle(
        &mut name[DOS_DRIVE_ROOT_LEN..],
        Some(&root),
        desired_access,
        create_disposition,
        create_options,
        OBJ_DONT_REPARSE,
    )?
    .ok_or_else(reparse_error)
}

pub(super) fn open_volume_root(name: &mut [u16]) -> io::Result<OwnedHandle> {
    let handle = open_nt_handle(
        name,
        /*root_directory*/ None,
        FILE_READ_ATTRIBUTES | FILE_TRAVERSE,
        FILE_OPEN,
        FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT,
        /*attributes*/ 0,
    )?
    .ok_or_else(reparse_error)?;
    let root = std::fs::File::from(handle);
    let metadata = root.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(reparse_error());
    }

    let mut name = [0u16; 128];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            root.as_raw_handle(),
            name.as_mut_ptr(),
            name.len() as u32,
            VOLUME_NAME_NT,
        )
    } as usize;
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    if length >= name.len() || !is_local_volume_root(&name[..length]) {
        return Err(reparse_error());
    }
    Ok(root.into())
}

pub(super) fn is_local_volume_root(name: &[u16]) -> bool {
    String::from_utf16(name).is_ok_and(|name| {
        name.strip_prefix(r"\Device\HarddiskVolume")
            .and_then(|suffix| suffix.strip_suffix('\\'))
            .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
    })
}
