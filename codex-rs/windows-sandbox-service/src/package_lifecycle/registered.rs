//! Registered runtime ownership and uninstall policy for the shared package watcher.
//! Native cleanup stays inside the caller's setup lock and precedes package removal.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::prepare_packaged_windows_sandbox_cleanup_with_retained_tokens;
use windows::ApplicationModel::PackageUninstallingEventArgs;
use windows::Management::Deployment::PackageManager;
use windows::core::HSTRING;

use super::PackageLifecycle;
use crate::installation_record::InstallationRecord;

pub(super) fn on_package_uninstalling(
    event: &PackageUninstallingEventArgs,
    owner_sid: &str,
    uninstalling: &AtomicBool,
) -> windows::core::Result<()> {
    let retention = crate::installation_record::load_runtime().and_then(|record| match record {
        Some(record) if record.user_sid == owner_sid => {
            crate::registered_runtime::has_retaining_registration(&record).map(Some)
        }
        Some(_) => Ok(None),
        None => Ok(Some(false)),
    });
    match retention {
        Ok(Some(true)) => {
            // Removing an old version during update must not remove the
            // managed user's registration. Check the whole family only
            // after the owner's uninstall has completed.
            if event.IsComplete()? {
                crate::service::wake_listener();
            }
        }
        Ok(None) => {}
        Ok(Some(false)) => {
            // No actual managed registration keeps this service installed,
            // even if a failed attempt left intent. Retain SCM-stop cleanup.
            uninstalling.store(!event.IsComplete()?, Ordering::Release);
        }
        Err(error) => crate::service::log_error(
            crate::service::EVENT_SERVICE_FAILED,
            &format!("unable to read registered runtime ownership: {error:#}"),
        ),
    }
    Ok(())
}

/// The shared cleanup entry holds the setup mutex until this function returns.
pub(super) fn clean_up(lifecycle: &PackageLifecycle, record: InstallationRecord) -> Result<()> {
    if !crate::installation_record::is_current_package_family(&record)? {
        return Ok(());
    }
    ensure!(
        record.runtime()?.retiring.is_none(),
        "interrupted runtime cleanup requires repair"
    );
    ensure!(
        !runtime_owner_has_package(&record)?
            || (lifecycle.uninstalling.load(Ordering::Acquire)
                && !crate::registered_runtime::has_retaining_registration(&record)?),
        "registered runtime owner still has the app installed"
    );
    ensure!(
        lifecycle
            .installation
            .borrow()
            .as_ref()
            .is_some_and(|installation| {
                installation.record.user_sid == record.user_sid
                    && installation.codex_home.as_ref() == Some(&record.codex_home)
            }),
        "waiting for the authenticated runtime owner and home before sandbox file cleanup"
    );
    let record = crate::registered_runtime::prepare_cleanup(record)?;
    let removal = {
        let installation = lifecycle.installation.borrow();
        let installation = installation
            .as_ref()
            .context("authenticated installation is missing")?;
        crate::registered_runtime::prepare_removal(installation.user_token.0, &record)?
    };
    let prepared =
        prepare_packaged_windows_sandbox_cleanup_with_retained_tokens(&removal.tokens())?;
    // A reinstall before this boundary cancels without retiring resources.
    ensure!(
        (lifecycle.uninstalling.load(Ordering::Acquire)
            && !crate::registered_runtime::has_retaining_registration(&record)?)
            || !runtime_owner_has_package(&record)?,
        "owner reinstalled before native cleanup started"
    );
    crate::installation_record::save_runtime(&record)?;
    // Retry only in this live invocation, retaining the outer setup lock.
    // A stored retirement fence is never proof that native cleanup finished.
    crate::service::retry_cleanup(|| lifecycle.clean_up_resources(&prepared, Some(&record)))?;
    removal.commit()
}

/// The managed registration must not keep the package alive after its owner
/// uninstalls it, but an update to another version of the family is not uninstall.
pub(crate) fn runtime_owner_removed() -> Result<bool> {
    let Some(record) = crate::installation_record::load_runtime()? else {
        return Ok(false);
    };
    Ok(
        crate::installation_record::is_current_package_family(&record)?
            && !runtime_owner_has_package(&record)?,
    )
}

// Callers have verified that the record belongs to this service's package family.
pub(super) fn runtime_owner_has_package(record: &InstallationRecord) -> Result<bool> {
    let packages = PackageManager::new()?.FindPackagesByUserSecurityIdPackageFamilyName(
        &HSTRING::from(&record.user_sid),
        &HSTRING::from(&record.runtime()?.package_family),
    )?;
    Ok(packages.First()?.HasCurrent()?)
}
