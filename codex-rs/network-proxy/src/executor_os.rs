//! Executor path semantics for policy validation, independent of the controller OS.
//! Socket support and native path normalization remain executor runtime concerns.

/// OS of the executor whose network policy is being validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkProxyExecutorOs {
    Linux,
    Macos,
    Windows,
    /// Legacy executors may omit their OS. Accept absolute syntax supported by
    /// either platform here; the executor validates against its own OS at launch.
    Unknown,
}

impl NetworkProxyExecutorOs {
    /// Uses executor metadata, never the OS of the process doing the validation.
    pub fn from_platform_os(platform_os: Option<&str>) -> Self {
        match platform_os {
            Some("linux") => Self::Linux,
            Some("macos") => Self::Macos,
            Some("windows") => Self::Windows,
            Some(_) | None => Self::Unknown,
        }
    }

    pub(crate) fn socket_path_is_absolute(self, path: &str) -> bool {
        // Core also accepts Unix-style absolute paths on Windows, for portability.
        path.starts_with('/')
            || match self {
                Self::Linux | Self::Macos => false,
                Self::Windows | Self::Unknown => windows_path_is_absolute(path),
            }
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
