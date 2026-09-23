//! Owns managed-account registration, AppData metadata access, and deferred removal.
//! Lifecycle and admission decisions stay with callers. Persist ownership before deployment;
//! retain profiles until Windows completes, or pending resources until service exit on shutdown.

use std::os::windows::io::AsRawHandle;
use std::os::windows::io::OwnedHandle;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::APP_CORE_RUNNER_ALIAS;
use codex_windows_sandbox::SandboxRuntimeAccount;
use codex_windows_sandbox::logon_existing_sandbox_account;
use codex_windows_sandbox::resolve_sid as lookup_sid;
use codex_windows_sandbox::sandbox_secrets_dir;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::token_groups;
use windows::Foundation::AsyncStatus;
use windows::Management::Deployment::DeploymentOptions;
use windows::Management::Deployment::PackageManager;
use windows::Win32::Foundation::BOOL;
use windows::core::HSTRING;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::System::SystemServices::SE_GROUP_ENABLED;
use windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData;
use windows_sys::Win32::UI::Shell::LoadUserProfileW;
use windows_sys::Win32::UI::Shell::PROFILEINFOW;
use windows_sys::Win32::UI::Shell::UnloadUserProfile;

use crate::installation_record::InstallationRecord;
use crate::installation_record::RuntimeAccountRegistration;
use crate::ipc::ClientIdentity;
use crate::package_lifecycle::with_owner_impersonation;

mod known_folder;
mod metadata;
mod removal;

pub(crate) use metadata::remove as remove_metadata;
pub(crate) use removal::prepare as prepare_removal;
pub(crate) use removal::restore_disabled_accounts;

/// Must escape request handling: pending resources are retained until this service exits.
#[derive(Debug)]
pub(crate) struct RegistrationInterrupted;

impl std::fmt::Display for RegistrationInterrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("runtime setup interrupted; stopping service")
    }
}

impl std::error::Error for RegistrationInterrupted {}

struct AccountProfile {
    user_sid: String,
    token: OwnedHandle,
    profile: HANDLE,
}

impl Drop for AccountProfile {
    fn drop(&mut self) {
        let _ = self.unload_profile();
    }
}

impl AccountProfile {
    fn unload_profile(&mut self) -> Result<()> {
        if self.profile != 0 {
            let unloaded =
                unsafe { UnloadUserProfile(self.token.as_raw_handle() as _, self.profile) };
            BOOL(unloaded)
                .ok()
                .context("unload managed runtime account profile")?;
            self.profile = 0;
        }
        Ok(())
    }
}

