//! Selects a Windows managed release by creating or retargeting the installer junction.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::System::IO::DeviceIoControl;

pub(super) fn select_release(root: &Path, release: &Path) -> Result<()> {
    let release = release.canonicalize()?;
    let current = root.join("current");
    if current.symlink_metadata().is_err() {
        let temporary = tempfile::TempDir::new_in(root)?;
        let junction = temporary.path().join("current");
        std::fs::create_dir(&junction)?;
        retarget_junction(&junction, &release)?;
        std::fs::rename(junction, &current)?;
        return Ok(());
    }
    validate_selection(root)?;
    retarget_junction(&current, &release)
}

pub(super) fn validate_selection(root: &Path) -> Result<()> {
    let current = root.join("current");
    if matches!(current.symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(());
    }
    anyhow::ensure!(
        current.canonicalize()?.parent() == Some(root.join("releases").canonicalize()?.as_path()),
        "refusing to replace a daemon selection outside its releases directory"
    );
    Ok(())
}

fn retarget_junction(current: &Path, release: &Path) -> Result<()> {
    // Use the same mount-point reparse operation as the standalone installer.
    // Retargeting in place keeps current available to concurrent readers.
    const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    let release: Vec<u16> = release.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    let release = release.strip_prefix(prefix.as_slice()).unwrap_or(&release);
    let substitute: Vec<u8> = r"\??\"
        .encode_utf16()
        .chain(release.iter().copied())
        .flat_map(u16::to_le_bytes)
        .collect();
    u16::try_from(substitute.len() + 20).context("managed release path is too long")?;
    let length = substitute.len() as u16;
    // REPARSE_DATA_BUFFER: an 8-byte header, then four u16 byte offsets/lengths
    // for substitute and print names. The UTF-16 path buffer starts at byte 16;
    // both names have a trailing NUL, and the print name is empty.
    let mut data = vec![0; substitute.len() + 20];
    data[0..4].copy_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
    data[4..6].copy_from_slice(&(length + 12).to_le_bytes());
    data[10..12].copy_from_slice(&length.to_le_bytes());
    data[12..14].copy_from_slice(&(length + 2).to_le_bytes());
    data[16..16 + substitute.len()].copy_from_slice(&substitute);
    let handle = std::fs::OpenOptions::new()
        .access_mode(GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(current)?;
    let mut returned = 0;
    if unsafe {
        DeviceIoControl(
            handle.as_raw_handle() as isize,
            FSCTL_SET_REPARSE_POINT,
            data.as_ptr().cast(),
            data.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .context("failed to retarget managed daemon junction");
    }
    Ok(())
}

#[cfg(test)]
#[path = "prepare_install_windows_tests.rs"]
mod tests;
