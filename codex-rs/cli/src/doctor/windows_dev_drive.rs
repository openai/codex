use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;

use codex_git_utils::get_git_repo_root;
use os_info::Version;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_NO_ROOT_DIR;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_UNKNOWN;

use super::CheckStatus;
use super::DoctorCheck;

const ID: &str = "git.worktree.dev_drive";
const DEV_VOLUME: u32 = 0x0000_2000;
const TRUSTED_VOLUME: u32 = 0x0000_4000;
const QUERY_PERSISTENT_VOLUME_STATE: u32 = 0x0009_023c;

#[repr(C)]
struct PersistentVolumeState {
    flags: u32,
    mask: u32,
    version: u32,
    reserved: u32,
}

pub(super) fn check(cwd: &Path) -> DoctorCheck {
    let Some(worktree) = get_git_repo_root(cwd) else {
        return DoctorCheck::new(ID, "git", CheckStatus::Ok, "no Git worktree is active");
    };
    if matches!(
        os_info::get().version(),
        Version::Semantic(_, _, build) if *build < 22_621
    ) {
        return DoctorCheck::new(
            ID,
            "git",
            CheckStatus::Ok,
            "Windows Dev Drives are unavailable on this Windows version",
        );
    }

    match volume_flags(&worktree) {
        Ok(flags) if flags & DEV_VOLUME == 0 => DoctorCheck::new(
            ID,
            "git",
            CheckStatus::Warning,
            "this worktree is not on a Windows Dev Drive; moving it to a trusted Dev Drive can significantly improve repository and filesystem performance",
        )
        .remediation(
            "create a trusted Windows Dev Drive for source repositories: https://learn.microsoft.com/en-us/windows/dev-drive/",
        ),
        Ok(flags) if flags & TRUSTED_VOLUME == 0 => DoctorCheck::new(
            ID,
            "git",
            CheckStatus::Warning,
            "the active Git worktree is on an untrusted Windows Dev Drive",
        )
        .remediation(
            "ask your administrator to trust the Windows Dev Drive: https://learn.microsoft.com/en-us/windows/dev-drive/#how-do-i-designate-a-dev-drive-as-trusted",
        ),
        Ok(_) => DoctorCheck::new(
            ID,
            "git",
            CheckStatus::Ok,
            "the active Git worktree is on a trusted Windows Dev Drive",
        ),
        Err(error) => DoctorCheck::new(
            ID,
            "git",
            CheckStatus::Warning,
            "the active Git worktree's Windows Dev Drive state could not be inspected",
        )
        .detail(format!("filesystem error: {:?}", error.kind()))
        .remediation("check access to the Git worktree and its Windows storage volume"),
    }
}

fn volume_flags(path: &Path) -> io::Result<u32> {
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut volume = vec![0_u16; path.len().max(261)];
    if unsafe { GetVolumePathNameW(path.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) } == 0 {
        return Err(io::Error::last_os_error());
    }
    match unsafe { GetDriveTypeW(volume.as_ptr()) } {
        DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the Windows storage volume could not be determined",
            ));
        }
        DRIVE_FIXED => {}
        _ => return Ok(0),
    }

    let length = volume
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(volume.len());
    let root = PathBuf::from(OsString::from_wide(&volume[..length]));
    let handle = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(root)?;
    let mut state = PersistentVolumeState {
        flags: 0,
        mask: DEV_VOLUME | TRUSTED_VOLUME,
        version: 1,
        reserved: 0,
    };
    let state_size = std::mem::size_of::<PersistentVolumeState>() as u32;
    let state_ptr = ptr::from_mut(&mut state);
    let mut bytes_returned = 0;
    if unsafe {
        DeviceIoControl(
            handle.as_raw_handle() as isize,
            QUERY_PERSISTENT_VOLUME_STATE,
            state_ptr.cast_const().cast(),
            state_size,
            state_ptr.cast(),
            state_size,
            &mut bytes_returned,
            ptr::null_mut(),
        )
    } == 0
    {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(0)
        } else {
            Err(error)
        };
    }

    Ok(state.flags)
}

#[cfg(test)]
#[path = "windows_dev_drive_tests.rs"]
mod tests;