/// The caller holds the setup mutex from owner admission through readiness.
/// Provision both users together. No runner launch performs package deployment.
pub(crate) fn provision(
    identity: &ClientIdentity,
    record: &mut InstallationRecord,
    full_name: &HSTRING,
    shutdown: &AtomicBool,
) -> Result<()> {
    let family = record.runtime()?.package_family.clone();
    validate_record(record)?;
    let mut profiles = Vec::new();
    let mut accounts = Vec::new();
    for account in [
        SandboxRuntimeAccount::Offline,
        SandboxRuntimeAccount::Online,
    ] {
        let token = with_owner_impersonation(identity.token.0, || {
            let mut pins = Vec::new();
            crate::ipc::pin_existing_ancestors(
                &sandbox_secrets_dir(&identity.codex_home),
                &mut pins,
            )?;
            logon_existing_sandbox_account(&identity.codex_home, account)
        })
        .with_context(|| format!("log on managed runtime account {account:?}"))?;
        let expected = record
            .runtime()?
            .accounts
            .iter()
            .find(|entry| entry.account == account);
        let profile = load_profile(
            token,
            account,
            expected.map(|entry| entry.user_sid.as_str()),
        )
        .with_context(|| format!("prepare managed runtime profile for {account:?}"))?;
        let installed = registered_packages(&profile.user_sid, &family)
            .with_context(|| format!("query existing runtime packages for {account:?}"))?;
        if expected.is_none() {
            ensure!(
                installed.is_empty(),
                "sandbox account has a runtime registration not owned by this service"
            );
        }
        let alias_path = known_folder::path(
            profile.token.as_raw_handle() as _,
            &FOLDERID_LocalAppData,
            /*flags*/ 0,
        )
        .with_context(|| format!("resolve runtime alias location for {account:?}"))?
        .join(r"Microsoft\WindowsApps")
        .join(&family)
        .join(APP_CORE_RUNNER_ALIAS);
        accounts.push(RuntimeAccountRegistration {
            cleanup_logon_pending: false,
            account,
            user_sid: profile.user_sid.clone(),
            alias_path: Some(alias_path),
        });
        profiles.push((profile, installed.contains(full_name)));
    }
    let receipt_current = record.runtime()?.ready_for_package(&full_name.to_string())
        && record.runtime()?.accounts == accounts
        && profiles.iter().all(|(_, installed)| *installed);
    // Both exact SIDs are durable before either package registration or metadata grant.
    // An idempotent setup check must not temporarily revoke a valid launch receipt.
    record.runtime_mut()?.accounts = accounts;
    if !receipt_current {
        record.runtime_mut()?.ready_package = None;
        crate::installation_record::save_runtime(record)
            .context("persist managed runtime account registration")?;
    }
    for (mut profile, installed) in profiles {
        if !installed {
            let operation = with_owner_impersonation(profile.token.as_raw_handle() as _, || {
                Ok(PackageManager::new()?.RegisterPackageByFullNameAsync(
                    full_name,
                    None,
                    DeploymentOptions::None,
                )?)
            })?;
            // As with provisioning, retain the request's resources until Windows finishes.
            // A client timeout or an unreadable status cannot safely release the profile.
            let mut status_error_logged = false;
            loop {
                if shutdown.load(Ordering::Acquire) {
                    // The typed error bypasses all response conversion and cleanup. Keep the
                    // in-flight operation and its profile alive until this own-process service exits.
                    std::mem::forget((operation, profile));
                    return Err(RegistrationInterrupted.into());
                }
                match operation.Status() {
                    Ok(AsyncStatus::Started) => {}
                    Ok(AsyncStatus::Completed | AsyncStatus::Canceled | AsyncStatus::Error) => {
                        break;
                    }
                    status => {
                        if !status_error_logged {
                            crate::service::log_error(
                                crate::service::EVENT_SERVICE_FAILED,
                                &format!(
                                    "runtime registration status unavailable; retaining request: {status:?}"
                                ),
                            );
                            status_error_logged = true;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            operation.GetResults()?.ExtendedErrorCode()?.ok()?;
            ensure!(
                registered_packages(&profile.user_sid, &family)?.contains(full_name),
                "runtime registration completed without the expected package"
            );
        }
        metadata::grant(identity.token.0, &profile.user_sid, record)?;
        profile.unload_profile()?;
    }
    if !receipt_current {
        record.runtime_mut()?.ready_package = Some(full_name.to_string());
        crate::installation_record::save_runtime(record)?;
    }
    Ok(())
}

/// Validate cleanup ownership under the caller's setup lock before obtaining logons.
pub(crate) fn prepare_cleanup(record: InstallationRecord) -> Result<InstallationRecord> {
    validate_record(&record)?;
    for account in &record.runtime()?.accounts {
        if codex_windows_sandbox::local_user_flags(account.account.username())?.is_some() {
            validate_account_sid(account)?;
        }
    }
    ensure!(
        record.runtime()?.retiring.is_none(),
        "interrupted runtime cleanup requires repair"
    );
    ensure!(
        record
            .runtime()?
            .accounts
            .iter()
            .all(|account| !account.cleanup_logon_pending),
        "cleanup logon recovery is pending"
    );
    Ok(record)
}

fn validate_account_sid(account: &RuntimeAccountRegistration) -> Result<()> {
    let sid = lookup_sid(account.account.username())?;
    ensure!(
        string_from_sid_bytes(&sid).map_err(anyhow::Error::msg)? == account.user_sid,
        "managed runtime account was replaced before cleanup"
    );
    Ok(())
}

fn validate_record(record: &InstallationRecord) -> Result<()> {
    let runtime = record.runtime()?;
    ensure!(
        runtime.accounts.len() <= 2,
        "runtime registration record has too many accounts"
    );
    if runtime.accounts.len() == 2 {
        ensure!(
            runtime.accounts[0].account != runtime.accounts[1].account,
            "runtime registration record contains duplicate accounts"
        );
    }
    Ok(())
}

fn registered_packages(user_sid: &str, family: &str) -> Result<Vec<HSTRING>> {
    let packages = PackageManager::new()?.FindPackagesByUserSecurityIdPackageFamilyName(
        &HSTRING::from(user_sid),
        &HSTRING::from(family),
    )?;
    // The Rust Iterator adapter suppresses Current/MoveNext errors. Absence must
    // be authoritative before releasing cleanup ownership or its package anchor.
    let iterator = packages.First()?;
    let mut names = Vec::new();
    let mut has_current = iterator.HasCurrent()?;
    while has_current {
        names.push(iterator.Current()?.Id()?.FullName()?);
        has_current = iterator.MoveNext()?;
    }
    Ok(names)
}

/// An intent is not an anchor: failed registration may leave no managed package.
pub(crate) fn has_retaining_registration(record: &InstallationRecord) -> Result<bool> {
    let runtime = record.runtime()?;
    for account in &runtime.accounts {
        if !registered_packages(&account.user_sid, &runtime.package_family)?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_profile(
    token: OwnedHandle,
    account: SandboxRuntimeAccount,
    expected_sid: Option<&str>,
) -> Result<AccountProfile> {
    let user_sid = validate_target(token.as_raw_handle() as _, account, expected_sid)?;
    let mut username = to_wide(account.username());
    let mut profile: PROFILEINFOW = unsafe { std::mem::zeroed() };
    profile.dwSize = std::mem::size_of::<PROFILEINFOW>() as u32;
    profile.dwFlags = 1; // PI_NOUI: service operations never prompt interactively.
    profile.lpUserName = username.as_mut_ptr();
    unsafe { BOOL(LoadUserProfileW(token.as_raw_handle() as _, &mut profile)).ok() }
        .context("load managed runtime account profile")?;
    Ok(AccountProfile {
        user_sid,
        token,
        profile: profile.hProfile,
    })
}

fn validate_target(
    token: HANDLE,
    account: SandboxRuntimeAccount,
    expected_sid: Option<&str>,
) -> Result<String> {
    let user = unsafe { codex_windows_sandbox::get_user_sid_bytes(token) }?;
    let user_sid = string_from_sid_bytes(&user).map_err(anyhow::Error::msg)?;
    ensure!(
        expected_sid.is_none_or(|expected| expected == user_sid),
        "managed runtime account SID changed"
    );
    ensure!(
        lookup_sid(account.username())? == user,
        "managed runtime account was replaced"
    );
    let groups = unsafe {
        token_groups(token, /*max_bytes*/ 65_536)
    }
    .context("read runtime account groups")?;
    ensure!(groups.len() <= 256, "invalid runtime account groups");
    let sandbox_group = lookup_sid(codex_windows_sandbox::SANDBOX_USERS_GROUP)?;
    let mut sandbox_member = false;
    for group in groups {
        ensure!(
            string_from_sid_bytes(&group.sid).map_err(anyhow::Error::msg)? != "S-1-5-32-544",
            "managed runtime account is an administrator"
        );
        if unsafe { security::EqualSid(group.sid.as_ptr() as _, sandbox_group.as_ptr() as _) } != 0
            && group.attributes & SE_GROUP_ENABLED as u32 != 0
        {
            sandbox_member = true;
        }
    }
    ensure!(
        sandbox_member,
        "runtime account is not in CodexSandboxUsers"
    );
    Ok(user_sid)
}
