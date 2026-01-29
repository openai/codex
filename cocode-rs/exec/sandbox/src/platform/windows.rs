//! Windows sandbox implementation.

use tracing::info;

use crate::config::SandboxConfig;
use crate::error::Result;
use crate::platform::SandboxPlatform;

/// Windows sandbox implementation.
///
/// Currently a stub that logs the sandbox configuration.
/// Future implementations may use Windows Job Objects or
/// AppContainers for process isolation.
pub struct WindowsSandbox;

impl SandboxPlatform for WindowsSandbox {
    fn available(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn apply(&self, config: &SandboxConfig) -> Result<()> {
        info!(
            mode = ?config.mode,
            allowed_paths = config.allowed_paths.len(),
            denied_paths = config.denied_paths.len(),
            allow_network = config.allow_network,
            "Windows sandbox applied (stub)"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_sandbox_available() {
        let sandbox = WindowsSandbox;
        let expected = cfg!(target_os = "windows");
        assert_eq!(sandbox.available(), expected);
    }

    #[test]
    fn test_windows_sandbox_apply() {
        let sandbox = WindowsSandbox;
        let config = SandboxConfig::default();
        assert!(sandbox.apply(&config).is_ok());
    }
}
