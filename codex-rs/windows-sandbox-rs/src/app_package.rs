//! Select the requested runtime without inferring authority from directory or manifest text.
//! Registered runners still require OS package identity and the exact staged runner image.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_ACCOUNTDISABLE;
use windows_sys::Win32::Storage::Packaging::Appx::GetPackageFullName;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::QueryFullProcessImageNameW;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStagedPackagePathByFullName(
        package_full_name: *const u16,
        path_length: *mut u32,
        path: *mut u16,
    ) -> i32;
}

/// Capture the startup request once; this flag never authorizes a package or process.
pub fn registered_core_requested() -> bool {
    static REQUESTED: OnceLock<bool> = OnceLock::new();
    *REQUESTED.get_or_init(|| {
        requested_value(std::env::var_os("CODEX_WINDOWS_REGISTERED_CORE").as_deref())
    })
}

fn requested_value(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

fn current_package_full_name() -> Result<Option<String>> {
    process_package_name(unsafe { GetCurrentProcess() })
}

fn process_package_name(process: HANDLE) -> Result<Option<String>> {
    query_package_name(
        |length, buffer| unsafe { GetPackageFullName(process, length, buffer) },
        /*max_length*/ 32768,
    )
}

pub(crate) fn query_package_name(
    mut query: impl FnMut(*mut u32, *mut u16) -> u32,
    max_length: u32,
) -> Result<Option<String>> {
    let mut length = 0;
    let status = query(&mut length, std::ptr::null_mut());
    if status == APPMODEL_ERROR_NO_PACKAGE {
        return Ok(None);
    }
    ensure!(
        status == ERROR_INSUFFICIENT_BUFFER && length > 1 && length <= max_length,
        "package name query failed: {status}"
    );
    let mut family = vec![0u16; length as usize];
    let status = query(&mut length, family.as_mut_ptr());
    ensure!(
        status == ERROR_SUCCESS && length > 1 && length as usize <= family.len(),
        "package name read failed: {status}"
    );
    let family = family[..length as usize]
        .strip_suffix(&[0])
        .context("unterminated package name")?;
    ensure!(!family.contains(&0), "invalid package name");
    Ok(Some(String::from_utf16(family)?))
}

/// Preserve the caller's OS-assigned package identity for sandboxed descendants.
pub(crate) fn current_process_has_package_identity() -> Result<bool> {
    let has_identity = current_package_full_name()?.is_some();
    ensure!(
        has_identity || !registered_core_requested(),
        "registered Core process has no package identity"
    );
    Ok(has_identity)
}

fn staged_package_root(name: &[u16]) -> Result<PathBuf> {
    let mut staged = vec![0u16; 32768];
    let mut length = staged.len() as u32;
    let status =
        unsafe { GetStagedPackagePathByFullName(name.as_ptr(), &mut length, staged.as_mut_ptr()) };
    ensure!(
        status == 0 && length > 1 && length as usize <= staged.len(),
        "registered Core staged package query failed: {status}"
    );
    let staged = &staged[..length as usize];
    ensure!(
        staged.last() == Some(&0) && !staged[..staged.len() - 1].contains(&0),
        "invalid registered Core staged package path"
    );
    dunce::canonicalize(OsString::from_wide(&staged[..staged.len() - 1]))
        .context("resolve OS-staged package root")
}

pub(crate) fn verify_registered_core_runner(process: HANDLE, expected_runner: &Path) -> Result<()> {
    let name =
        process_package_name(process)?.context("registered Core runner has no package identity")?;
    let staged = staged_package_root(&crate::winutil::to_wide(name))?;
    let expected =
        dunce::canonicalize(expected_runner).context("resolve registered Core runner")?;
    ensure!(
        expected.as_os_str().eq_ignore_ascii_case(
            staged
                .join("app")
                .join("resources")
                .join("codex-command-runner.exe")
                .as_os_str()
        ),
        "registered alias selected a different package identity"
    );
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut length) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("inspect registered Core runner image");
    }
    ensure!(
        length > 0 && length as usize <= image.len(),
        "invalid registered Core image length"
    );
    let image = dunce::canonicalize(OsString::from_wide(&image[..length as usize]))?;
    ensure!(
        image.as_os_str().eq_ignore_ascii_case(expected.as_os_str()),
        "registered alias selected a different runner image"
    );
    Ok(())
}

