use std::path::Path;
use std::path::PathBuf;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub fn unsupported_windows_sandbox_workspace_reason(path: &Path) -> Option<String> {
    unsupported_windows_sandbox_workspace_kind(path).map(|kind| {
        format!(
            "windows sandbox does not support {kind} workspace paths ({path}); use a local checkout or disable the sandbox for this session",
            path = path.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn unsupported_windows_sandbox_workspace_kind(path: &Path) -> Option<&'static str> {
    use windows_sys::Win32::Storage::FileSystem::DRIVE_REMOTE;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    let path_text = path.as_os_str().to_string_lossy();
    if path_text.starts_with(r"\\") || path_text.starts_with("//") {
        return Some("UNC network share");
    }

    let root = windows_drive_root(path)?;
    let root_wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let drive_type = unsafe { GetDriveTypeW(root_wide.as_ptr()) };
    (drive_type == DRIVE_REMOTE).then_some("mapped network drive")
}

#[cfg(not(target_os = "windows"))]
fn unsupported_windows_sandbox_workspace_kind(_path: &Path) -> Option<&'static str> {
    None
}

#[cfg(target_os = "windows")]
fn windows_drive_root(path: &Path) -> Option<String> {
    let path_text = path.as_os_str().to_string_lossy();
    let bytes = path_text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        Some(format!("{}:\\", bytes[0] as char))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::unsupported_windows_sandbox_workspace_reason;
    use super::canonical_path_key;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn canonical_path_key_normalizes_case_and_separators() {
        let windows_style = Path::new(r"C:\Users\Dev\Repo");
        let slash_style = Path::new("c:/users/dev/repo");

        assert_eq!(canonical_path_key(windows_style), canonical_path_key(slash_style));
    }

    #[test]
    fn unsupported_workspace_reason_flags_unc_paths() {
        let reason = unsupported_windows_sandbox_workspace_reason(Path::new(r"\\server\share\repo"))
            .expect("UNC path should be rejected");
        assert!(reason.contains("UNC network share"));
    }
}
