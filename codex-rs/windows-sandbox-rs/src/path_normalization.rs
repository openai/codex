use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::DRIVE_REMOTE;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

#[cfg(target_os = "windows")]
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

pub fn ensure_sandbox_local_path(path: &Path, context: &str) -> Result<()> {
    if let Some(kind) = describe_remote_windows_path(path) {
        anyhow::bail!(
            "Windows sandbox does not support {kind} for {context}: {}. Use a local checkout or disable sandbox/full-access for this workspace.",
            path.display()
        );
    }
    Ok(())
}

pub fn ensure_sandbox_local_paths<'a>(
    paths: impl IntoIterator<Item = &'a PathBuf>,
    context: &str,
) -> Result<()> {
    for path in paths {
        ensure_sandbox_local_path(path, context)?;
    }
    Ok(())
}

fn describe_remote_windows_path(path: &Path) -> Option<&'static str> {
    let path_str = path.as_os_str().to_string_lossy();
    if path_str.starts_with(r"\\") || path_str.starts_with("//") {
        return Some("UNC network-share paths");
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(root) = drive_root(path) {
            let root_wide = to_wide(root);
            if unsafe { GetDriveTypeW(root_wide.as_ptr()) } == DRIVE_REMOTE {
                return Some("mapped network-drive paths");
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn drive_root(path: &Path) -> Option<String> {
    use std::path::Component;
    use std::path::Prefix;

    match path.components().next()? {
        Component::Prefix(prefix)
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) =>
        {
            Some(format!(r"{}\", prefix.as_os_str().to_string_lossy()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    use super::ensure_sandbox_local_path;
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
    fn unc_paths_are_rejected_for_sandbox_use() {
        let err =
            ensure_sandbox_local_path(Path::new(r"\\server\share\workspace"), "sandbox workspace")
                .expect_err("UNC path should be rejected");

        assert!(
            err.to_string()
                .contains("Windows sandbox does not support UNC network-share paths")
        );
    }

    #[test]
    fn local_paths_are_allowed_for_sandbox_use() {
        ensure_sandbox_local_path(Path::new(r"C:\Users\Dev\Repo"), "sandbox workspace")
            .expect("local drive path should be allowed");
    }
}
