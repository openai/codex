//! Removes one authenticated owner's sandbox resources during package uninstall.
//! Desktop paths are deleted only while impersonating the owner; native cleanup keeps its order.

use std::io;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::clean_up_packaged_windows_sandbox;
use codex_windows_sandbox::resolve_sid;
use codex_windows_sandbox::revoke_ace;

use super::UserInstallation;
use super::with_owner_impersonation;

pub(super) fn clean_up(installation: &mut UserInstallation) -> Result<()> {
    crate::service::log_information(
        crate::service::EVENT_CLEANUP_STARTED,
        "sandbox uninstall cleanup started",
    );
    // A partial uninstall must not let a later install inherit stale directory ownership.
    crate::installation_record::remove()?;
    let codex_home = installation.codex_home.clone();
    clean_up_packaged_windows_sandbox(codex_home.as_deref(), || {
        // Privileged file cleanup is finished; release the guard before owner-scoped removal.
        installation.directory_guard.take();
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
                if desktop.created_codex_home {
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
    })
    .context("remove packaged Windows sandbox resources")?;
    crate::service::log_information(
        crate::service::EVENT_CLEANUP_FINISHED,
        "sandbox uninstall cleanup finished",
    );
    Ok(())
}
