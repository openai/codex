#[cfg(windows)]
use std::path::Component;
use std::path::Path;
#[cfg(windows)]
use std::path::Prefix;

#[cfg(windows)]
const WINDOWS_EXECUTABLE_SUFFIXES: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];

pub(crate) fn executable_lookup_key(raw: &str) -> String {
    #[cfg(windows)]
    {
        executable_lookup_key_windows(raw)
    }

    #[cfg(not(windows))]
    {
        raw.to_string()
    }
}

pub(crate) fn executable_path_lookup_key(path: &Path) -> Option<String> {
    let raw = path.file_name()?.to_str()?;

    #[cfg(windows)]
    {
        if has_windows_verbatim_or_device_prefix(path) {
            Some(executable_literal_lookup_key_windows(raw))
        } else {
            Some(executable_lookup_key_windows(raw))
        }
    }

    #[cfg(not(windows))]
    {
        Some(executable_lookup_key(raw))
    }
}

#[cfg(windows)]
fn executable_lookup_key_windows(raw: &str) -> String {
    executable_literal_lookup_key_windows(raw.trim_end_matches([' ', '.']))
}

#[cfg(windows)]
fn executable_literal_lookup_key_windows(raw: &str) -> String {
    let raw = raw.to_ascii_lowercase();
    for suffix in WINDOWS_EXECUTABLE_SUFFIXES {
        if let Some(raw) = raw.strip_suffix(suffix) {
            return raw.to_string();
        }
    }
    raw
}

#[cfg(windows)]
pub(crate) fn has_windows_verbatim_or_device_prefix(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                Prefix::Verbatim(_)
                    | Prefix::VerbatimUNC(_, _)
                    | Prefix::VerbatimDisk(_)
                    | Prefix::DeviceNS(_)
            )
    )
}

#[cfg(all(test, windows))]
#[path = "executable_name_tests.rs"]
mod tests;
