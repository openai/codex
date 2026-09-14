//! Shares the protected installation record with mutating sandbox setup.
//! Callers must hold the sandbox setup mutex while admitting or changing ownership.

use std::io;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::System::Registry as registry;

/// Selects legacy helper materialization or verified app-contained Core for setup.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetupRuntime {
    #[default]
    Legacy,
    Registered,
}

impl SetupRuntime {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde requires a borrowed field"
    )]
    pub(crate) fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

/// Package-scoped execution alias declared for the app's sandbox runner.
pub const APP_CORE_RUNNER_ALIAS: &str = "codex-core-command-runner.exe";

/// The service may register its package only for these managed sandbox accounts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxRuntimeAccount {
    Offline,
    Online,
}

impl SandboxRuntimeAccount {
    pub fn username(self) -> &'static str {
        match self {
            Self::Offline => crate::setup::OFFLINE_USERNAME,
            Self::Online => crate::setup::ONLINE_USERNAME,
        }
    }
}

pub use crate::installation_record::INSTALLATION_KEY;
pub use crate::installation_record::INSTALLATION_VALUE;
use crate::installation_record::InstallationRecord;
use crate::winutil::to_wide;

// Old services overwrite the parent value and delete its key on uninstall.
// A child key preserves Core state and makes that old whole-key delete fail.
pub const CORE_INSTALLATION_KEY: &str =
    r"SOFTWARE\OpenAI\Codex\WindowsSandboxService\RegisteredCore";

/// Written before registration so a service restart cannot lose cleanup ownership.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeRegistration {
    pub package_family: String,
    pub accounts: Vec<RuntimeAccountRegistration>,
    /// Owner-authorized AppData roots, recorded before granting metadata access.
    /// Retain former roots through redirection and package updates until cleanup.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata_roots: Vec<std::path::PathBuf>,
    /// Committed only after both registrations and metadata grants succeed.
    #[serde(default)]
    pub ready_package: Option<String>,
    /// Fences setup during normal teardown; a restart must not replay it.
    /// Old non-null phase journals fail to decode rather than lose their fence.
    #[serde(default, alias = "cleanup")]
    pub retiring: Option<String>,
}

impl RuntimeRegistration {
    pub fn ready_for_package(&self, full_name: &str) -> bool {
        self.retiring.is_none()
            && self.ready_package.as_deref() == Some(full_name)
            && self.accounts.len() == 2
            && self.accounts[0].account != self.accounts[1].account
            && self
                .accounts
                .iter()
                .all(|account| account.alias_path.is_some())
    }
}

impl InstallationRecord {
    pub fn runtime(&self) -> Result<&RuntimeRegistration> {
        self.runtime
            .as_ref()
            .context("registered runtime ownership is missing")
    }

    pub fn runtime_mut(&mut self) -> Result<&mut RuntimeRegistration> {
        self.runtime
            .as_mut()
            .context("registered runtime ownership is missing")
    }

    /// Any persisted cleanup has committed to removing this resource generation.
    pub fn admit_owner(&self, owner: &InstallationRecord, family: &str) -> Result<()> {
        ensure!(
            self.user_sid == owner.user_sid
                && self.codex_home == owner.codex_home
                && self.runtime()?.package_family.eq_ignore_ascii_case(family),
            "registered sandbox resources belong to a different owner or package"
        );
        ensure!(
            self.runtime()?.retiring.is_none(),
            "registered sandbox cleanup must finish before provisioning"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAccountRegistration {
    pub account: SandboxRuntimeAccount,
    pub user_sid: String,
    /// OS-resolved alias written by the service, never inferred from a user name.
    #[serde(default)]
    pub alias_path: Option<std::path::PathBuf>,
}

/// Core owns its complete record; the old parent is only a fallback before/after Core.
pub fn load_installation() -> Result<Option<InstallationRecord>> {
    if let Some(record) = crate::installation_record::load_from(CORE_INSTALLATION_KEY)? {
        ensure!(
            record.runtime.is_some(),
            "Core installation record is missing runtime state"
        );
        return Ok(Some(record));
    }
    crate::installation_record::load_from(INSTALLATION_KEY)
}

/// Retires only legacy ownership while the caller holds the setup lock.
/// Registered Core ownership belongs to its post-service-exit finalizer.
pub fn remove_installation() -> Result<()> {
    let Some(record) = load_installation()? else {
        return Ok(());
    };
    ensure!(
        record.runtime.is_none(),
        "registered Core ownership must be retired after service exit"
    );
    let status = unsafe {
        registry::RegDeleteKeyValueW(
            registry::HKEY_LOCAL_MACHINE,
            to_wide(INSTALLATION_KEY).as_ptr(),
            to_wide(INSTALLATION_VALUE).as_ptr(),
        )
    };
    match status {
        foundation::ERROR_SUCCESS
        | foundation::ERROR_FILE_NOT_FOUND
        | foundation::ERROR_PATH_NOT_FOUND => flush_installation(INSTALLATION_KEY),
        status => Err(io::Error::from_raw_os_error(status as i32))
            .context("remove protected legacy sandbox installation record"),
    }
}

/// Persists and flushes ownership before an operation may outlive this service.
/// The caller must hold the sandbox setup mutex across its read/modify/write.
pub fn save_installation(record: &InstallationRecord) -> Result<()> {
    let key = if record.runtime.is_some() {
        CORE_INSTALLATION_KEY
    } else {
        INSTALLATION_KEY
    };
    crate::installation_record::save_to(key, record)?;
    flush_installation(key)
}

fn flush_installation(path: &str) -> Result<()> {
    let mut key = 0;
    let status = unsafe {
        registry::RegOpenKeyExW(
            registry::HKEY_LOCAL_MACHINE,
            to_wide(path).as_ptr(),
            0,
            registry::KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status != foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("open runtime ownership record");
    }
    let status = unsafe { registry::RegFlushKey(key) };
    unsafe { registry::RegCloseKey(key) };
    if status != foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("flush runtime ownership record");
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_ownership_tests.rs"]
mod tests;
