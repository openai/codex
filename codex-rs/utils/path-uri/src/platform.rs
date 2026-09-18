//! Platform identity and its path convention, independent of the resolving host.

use crate::PathConvention;

/// Operating system whose paths and execution configuration are being resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Macos,
    Windows,
    /// Missing or unrecognized platform metadata carries no path convention.
    Unknown,
}

impl Platform {
    /// Read platform metadata without substituting the current host's platform.
    pub fn from_platform_os(platform_os: Option<&str>) -> Self {
        match platform_os {
            Some("linux") => Self::Linux,
            Some("macos") => Self::Macos,
            Some("windows") => Self::Windows,
            Some(_) | None => Self::Unknown,
        }
    }

    /// Return the platform of the current process.
    pub const fn native() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Unknown
        }
    }

    /// Derive path grammar from platform identity while preserving unknown metadata.
    pub const fn path_convention(self) -> Option<PathConvention> {
        match self {
            Self::Linux | Self::Macos => Some(PathConvention::Posix),
            Self::Windows => Some(PathConvention::Windows),
            Self::Unknown => None,
        }
    }
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod tests;
