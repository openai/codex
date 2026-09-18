//! Executor-specific absolute socket path validation, independent of the controller OS.
//! Socket support and native path normalization remain executor runtime concerns.

use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::Platform;

pub(crate) fn socket_path_is_absolute(platform: Platform, path: &str) -> bool {
    // Core also accepts Unix-style absolute paths on Windows, for portability.
    path.starts_with('/')
        || match platform.path_convention() {
            Some(PathConvention::Posix) => false,
            // Legacy executors without OS metadata validate against their own
            // platform at launch; accept either absolute syntax here.
            Some(PathConvention::Windows) | None => windows_path_is_absolute(path),
        }
}

// Match std::path's Windows absolute-path syntax without consulting the host.
// Non-drive prefixes have an implicit root, including device and verbatim paths.
fn windows_path_is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    if matches!(bytes, [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic()) {
        return true;
    }
    let [b'\\' | b'/', b'\\' | b'/', rest @ ..] = bytes else {
        return false;
    };
    if path.starts_with(r"\\?\") || matches!(rest, [b'.', b'\\' | b'/', ..]) {
        return true;
    }
    let mut components = rest.split(|byte| matches!(byte, b'\\' | b'/'));
    matches!(
        (components.next(), components.next()),
        (Some(server), Some(share)) if !server.is_empty() && !share.is_empty()
    )
}
