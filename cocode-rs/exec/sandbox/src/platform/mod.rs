//! Platform-specific sandbox implementations.
//!
//! Provides a `SandboxPlatform` trait with platform-gated implementations
//! for Unix (macOS/Linux) and Windows.

use crate::config::SandboxConfig;
use crate::error::Result;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

/// Platform-specific sandbox enforcement.
///
/// Implementations of this trait apply OS-level restrictions
/// (e.g., seccomp, Seatbelt, Windows Job Objects) based on the
/// provided sandbox configuration.
pub trait SandboxPlatform: Send + Sync {
    /// Returns true if this sandbox implementation is available on the current OS.
    fn available(&self) -> bool;

    /// Applies the sandbox configuration to the current process or child processes.
    fn apply(&self, config: &SandboxConfig) -> Result<()>;
}

/// Returns the platform-appropriate sandbox implementation.
#[cfg(unix)]
pub fn platform_sandbox() -> unix::UnixSandbox {
    unix::UnixSandbox
}

/// Returns the platform-appropriate sandbox implementation.
#[cfg(windows)]
pub fn platform_sandbox() -> windows::WindowsSandbox {
    windows::WindowsSandbox
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_sandbox_available() {
        let sandbox = platform_sandbox();
        // On any supported platform, this should return true
        assert!(sandbox.available());
    }

    #[test]
    fn test_platform_sandbox_apply_none_mode() {
        let sandbox = platform_sandbox();
        let config = SandboxConfig::default();
        // Applying a no-op sandbox should succeed
        assert!(sandbox.apply(&config).is_ok());
    }
}
