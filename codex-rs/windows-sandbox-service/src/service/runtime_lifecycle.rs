//! Runs owner-aware cleanup in the live provisioning broker before further admission.
//! Shutdown means SCM stop until cleanup finishes; retirement fences are never replayed.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::System::Services::SERVICE_RUNNING;
use windows_sys::Win32::System::Services::SERVICE_STOP_PENDING;

use super::EVENT_SERVICE_FAILED;
use super::EVENT_SERVICE_STARTED;
use super::SERVICE_STATE;
use super::ServiceState;
use super::log_error;
use super::log_information;
use crate::package_lifecycle::PackageLifecycle;

/// Foreground debugging has no persistent watcher, but must retain the same admission rules.
#[cfg(debug_assertions)]
pub(super) fn foreground_owner(
    mut record: crate::installation_record::InstallationRecord,
    _token: crate::ipc::OwnedHandle,
    runtime: codex_windows_sandbox::SetupRuntime,
) -> Result<crate::installation_record::InstallationRecord> {
    let _lock = codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;
    if let Some(previous) = crate::installation_record::load()? {
        let same_owner =
            previous.user_sid == record.user_sid && previous.codex_home == record.codex_home;
        if previous.runtime.is_some() {
            let family = windows::ApplicationModel::Package::Current()?
                .Id()?
                .FamilyName()?;
            previous.admit_owner(&record, &family.to_string())?;
            record.runtime = previous.runtime;
        } else if runtime == codex_windows_sandbox::SetupRuntime::Registered {
            anyhow::ensure!(
                same_owner,
                "shared sandbox resources belong to a different owner"
            );
        }
        if same_owner {
            record.desktop_installation = previous
                .desktop_installation
                .or(record.desktop_installation);
        }
    }
    Ok(record)
}

pub(super) fn run(state: &ServiceState, package_lifecycle: &PackageLifecycle) -> Result<()> {
    let cleaned = Cell::new(false);
    let last_cleanup_error = Cell::new(None);
    let restore_owner = || -> Result<()> {
        crate::registered_runtime::restore_disabled_accounts()?;
        let restored = crate::installation_record::load().and_then(|record| {
            record.map_or(Ok(()), |record| {
                package_lifecycle.restore_logged_in_owner(record.session_id)
            })
        });
        if let Err(error) = restored {
            log_error(
                EVENT_SERVICE_FAILED,
                &format!("unable to restore package uninstall listener: {error:#}"),
            );
        }
        Ok(())
    };
    crate::ipc::run(
        Arc::clone(&state.shutdown),
        || {
            restore_owner()?;
            state.report_status(SERVICE_RUNNING, NO_ERROR)?;
            log_information(
                EVENT_SERVICE_STARTED,
                "The Codex sandbox service is running.",
            );
            Ok(())
        },
        |installation, user_token, _runtime| {
            package_lifecycle.register_authenticated_user(installation, user_token)
        },
        || loop {
            restore_owner()?;
            if !crate::package_lifecycle::runtime_owner_removed()? {
                last_cleanup_error.set(None);
                return Ok(());
            }
            let Err(error) = package_lifecycle.clean_up() else {
                cleaned.set(true);
                state.shutdown.store(true, Ordering::Release);
                return state.report_status(SERVICE_STOP_PENDING, NO_ERROR);
            };
            if crate::installation_record::load_runtime()?.is_none_or(|record| {
                record
                    .runtime
                    .is_some_and(|runtime| runtime.retiring.is_some())
            }) {
                return Err(error);
            }
            log_cleanup_error(&error, &last_cleanup_error);
            // Keep the pipe available during pre-fence backoff. A reinstall
            // resumes admission on the next owner check; SCM stops are never reset.
            if !wait_for_cleanup_retry(state) {
                return Err(error);
            }
        },
    )
    .context("run the sandbox provisioning broker")?;
    if !cleaned.get()
        && state.stop_requested.load(Ordering::Acquire)
        && (state.uninstalling.load(Ordering::Acquire)
            || crate::package_lifecycle::runtime_owner_removed()?)
    {
        package_lifecycle.clean_up()?;
    }
    Ok(())
}

/// Retries the current teardown step, never a phase recovered from stored intent.
pub(crate) fn retry_cleanup(mut operation: impl FnMut() -> Result<()>) -> Result<()> {
    let mut stop_failures = 0;
    let last_cleanup_error = Cell::new(None);
    loop {
        let Err(error) = operation() else {
            return Ok(());
        };
        let Some(state) = SERVICE_STATE.get() else {
            return Err(error);
        };
        log_cleanup_error(&error, &last_cleanup_error);
        if state.stop_requested.load(Ordering::Acquire) {
            // SCM stop initiates uninstall cleanup rather than cancelling it.
            // Four one-second retry waits fit within the 10-second SCM wait hint.
            stop_failures += 1;
            if stop_failures == 5 {
                return Err(error);
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        } else if !wait_for_cleanup_retry(state) && !state.stop_requested.load(Ordering::Acquire) {
            return Err(error);
        }
    }
}

fn log_cleanup_error(error: &anyhow::Error, previous: &Cell<Option<String>>) {
    let message = format!("sandbox cleanup deferred: {error:#}");
    // Retrying an unchanged failure must not flood the Windows event log.
    if previous.take().as_deref() != Some(message.as_str()) {
        log_error(EVENT_SERVICE_FAILED, &message);
    }
    previous.set(Some(message));
}

fn wait_for_cleanup_retry(state: &ServiceState) -> bool {
    for _ in 0..30 {
        if state.shutdown.load(Ordering::Acquire) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    !state.shutdown.load(Ordering::Acquire)
}

#[cfg(test)]
#[path = "runtime_lifecycle_tests.rs"]
mod tests;
