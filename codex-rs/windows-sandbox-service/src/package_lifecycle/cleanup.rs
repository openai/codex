//! Removes one authenticated owner's sandbox resources using prepared native cleanup.
//! Owner impersonation, directory pins, and registration-aware cleanup order are preserved.

use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::PreparedWindowsSandboxCleanup;
use codex_windows_sandbox::resolve_sid;
use codex_windows_sandbox::revoke_ace;

use super::UserInstallation;
use super::with_owner_impersonation;
use crate::installation_record::InstallationRecord;
use crate::service::EVENT_CLEANUP_DETAIL;
use crate::service::log_error;
use crate::service::log_information;

pub(super) fn clean_up(
    installation: &mut UserInstallation,
    prepared: &PreparedWindowsSandboxCleanup,
    runtime: Option<&InstallationRecord>,
    uninstalling: &AtomicBool,
) -> Result<()> {
    crate::service::log_information(
        crate::service::EVENT_CLEANUP_STARTED,
        "sandbox uninstall cleanup started",
    );
    let codex_home = installation.codex_home.clone();
    // Remove exact grants from the locked cleanup record before native account deletion.
    if let Some(record) = runtime {
        log_cleanup("removing registered runtime metadata");
        crate::registered_runtime::remove_metadata(installation.user_token.0, record)?;
    }
    log_cleanup("removing native sandbox resources");
    let mut prune_codex_home = false;
    // Once owner-scoped deletion releases the home, retries must not traverse it as SYSTEM.
    let sandbox_home = codex_home
        .as_deref()
        .filter(|_| installation.directory_guard.is_some());
    let result = prepared.finish(sandbox_home, log_cleanup, || {
        if let Some(record) = runtime
            && !super::registered::owner_allows_cleanup(uninstalling, record)?
        {
            // Only the old sandbox resources are repaired on reinstall. Never remove
            // the reinstalled app's desktop-created home or runtime cache.
            log_cleanup("skipping desktop directories: owner reinstalled the app");
            return Ok(());
        }
        let Some(desktop) = &installation.record.desktop_installation else {
            log_cleanup("skipping desktop directories: no desktop installation record");
            return Ok(());
        };
        // The marker is user-writable. It must never authorize deletion as LocalSystem.
        with_owner_impersonation(installation.user_token.0, || {
            let mut errors = Vec::new();
            let mut record_result = |operation: &str, result: io::Result<()>| match result {
                Ok(()) => log_cleanup(&format!("{operation}: completed")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    log_cleanup(&format!("{operation}: skipped, not found"));
                }
                Err(error) => {
                    let message = format!("{operation}: failed, {error}");
                    log_error(EVENT_CLEANUP_DETAIL, &message);
                    errors.push(message);
                }
            };
            if let Some(home) = &codex_home {
                if desktop.created_codex_home {
                    // Release the home itself so it can be deleted; keep its ancestors pinned.
                    if installation.directory_guard.take().is_some() {
                        installation.directory_handles.pop();
                    }
                    record_result(
                        "remove desktop-created codex home",
                        std::fs::remove_dir_all(home),
                    );
                } else {
                    prune_codex_home = true;
                    log_cleanup("skipping recursive codex home removal: existing CLI home");
                    // Preserve CLI data without leaving inherited permissions for the deleted group.
                    record_result(
                        "remove codex home sandbox permissions",
                        resolve_sid("CodexSandboxUsers")
                            .and_then(|mut sid| unsafe {
                                revoke_ace(home, sid.as_mut_ptr().cast())
                            })
                            .map_err(io::Error::other),
                    );
                }
            } else {
                log_cleanup("skipping codex home: no pinned home");
            }
            // The cache may have been created after provisioning. Pin it only for cleanup.
            let mut cache_directory_handles = Vec::new();
            if desktop.cache_home.is_dir() {
                match crate::ipc::pin_existing_ancestors(
                    &desktop.cache_home,
                    &mut cache_directory_handles,
                ) {
                    Ok(()) => {
                        record_result(
                            "remove codex runtime cache",
                            std::fs::remove_dir_all(desktop.cache_home.join("codex-runtimes")),
                        );
                        // Release only the cache root; its ancestors must remain pinned.
                        cache_directory_handles.pop();
                        remove_empty_directory(&desktop.cache_home, "cache home");
                    }
                    Err(error) => {
                        log_error(
                            EVENT_CLEANUP_DETAIL,
                            &format!("skipping runtime cache: could not pin cache home, {error:#}"),
                        );
                        errors.push(error.to_string());
                    }
                }
            } else {
                log_cleanup("skipping cache home: no accessible directory");
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
    if prune_codex_home && let Some(home) = &codex_home {
        // Keep the home pinned through native retries. Empty-root pruning is best effort
        // so a failure cannot restart native cleanup through an unpinned home.
        if let Err(error) = with_owner_impersonation(installation.user_token.0, || {
            if installation.directory_guard.take().is_some() {
                installation.directory_handles.pop();
            }
            remove_empty_directory(home, "codex home");
            Ok(())
        }) {
            log_error(
                EVENT_CLEANUP_DETAIL,
                &format!("remove empty codex home: failed, {error:#}"),
            );
        }
    }
    crate::service::log_information(
        if runtime.is_some() {
            EVENT_CLEANUP_DETAIL
        } else {
            crate::service::EVENT_CLEANUP_FINISHED
        },
        if runtime.is_some() {
            "native sandbox cleanup finished; registered runtime cleanup pending"
        } else {
            "sandbox uninstall cleanup finished"
        },
    );
    Ok(())
}

fn remove_empty_directory(path: &Path, target: &str) {
    match std::fs::remove_dir(path) {
        Ok(()) => log_cleanup(&format!("removed empty {target}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log_cleanup(&format!("skipping empty {target} removal: not found"));
        }
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
            log_cleanup(&format!("preserving {target}: directory is not empty"));
        }
        Err(error) => {
            log_error(
                EVENT_CLEANUP_DETAIL,
                &format!("remove empty {target}: failed, {error}"),
            );
        }
    }
}

fn log_cleanup(message: &str) {
    log_information(EVENT_CLEANUP_DETAIL, message);
}
