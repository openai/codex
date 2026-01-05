#![cfg(target_os = "windows")]

use crate::logging::log_note;
use crate::winutil::format_last_error;
use crate::winutil::to_wide;
use anyhow::anyhow;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
use windows_sys::Win32::Storage::FileSystem::SetFileAttributesW;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SYSTEM;
use windows_sys::Win32::Storage::FileSystem::INVALID_FILE_ATTRIBUTES;
use windows_sys::Win32::System::Registry::RegCloseKey;
use windows_sys::Win32::System::Registry::RegCreateKeyExW;
use windows_sys::Win32::System::Registry::RegSetValueExW;
use windows_sys::Win32::System::Registry::HKEY;
use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;
use windows_sys::Win32::System::Registry::KEY_WRITE;
use windows_sys::Win32::System::Registry::REG_DWORD;
use windows_sys::Win32::System::Registry::REG_OPTION_NON_VOLATILE;

const USERLIST_KEY_PATH: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList";

pub fn hide_newly_created_users(usernames: &[String], log_base: &Path) {
    if usernames.is_empty() {
        return;
    }
    if let Err(err) = hide_users_in_winlogon(usernames, log_base) {
        log_note(
            &format!("hide users: failed to update Winlogon UserList: {err}"),
            Some(log_base),
        );
    }
}

/// Best-effort: hides the current sandbox user's profile directory once it exists.
///
/// Windows only creates profile directories when that user first logs in.
/// This intentionally runs in the command-runner (as the sandbox user) because
/// command running is what causes us to log in as a particular sandbox user.
pub fn hide_current_user_profile_dir(log_base: &Path) {
    let Some(profile) = std::env::var_os("USERPROFILE") else {
        return;
    };
    let profile_dir = PathBuf::from(profile);
    if !profile_dir.exists() {
        return;
    }

    match hide_directory(&profile_dir) {
        Ok(true) => {
            // Log only when we actually change attributes, so this stays one-time per profile dir.
            log_note(
                &format!(
                    "hide users: profile dir hidden for current user ({})",
                    profile_dir.display()
                ),
                Some(log_base),
            );
        }
        Ok(false) => {}
        Err(err) => {
            log_note(
                &format!(
                    "hide users: failed to hide current user profile dir ({}): {err}",
                    profile_dir.display()
                ),
                Some(log_base),
            );
        }
    }
}

fn hide_users_in_winlogon(usernames: &[String], log_base: &Path) -> anyhow::Result<()> {
    let key = create_userlist_key()?;
    for username in usernames {
        let name_w = to_wide(OsStr::new(username));
        let value: u32 = 0;
        let status = unsafe {
            RegSetValueExW(
                key,
                name_w.as_ptr(),
                0,
                REG_DWORD,
                &value as *const u32 as *const u8,
                std::mem::size_of_val(&value) as u32,
            )
        };
        if status != 0 {
            log_note(
                &format!(
                    "hide users: failed to set UserList value for {username}: {status} ({error})",
                    error = format_last_error(status as i32)
                ),
                Some(log_base),
            );
        }
    }
    unsafe {
        RegCloseKey(key);
    }
    Ok(())
}

fn create_userlist_key() -> anyhow::Result<HKEY> {
    let key_path = to_wide(USERLIST_KEY_PATH);
    let mut key: HKEY = 0;
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            key_path.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null_mut(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(anyhow!(
            "RegCreateKeyExW failed: {status} ({error})",
            error = format_last_error(status as i32)
        ));
    }
    Ok(key)
}

/// Sets HIDDEN|SYSTEM on `path` if needed, returning whether it changed anything.
fn hide_directory(path: &Path) -> anyhow::Result<bool> {
    let wide = to_wide(path);
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        let err = unsafe { GetLastError() } as i32;
        return Err(anyhow!(
            "GetFileAttributesW failed for {}: {err} ({error})",
            path.display(),
            error = format_last_error(err)
        ));
    }
    let new_attrs = attrs | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;
    if new_attrs == attrs {
        return Ok(false);
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
    if ok == 0 {
        let err = unsafe { GetLastError() } as i32;
        return Err(anyhow!(
            "SetFileAttributesW failed for {}: {err} ({error})",
            path.display(),
            error = format_last_error(err)
        ));
    }
    Ok(true)
}
