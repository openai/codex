//! Unix (macOS/Linux) sandbox implementation.

use tracing::info;

use crate::config::SandboxConfig;
use crate::error::Result;
use crate::platform::SandboxPlatform;

/// Unix sandbox implementation.
///
/// Currently a stub that logs the sandbox configuration.
/// Future implementations may use platform-specific mechanisms:
/// - macOS: Seatbelt (sandbox-exec)
/// - Linux: seccomp, landlock, or namespaces
pub struct UnixSandbox;

impl SandboxPlatform for UnixSandbox {
    fn available(&self) -> bool {
        cfg!(target_os = "macos") || cfg!(target_os = "linux")
    }

    fn apply(&self, config: &SandboxConfig) -> Result<()> {
        info!(
            mode = ?config.mode,
            allowed_paths = config.allowed_paths.len(),
            denied_paths = config.denied_paths.len(),
            allow_network = config.allow_network,
            "Unix sandbox applied (stub)"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_sandbox_available() {
        let sandbox = UnixSandbox;
        let expected = cfg!(target_os = "macos") || cfg!(target_os = "linux");
        assert_eq!(sandbox.available(), expected);
    }

    #[test]
    fn test_unix_sandbox_apply() {
        let sandbox = UnixSandbox;
        let config = SandboxConfig::default();
        assert!(sandbox.apply(&config).is_ok());
    }
}
