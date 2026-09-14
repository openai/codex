//! Connects the provisioning broker to the existing package owner and uninstall listener.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::System::Services::SERVICE_RUNNING;

use super::EVENT_SERVICE_FAILED;
use super::EVENT_SERVICE_STARTED;
use super::ServiceState;
use super::log_error;
use super::log_information;
use crate::package_lifecycle::PackageLifecycle;

pub(super) fn run(state: &ServiceState, package_lifecycle: &PackageLifecycle) -> Result<()> {
    crate::ipc::run(
        Arc::clone(&state.shutdown),
        || {
            state.report_status(SERVICE_RUNNING, NO_ERROR)?;
            if let Some(record) = crate::installation_record::load()?
                && let Err(error) = package_lifecycle.restore_logged_in_owner(record.session_id)
            {
                log_error(
                    EVENT_SERVICE_FAILED,
                    &format!("unable to restore package uninstall listener: {error:#}"),
                );
            }
            log_information(
                EVENT_SERVICE_STARTED,
                "The Codex sandbox service is running.",
            );
            Ok(())
        },
        |installation, user_token| {
            package_lifecycle.register_authenticated_user(installation, user_token)
        },
        || {
            let session = state.changed_session.swap(u32::MAX, Ordering::AcqRel);
            if session == u32::MAX {
                return Ok(());
            }
            if let Err(error) = package_lifecycle.restore_authenticated_user(session) {
                log_error(
                    EVENT_SERVICE_FAILED,
                    &format!("unable to restore package uninstall listener: {error:#}"),
                );
            }
            Ok(())
        },
    )
    .context("run the sandbox provisioning broker")?;

    if state.stop_requested.load(Ordering::Acquire) && state.uninstalling.load(Ordering::Acquire) {
        package_lifecycle.clean_up()?;
    }
    Ok(())
}
