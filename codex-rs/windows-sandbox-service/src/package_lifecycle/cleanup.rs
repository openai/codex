//! Removes one authenticated owner's sandbox resources using prepared native cleanup.
//! Owner impersonation, directory pins, and registration-aware cleanup order are preserved.

use std::io;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::PreparedWindowsSandboxCleanup;
use codex_windows_sandbox::resolve_sid;
use codex_windows_sandbox::revoke_ace;

use super::UserInstallation;
use super::with_owner_impersonation;
use crate::installation_record::InstallationRecord;

pub(super) fn clean_up(
    installation: &mut UserInstallation,
    prepared: &PreparedWindowsSandboxCleanup,
    runtime: Option<&InstallationRecord>,
) -> Result<()> {
    crate::service::log_information(
        crate::service::EVENT_CLEANUP_STARTED,
        "sandbox uninstall cleanup started",
    );
    let codex_home = installation.codex_home.clone();
    // Remove exact grants from the locked cleanup record before native account deletion.
    if let Some(record) = runtime {
        crate::registered_runtime::remove_metadata(installation.user_token.0, record)?;
    }
    let result = prepared.finish(codex_home.as_deref(), || {
        // Release once, even when registered cleanup retries its remaining steps.
        installation.directory_guard.take();
        if let Some(record) = runtime
            && super::registered::runtime_owner_has_package(record)?
        {
            // Only the old sandbox resources are repaired on reinstall. Never remove
            // the reinstalled app's desktop-created home or runtime cache.
            return Ok(());
        }
        let Some(desktop) = &installation.record.desktop_installation else {
            return Ok(());
        };
        // The marker is user-writable. It must never authorize deletion as LocalSystem.
        with_owner_impersonation(installation.user_token.0, || {
            let mut errors = Vec::new();
            let mut record_error = |result: io::Result<()>| {
                if let Err(error) = result
                    && error.kind() != io::ErrorKind::NotFound
                {
                    errors.push(error.to_string());
                }
            };
            if let Some(home) = &codex_home {
                if desktop.created_codex_home && runtime.is_none() {
                    // Release the home itself so it can be deleted; keep its ancestors pinned.
                    installation.directory_handles.pop();
                    record_error(std::fs::remove_dir_all(home));
                } else {
                    // Preserve CLI data without leaving inherited permissions for the deleted group.
                    record_error(
                        resolve_sid("CodexSandboxUsers")
                            .and_then(|mut sid| unsafe {
                                revoke_ace(home, sid.as_mut_ptr().cast())
                            })
                            .map_err(io::Error::other),
                    );
                }
            }
            // The cache may have been created after provisioning. Pin it only for cleanup.
            let mut cache_directory_handles = Vec::new();
            if desktop.cache_home.is_dir() {
                match crate::ipc::pin_existing_ancestors(
                    &desktop.cache_home,
                    &mut cache_directory_handles,
                ) {
                    Ok(()) => record_error(std::fs::remove_dir_all(
                        desktop.cache_home.join("codex-runtimes"),
                    )),
                    Err(error) => errors.push(error.to_string()),
                }
            }
            ensure!(
                errors.is_empty(),
                "remove desktop directories: {}",
                errors.join("; ")
            );
            Ok(())
        })
    });
    result.context("remove packaged Windows sandbox resources")?;
    crate::service::log_information(
        crate::service::EVENT_CLEANUP_FINISHED,
        "sandbox uninstall cleanup finished",
    );
    Ok(())
}