/// Checks the service's committed receipt without provisioning or mutating either account.
pub(crate) fn registered_setup_is_ready(codex_home: &Path) -> Result<bool> {
    for account in [
        crate::setup::OFFLINE_USERNAME,
        crate::setup::ONLINE_USERNAME,
    ] {
        if crate::winutil::local_user_flags(account)?
            .is_none_or(|flags| flags & UF_ACCOUNTDISABLE != 0)
        {
            return Ok(false);
        }
    }
    let Some(package) = current_package_full_name()? else {
        return Ok(false);
    };
    let Some(record) = crate::runtime_ownership::load_installation()? else {
        return Ok(false);
    };
    Ok(record.codex_home == codex_home.canonicalize()?
        && record.runtime()?.ready_for_package(&package))
}

/// A startup hint only; the service rechecks ownership and readiness under its setup lock.
#[doc(hidden)]
pub fn registered_core_needs_refresh(codex_home: &Path) -> Result<bool> {
    let Some(package) = current_package_full_name()? else {
        return Ok(false);
    };
    let Some(record) = crate::runtime_ownership::load_installation()? else {
        return Ok(false);
    };
    let runtime = record.runtime()?;
    Ok(record.codex_home == codex_home.canonicalize()?
        && runtime
            .ready_package
            .as_deref()
            .is_some_and(|ready| ready != package && runtime.ready_for_package(ready)))
}

/// Consume the service's setup receipt; launching a command never installs a package.
pub(crate) fn registered_runner_alias(
    codex_home: &Path,
    sandbox_username: &str,
) -> anyhow::Result<PathBuf> {
    let account = if sandbox_username.eq_ignore_ascii_case(crate::setup::OFFLINE_USERNAME) {
        crate::SandboxRuntimeAccount::Offline
    } else if sandbox_username.eq_ignore_ascii_case(crate::setup::ONLINE_USERNAME) {
        crate::SandboxRuntimeAccount::Online
    } else {
        bail!("app runtime registration requires a managed sandbox account");
    };
    let record = crate::runtime_ownership::load_installation()?
        .context("registered Core setup has not completed")?;
    let package =
        current_package_full_name()?.context("registered Core launch requires an installed app")?;
    let owner = crate::winutil::resolve_sid(&crate::runtime_ownership::current_setup_user()?)?;
    let owner = crate::winutil::string_from_sid_bytes(&owner).map_err(anyhow::Error::msg)?;
    authorize_runner_receipt(&record, &owner, &codex_home.canonicalize()?, &package)?;
    let entry = record
        .runtime()?
        .accounts
        .iter()
        .find(|entry| entry.account == account)
        .context("managed account registration is not ready; retry setup")?;
    let sid = crate::winutil::resolve_sid(sandbox_username)?;
    anyhow::ensure!(
        entry.user_sid
            == crate::winutil::string_from_sid_bytes(&sid).map_err(anyhow::Error::msg)?,
        "managed runtime account changed; retry setup"
    );
    let path = entry
        .alias_path
        .as_ref()
        .context("registered Core alias is not ready; retry setup")?;
    anyhow::ensure!(
        path.is_absolute()
            && path.file_name() == Some(std::ffi::OsStr::new(crate::APP_CORE_RUNNER_ALIAS)),
        "registered Core setup contains an invalid alias"
    );
    Ok(path.clone())
}

fn authorize_runner_receipt(
    record: &crate::installation_record::InstallationRecord,
    owner: &str,
    codex_home: &Path,
    package: &str,
) -> Result<()> {
    ensure!(
        record.user_sid == owner,
        "registered Core setup belongs to another owner"
    );
    ensure!(
        record.codex_home == codex_home,
        "registered Core setup belongs to another Codex home"
    );
    let runtime = record.runtime()?;
    ensure!(
        runtime.retiring.is_none(),
        "registered Core setup is being removed"
    );
    ensure!(
        runtime.ready_for_package(package),
        "registered Core setup is not complete for the installed app; retry sandbox setup"
    );
    Ok(())
}

#[cfg(test)]
#[path = "app_package_tests.rs"]
mod tests;
