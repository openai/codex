//! Finds desktop-created directory ownership; storage is shared with sandbox setup.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;

use anyhow::Context;
use anyhow::Result;
pub(crate) use codex_windows_sandbox::CORE_INSTALLATION_KEY;
pub(crate) use codex_windows_sandbox::DesktopInstallation;
pub(crate) use codex_windows_sandbox::INSTALLATION_KEY;
pub(crate) use codex_windows_sandbox::INSTALLATION_VALUE;
pub(crate) use codex_windows_sandbox::InstallationRecord;
pub(crate) use codex_windows_sandbox::RuntimeAccountRegistration;
pub(crate) use codex_windows_sandbox::RuntimeRegistration;
pub(crate) use codex_windows_sandbox::load_installation as load;
pub(crate) use codex_windows_sandbox::remove_installation as remove;
pub(crate) use codex_windows_sandbox::save_installation as save_runtime;
use codex_windows_sandbox::validate_local_directory_path;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::UI::Shell::GetUserProfileDirectoryW;

const DESKTOP_INSTALLATION_MARKER: &str = ".desktop-created";

// Read this app-owned marker while impersonating the authenticated user. The
// desktop writes it only when creating the home. Cache cleanup does not need ownership.
pub(crate) fn read_desktop_installation(
    home: &Path,
    user_token: foundation::HANDLE,
) -> Result<DesktopInstallation> {
    let mut length = 0;
    unsafe { GetUserProfileDirectoryW(user_token, ptr::null_mut(), &mut length) };
    let mut profile = vec![0_u16; length as usize];
    if unsafe { GetUserProfileDirectoryW(user_token, profile.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error()).context("read sandbox owner's profile directory");
    }
    let cache_home =
        PathBuf::from(OsString::from_wide(&profile[..length as usize - 1])).join(".cache");
    validate_local_directory_path(&cache_home)?;
    Ok(DesktopInstallation {
        created_codex_home: home.join(DESKTOP_INSTALLATION_MARKER).is_file(),
        cache_home,
    })
}

pub(crate) fn load_runtime() -> Result<Option<InstallationRecord>> {
    Ok(load()?.filter(|record| record.runtime.is_some()))
}

pub(crate) fn is_current_package_family(record: &InstallationRecord) -> Result<bool> {
    Ok(record.runtime()?.package_family.eq_ignore_ascii_case(
        &windows::ApplicationModel::Package::Current()?
            .Id()?
            .FamilyName()?
            .to_string(),
    ))
}

pub(crate) fn save(mut record: InstallationRecord) -> Result<InstallationRecord> {
    let _setup_lock = codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;

    record.runtime = None;
    if let Some(current) = load_runtime()? {
        let family = windows::ApplicationModel::Package::Current()?
            .Id()?
            .FamilyName()?;
        current.admit_owner(&record, &family.to_string())?;
        // The helper wait may have admitted another account. Preserve fresh Core
        // state instead of overwriting it with the caller's owner-only snapshot.
        record.runtime = current.runtime;
    }
    save_runtime(&record)?;
    Ok(record)
}
