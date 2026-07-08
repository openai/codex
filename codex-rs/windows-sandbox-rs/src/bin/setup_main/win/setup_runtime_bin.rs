use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use codex_windows_sandbox::ensure_allow_mask_aces_with_inheritance;
use codex_windows_sandbox::path_mask_allows;
use windows_sys::Win32::Security::CONTAINER_INHERIT_ACE;
use windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;

#[cfg(test)]
#[path = "setup_runtime_bin_tests.rs"]
mod tests;

pub(super) fn ensure_codex_app_runtime_paths_readable(
    sandbox_group_psid: *mut c_void,
    refresh_errors: &mut Vec<String>,
    log: &mut dyn Write,
) -> Result<()> {
    let read_execute_mask = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
    let runtime_paths = runtime_paths(
        local_app_data_root(),
        std::env::var_os("USERPROFILE").map(PathBuf::from),
    );

    for runtime_path in runtime_paths {
        if !runtime_path.is_dir() {
            continue;
        }

        let has_access = match path_mask_allows(
            &runtime_path,
            &[sandbox_group_psid],
            read_execute_mask,
            /*require_all_bits*/ true,
        ) {
            Ok(has_access) => has_access,
            Err(err) => {
                refresh_errors.push(format!(
                    "runtime read/execute mask check failed on {} for sandbox_group: {err}",
                    runtime_path.display()
                ));
                super::log_line(
                    log,
                    &format!(
                        "runtime read/execute mask check failed on {} for sandbox_group: {err}; continuing",
                        runtime_path.display()
                    ),
                )?;
                false
            }
        };
        if has_access {
            continue;
        }

        super::log_line(
            log,
            &format!(
                "granting read/execute ACE to {} for sandbox users",
                runtime_path.display()
            ),
        )?;
        let result = unsafe {
            ensure_allow_mask_aces_with_inheritance(
                &runtime_path,
                &[sandbox_group_psid],
                read_execute_mask,
                OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
            )
        };
        if let Err(err) = result {
            refresh_errors.push(format!(
                "grant read/execute ACE failed on {} for sandbox_group: {err}",
                runtime_path.display()
            ));
            super::log_line(
                log,
                &format!(
                    "grant read/execute ACE failed on {} for sandbox_group: {err}",
                    runtime_path.display()
                ),
            )?;
        }
    }
    Ok(())
}

fn runtime_paths(local_app_data: Option<PathBuf>, user_profile: Option<PathBuf>) -> Vec<PathBuf> {
    let mut runtime_paths = Vec::new();
    if let Some(local_app_data) = local_app_data {
        let codex_root = local_app_data.join("OpenAI").join("Codex");
        runtime_paths.extend([codex_root.join("bin"), codex_root.join("runtimes")]);
    }
    // The managed primary runtime is installed outside the LocalAppData runtime roots.
    if let Some(user_profile) = user_profile {
        runtime_paths.push(user_profile.join(".cache").join("codex-runtimes"));
    }

    runtime_paths
}

fn local_app_data_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|profile| profile.join("AppData").join("Local"))
        })
}
