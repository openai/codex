use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::DRIVE_REMOTE;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
pub fn unsupported_workspace_root_reason(path: &Path) -> Option<String> {
    let display = path.display().to_string();
    let raw = path.as_os_str().to_string_lossy();
    if is_unc_like_path_str(&raw) {
        return Some(format!("{display} uses a UNC/network path"));
    }

    let drive_root = drive_root_from_path_str(&raw)?;
    let drive_root_wide = crate::winutil::to_wide(&drive_root);
    let drive_type = unsafe { GetDriveTypeW(drive_root_wide.as_ptr()) };
    (drive_type == DRIVE_REMOTE).then(|| {
        format!(
            "{display} is on mapped drive {}",
            drive_root.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn is_unc_like_path_str(path: &str) -> bool {
    if path.starts_with(r"\\?\UNC\")
        || path.starts_with(r"\\.\UNC\")
    {
        return true;
    }
    if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
        return false;
    }
    path.starts_with(r"\\") || path.starts_with("//")
}

#[cfg(target_os = "windows")]
fn drive_root_from_path_str(path: &str) -> Option<PathBuf> {
    let trimmed = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\\.\"))
        .unwrap_or(path);
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' {
        return None;
    }
    Some(PathBuf::from(format!("{}:\\", trimmed.chars().next()?)))
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    #[cfg(target_os = "windows")]
    use super::drive_root_from_path_str;
    #[cfg(target_os = "windows")]
    use super::is_unc_like_path_str;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn canonical_path_key_normalizes_case_and_separators() {
        let windows_style = Path::new(r"C:\Users\Dev\Repo");
        let slash_style = Path::new("c:/users/dev/repo");

        assert_eq!(canonical_path_key(windows_style), canonical_path_key(slash_style));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn detects_unc_like_workspace_paths() {
        assert!(is_unc_like_path_str(r"\\server\share\repo"));
        assert!(is_unc_like_path_str(r"\\?\UNC\server\share\repo"));
        assert!(!is_unc_like_path_str(r"C:\repo"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn extracts_drive_root_from_verbatim_or_normal_drive_paths() {
        assert_eq!(
            drive_root_from_path_str(r"L:\repo").expect("drive root"),
            Path::new(r"L:\")
        );
        assert_eq!(
            drive_root_from_path_str(r"\\?\L:\repo").expect("verbatim drive root"),
            Path::new(r"L:\")
        );
    }
}
