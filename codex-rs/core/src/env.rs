//! Functions for environment detection that need to be shared across crates.

fn env_var_set(key: &str) -> bool {
    std::env::var(key).is_ok_and(|v| !v.trim().is_empty())
}

/// Returns true if the current process is running under Windows Subsystem for Linux.
pub fn is_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WSL_DISTRO_NAME").is_some() {
            return true;
        }
        match std::fs::read_to_string("/proc/version") {
            Ok(version) => version.to_lowercase().contains("microsoft"),
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Returns true when Codex is likely running in an environment without a usable GUI.
///
/// This is intentionally conservative and is used by frontends to avoid flows that would try to
/// open a browser (e.g. device-code auth fallback).
pub fn is_headless_environment() -> bool {
    if env_var_set("CI")
        || env_var_set("SSH_CONNECTION")
        || env_var_set("SSH_CLIENT")
        || env_var_set("SSH_TTY")
    {
        return true;
    }

    #[cfg(target_os = "linux")]
    {
        if !env_var_set("DISPLAY") && !env_var_set("WAYLAND_DISPLAY") {
            return true;
        }
    }

    false
}
