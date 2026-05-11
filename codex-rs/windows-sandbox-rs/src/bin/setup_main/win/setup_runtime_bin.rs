use std::ffi::c_void;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use codex_windows_sandbox::ensure_allow_mask_aces_with_inheritance;
use codex_windows_sandbox::path_mask_allows;
use windows_sys::Win32::Security::CONTAINER_INHERIT_ACE;
use windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;

pub(super) fn ensure_codex_app_runtime_bin_readable(
    sandbox_group_psid: *mut c_void,
    refresh_errors: &mut Vec<String>,
    log: &mut File,
) -> Result<()> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|profile| profile.join("AppData").join("Local"))
        });
    let Some(local_app_data) = local_app_data else {
        return Ok(());
    };

    // Codex desktop copies bundled Windows binaries out of WindowsApps to this
    // fixed LocalAppData cache before launching codex.exe.
    let runtime_bin_dir = local_app_data.join("OpenAI").join("Codex").join("bin");
    if !runtime_bin_dir.is_dir() {
        return Ok(());
    }

    let read_execute_mask = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
    let has_access = match path_mask_allows(
        &runtime_bin_dir,
        &[sandbox_group_psid],
        read_execute_mask,
        /*require_all_bits*/ true,
    ) {
        Ok(has_access) => has_access,
        Err(err) => {
            refresh_errors.push(format!(
                "runtime bin read/execute mask check failed on {} for sandbox_group: {err}",
                runtime_bin_dir.display()
            ));
            super::log_line(
                log,
                &format!(
                    "runtime bin read/execute mask check failed on {} for sandbox_group: {err}; continuing",
                    runtime_bin_dir.display()
                ),
            )?;
            false
        }
    };
    if has_access {
        return Ok(());
    }

    super::log_line(
        log,
        &format!(
            "granting read/execute ACE to {} for sandbox users",
            runtime_bin_dir.display()
        ),
    )?;
    let result = unsafe {
        ensure_allow_mask_aces_with_inheritance(
            &runtime_bin_dir,
            &[sandbox_group_psid],
            read_execute_mask,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
        )
    };
    if let Err(err) = result {
        refresh_errors.push(format!(
            "grant read/execute ACE failed on {} for sandbox_group: {err}",
            runtime_bin_dir.display()
        ));
        super::log_line(
            log,
            &format!(
                "grant read/execute ACE failed on {} for sandbox_group: {err}",
                runtime_bin_dir.display()
            ),
        )?;
    }
    Ok(())
}
