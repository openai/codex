use std::path::PathBuf;
use std::sync::OnceLock;

/// Return true when running under WSL (heuristics: WSL_DISTRO_NAME or /proc/version contains "microsoft").
pub fn is_running_under_wsl() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        // Strong signals: env vars set by WSL when interop is available
        let has_wsl_env = std::env::var_os("WSL_INTEROP").is_some()
            || std::env::var_os("WSL_DISTRO_NAME").is_some();

        // Kernel hint: often contains "microsoft" under WSL1/2
        let kernel_mentions_ms = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .or_else(|_| std::fs::read_to_string("/proc/version"))
            .map(|s| s.to_ascii_lowercase().contains("microsoft"))
            .unwrap_or(false);

        // Be conservative: require both a WSL env var AND a microsoft kernel hint
        has_wsl_env && kernel_mentions_ms
    })
}

/// Map a Windows drive-letter path (e.g. `C:\Users\Alice\file.png`) to a
/// WSL path (`/mnt/c/Users/Alice/file.png`). Returns `None` if the input
/// doesn't look like a drive-letter path.
pub fn try_map_windows_drive_to_wsl_path(win_path: &str) -> Option<PathBuf> {
    let s = win_path.trim();
    let mut chars = s.chars();
    let drive = chars.next()?;
    let colon = chars.next()?;
    if !drive.is_ascii_alphabetic() || colon != ':' {
        return None;
    }
    let rest = chars.as_str().trim_start_matches(['\\', '/']);
    let drive_lower = drive.to_ascii_lowercase();
    let mapped = format!("/mnt/{}/{}", drive_lower, rest.replace('\\', "/"));
    Some(PathBuf::from(mapped))
}
