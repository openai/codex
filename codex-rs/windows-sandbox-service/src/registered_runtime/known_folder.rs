//! Resolves Known Folders for an explicit token and caller-selected flags.
//! Preserve Windows UTF-16 paths and free the returned allocation on every result.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use anyhow::Result;
use anyhow::ensure;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
use windows_sys::core::GUID;

pub(super) fn path(token: HANDLE, folder: &GUID, flags: u32) -> Result<PathBuf> {
    let mut raw = std::ptr::null_mut();
    let status = unsafe { SHGetKnownFolderPath(folder, flags, token, &mut raw) };
    let result = (|| {
        windows::core::HRESULT(status).ok()?;
        ensure!(!raw.is_null(), "Known Folder path missing");
        let mut length = 0;
        while length < 32768 && unsafe { *raw.add(length) } != 0 {
            length += 1;
        }
        ensure!(length < 32768, "Known Folder path exceeds bound");
        Ok(PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, length)
        })))
    })();
    unsafe { CoTaskMemFree(raw.cast()) };
    result
}
