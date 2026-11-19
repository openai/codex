use std::path::Path;

#[cfg(windows)]
const WINDOWS_EXECUTABLE_SUFFIXES: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];

/// Return the key shared by host executable declarations and command lookups.
/// Windows folds ASCII case and ignores a recognized executable suffix.
pub fn executable_lookup_key(raw: &str) -> String {
    #[cfg(windows)]
    {
        let raw = raw.to_ascii_lowercase();
        for suffix in WINDOWS_EXECUTABLE_SUFFIXES {
            if raw.ends_with(suffix) {
                let stripped_len = raw.len() - suffix.len();
                return raw[..stripped_len].to_string();
            }
        }
        raw
    }

    #[cfg(not(windows))]
    {
        raw.to_string()
    }
}

/// Derive a lookup key from the filename when it is valid UTF-8.
pub fn executable_path_lookup_key(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(executable_lookup_key)
}
