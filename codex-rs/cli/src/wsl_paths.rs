use std::ffi::OsStr;

/// Returns true if the current process is running under WSL.
pub use codex_core::env::is_wsl;

/// Convert a Windows absolute path (`C:\foo\bar` or `C:/foo/bar`) to a WSL mount path (`/mnt/c/foo/bar`).
/// Returns `None` if the input does not look like a Windows drive path.
pub fn win_path_to_wsl(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    if bytes.len() < 3
        || bytes[1] != b':'
        || !(bytes[2] == b'\\' || bytes[2] == b'/')
        || !bytes[0].is_ascii_alphabetic()
    {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let tail = path[3..].replace('\\', "/");
    if tail.is_empty() {
        return Some(format!("/mnt/{drive}"));
    }
    Some(format!("/mnt/{drive}/{tail}"))
}

/// If under WSL and given a Windows-style path, return the equivalent `/mnt/<drive>/…` path.
/// Otherwise returns the input unchanged.
pub fn normalize_for_wsl<P: AsRef<OsStr>>(path: P) -> String {
    let value = path.as_ref().to_string_lossy().to_string();
    if !is_wsl() {
        return value;
    }
    if let Some(mapped) = win_path_to_wsl(&value) {
        return mapped;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_to_wsl_basic() {
        assert_eq!(
            win_path_to_wsl(r"C:\Temp\codex.zip").as_deref(),
            Some("/mnt/c/Temp/codex.zip")
        );
        assert_eq!(
            win_path_to_wsl("D:/Work/codex.tgz").as_deref(),
            Some("/mnt/d/Work/codex.tgz")
        );
        assert!(win_path_to_wsl("/home/user/codex").is_none());
    }

    #[test]
    fn normalize_is_noop_on_unix_paths() {
        assert_eq!(normalize_for_wsl("/home/u/x"), "/home/u/x");
    }
}
