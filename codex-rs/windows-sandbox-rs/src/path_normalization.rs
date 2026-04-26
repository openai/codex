use std::path::Path;
use std::path::PathBuf;

use crate::winutil::to_wide;
use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub fn normalize_command_cwd(path: &Path) -> PathBuf {
    let simplified = dunce::simplified(path).to_path_buf();
    normalize_mapped_drive_path_with(&simplified, mapped_drive_remote_root).unwrap_or(simplified)
}

fn normalize_mapped_drive_path_with<F>(path: &Path, resolve_remote_root: F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    let raw = path.to_string_lossy();
    let bytes = raw.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return None;
    }

    let drive = raw[..2].to_ascii_uppercase();
    let remote_root = resolve_remote_root(&drive)?;
    let suffix = raw[2..].trim_start_matches(['\\', '/']);
    let mut normalized = PathBuf::from(remote_root);
    if !suffix.is_empty() {
        normalized.push(suffix);
    }
    Some(normalized)
}

fn mapped_drive_remote_root(drive: &str) -> Option<String> {
    let drive_wide = to_wide(drive);
    let mut len = 260u32;

    loop {
        let mut buf = vec![0u16; len as usize];
        let status = unsafe { WNetGetConnectionW(drive_wide.as_ptr(), buf.as_mut_ptr(), &mut len) };
        match status {
            NO_ERROR => {
                let end = buf.iter().position(|ch| *ch == 0).unwrap_or(buf.len());
                return String::from_utf16(&buf[..end]).ok();
            }
            ERROR_MORE_DATA => continue,
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    use super::normalize_command_cwd;
    use super::normalize_mapped_drive_path_with;
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
    fn mapped_drive_paths_expand_to_unc_roots() {
        let path = Path::new(r"L:\cs-web\context");
        let normalized = normalize_mapped_drive_path_with(path, |drive| {
            (drive == "L:").then(|| r"\\video1\node".to_string())
        });
        assert_eq!(
            normalized,
            Some(PathBuf::from(r"\\video1\node\cs-web\context"))
        );
    }

    #[test]
    fn local_paths_are_left_alone() {
        let path = Path::new(r"C:\Users\Dev\Repo");
        assert_eq!(
            normalize_command_cwd(path),
            PathBuf::from(r"C:\Users\Dev\Repo")
        );
    }
}
