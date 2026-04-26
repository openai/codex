use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Storage::FileSystem::DRIVE_REMOTE;
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

use crate::winutil::to_wide;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub fn ensure_windows_sandbox_local_path(path: &Path, context: &str) -> Result<()> {
    let raw = path.to_string_lossy();
    let normalized = normalize_windows_device_path(&raw).unwrap_or_else(|| raw.into_owned());
    if normalized.starts_with(r"\\") {
        anyhow::bail!(
            "windows sandbox does not support {context} on UNC or mapped network paths: {}. Use a local drive workspace or run without the Windows sandbox for this session.",
            path.display()
        );
    }

    if let Some(root) = windows_drive_root(&normalized) {
        let drive_type = unsafe { GetDriveTypeW(to_wide(&root).as_ptr()) };
        if drive_type == DRIVE_REMOTE {
            anyhow::bail!(
                "windows sandbox does not support {context} on mapped network drives: {}. Use the underlying local drive path or run without the Windows sandbox for this session.",
                path.display()
            );
        }
    }

    Ok(())
}

fn windows_drive_root(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Some(format!("{}:\\", path[..1].to_ascii_uppercase()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    use super::windows_drive_root;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn canonical_path_key_normalizes_case_and_separators() {
        let windows_style = Path::new(r"C:\Users\Dev\Repo");
        let slash_style = Path::new("c:/users/dev/repo");

        assert_eq!(
            canonical_path_key(windows_style),
            canonical_path_key(slash_style)
        );
    }

    #[test]
    fn windows_drive_root_extracts_drive_prefix() {
        assert_eq!(windows_drive_root(r"l:\cs-web"), Some(r"L:\".to_string()));
        assert_eq!(windows_drive_root(r"C:/repo"), Some(r"C:\".to_string()));
        assert_eq!(windows_drive_root(r"\\video1\node\cs-web"), None);
    }
}
