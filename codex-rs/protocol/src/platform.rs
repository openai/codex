use std::path::PathBuf;

/// Return true when running under WSL (heuristics: WSL_DISTRO_NAME or /proc/version contains "microsoft").
pub fn is_running_under_wsl() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return true;
    }
    if let Ok(s) = std::fs::read_to_string("/proc/version") {
        return s.to_ascii_lowercase().contains("microsoft");
    }
    false
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
    let rest = chars.as_str().trim_start_matches(|c| c == '\\' || c == '/');
    let drive_lower = drive.to_ascii_lowercase();
    let mapped = format!("/mnt/{}/{}", drive_lower, rest.replace('\\', "/"));
    Some(PathBuf::from(mapped))
}
