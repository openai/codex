//! Permission checking for sandbox-enforced operations.

use std::path::Path;

use crate::config::SandboxConfig;
use crate::config::SandboxMode;
use crate::error::Result;
use crate::error::sandbox_error::*;
use crate::error::{self};

/// Checks permissions against the sandbox configuration.
#[derive(Debug, Clone)]
pub struct PermissionChecker {
    config: SandboxConfig,
}

impl PermissionChecker {
    /// Creates a new checker with the given configuration.
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the underlying configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Checks whether the given path is accessible.
    ///
    /// - In `None` mode, all paths are allowed.
    /// - In `ReadOnly` mode, read access is always allowed; write access is denied.
    /// - In `Strict` mode, the path must be explicitly in `allowed_paths` and not in
    ///   `denied_paths`. Write access requires the path to be allowed.
    pub fn check_path(&self, path: &Path, write: bool) -> Result<()> {
        match self.config.mode {
            SandboxMode::None => Ok(()),
            SandboxMode::ReadOnly => {
                if write {
                    return WriteDeniedSnafu {
                        message: format!(
                            "sandbox is in read-only mode, cannot write to: {}",
                            path.display()
                        ),
                    }
                    .fail();
                }
                Ok(())
            }
            SandboxMode::Strict => {
                // Check denied paths first (takes precedence)
                if self.is_denied_path(path) {
                    return PathDeniedSnafu {
                        path: path.display().to_string(),
                    }
                    .fail();
                }

                // In strict mode, the path must be explicitly allowed
                if !self.is_allowed_path(path) {
                    return PathDeniedSnafu {
                        path: path.display().to_string(),
                    }
                    .fail();
                }

                // Write access requires the path to be allowed (already checked above)
                if write && !self.config.mode_allows_write() {
                    return WriteDeniedSnafu {
                        message: format!("write denied in strict mode: {}", path.display()),
                    }
                    .fail();
                }

                Ok(())
            }
        }
    }

    /// Checks whether network access is allowed.
    pub fn check_network(&self) -> Result<()> {
        if self.config.mode == SandboxMode::None {
            return Ok(());
        }

        if !self.config.allow_network {
            return error::sandbox_error::NetworkDeniedSnafu.fail();
        }

        Ok(())
    }

    /// Returns true if the path is under one of the allowed paths.
    pub fn is_allowed_path(&self, path: &Path) -> bool {
        if self.config.allowed_paths.is_empty() {
            // If no allowed paths are configured, allow all (for None/ReadOnly modes)
            return self.config.mode != SandboxMode::Strict;
        }

        self.config
            .allowed_paths
            .iter()
            .any(|allowed| path.starts_with(allowed))
    }

    /// Returns true if the path is under one of the denied paths.
    fn is_denied_path(&self, path: &Path) -> bool {
        self.config
            .denied_paths
            .iter()
            .any(|denied| path.starts_with(denied))
    }
}

/// Extension trait for SandboxConfig to check write permissions.
trait SandboxConfigExt {
    fn mode_allows_write(&self) -> bool;
}

impl SandboxConfigExt for SandboxConfig {
    fn mode_allows_write(&self) -> bool {
        // In strict mode, writes are allowed to explicitly allowed paths
        // None mode allows all, ReadOnly denies all writes
        !matches!(self.mode, SandboxMode::ReadOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn strict_config() -> SandboxConfig {
        SandboxConfig {
            mode: SandboxMode::Strict,
            allowed_paths: vec![PathBuf::from("/home/user/project")],
            denied_paths: vec![PathBuf::from("/home/user/project/.env")],
            allow_network: false,
        }
    }

    fn readonly_config() -> SandboxConfig {
        SandboxConfig {
            mode: SandboxMode::ReadOnly,
            allowed_paths: vec![],
            denied_paths: vec![],
            allow_network: false,
        }
    }

    fn none_config() -> SandboxConfig {
        SandboxConfig::default()
    }

    #[test]
    fn test_none_mode_allows_everything() {
        let checker = PermissionChecker::new(none_config());
        assert!(checker.check_path(Path::new("/any/path"), false).is_ok());
        assert!(checker.check_path(Path::new("/any/path"), true).is_ok());
        assert!(checker.check_network().is_ok());
    }

    #[test]
    fn test_readonly_allows_reads() {
        let checker = PermissionChecker::new(readonly_config());
        assert!(checker.check_path(Path::new("/any/path"), false).is_ok());
    }

    #[test]
    fn test_readonly_denies_writes() {
        let checker = PermissionChecker::new(readonly_config());
        assert!(checker.check_path(Path::new("/any/path"), true).is_err());
    }

    #[test]
    fn test_readonly_denies_network() {
        let checker = PermissionChecker::new(readonly_config());
        assert!(checker.check_network().is_err());
    }

    #[test]
    fn test_strict_allows_allowed_path() {
        let checker = PermissionChecker::new(strict_config());
        assert!(
            checker
                .check_path(Path::new("/home/user/project/src/main.rs"), false)
                .is_ok()
        );
    }

    #[test]
    fn test_strict_denies_non_allowed_path() {
        let checker = PermissionChecker::new(strict_config());
        assert!(checker.check_path(Path::new("/etc/passwd"), false).is_err());
    }

    #[test]
    fn test_strict_denied_path_takes_precedence() {
        let checker = PermissionChecker::new(strict_config());
        // .env is under the allowed project path but explicitly denied
        assert!(
            checker
                .check_path(Path::new("/home/user/project/.env"), false)
                .is_err()
        );
    }

    #[test]
    fn test_strict_denies_network_by_default() {
        let checker = PermissionChecker::new(strict_config());
        assert!(checker.check_network().is_err());
    }

    #[test]
    fn test_strict_allows_network_when_configured() {
        let mut config = strict_config();
        config.allow_network = true;
        let checker = PermissionChecker::new(config);
        assert!(checker.check_network().is_ok());
    }

    #[test]
    fn test_is_allowed_path_empty_none_mode() {
        let checker = PermissionChecker::new(none_config());
        // No allowed_paths configured, but mode is None so everything is allowed
        assert!(checker.is_allowed_path(Path::new("/anything")));
    }

    #[test]
    fn test_is_allowed_path_empty_strict_mode() {
        let config = SandboxConfig {
            mode: SandboxMode::Strict,
            allowed_paths: vec![],
            denied_paths: vec![],
            allow_network: false,
        };
        let checker = PermissionChecker::new(config);
        // No allowed_paths in strict mode means nothing is allowed
        assert!(!checker.is_allowed_path(Path::new("/anything")));
    }

    #[test]
    fn test_is_allowed_path_prefix_match() {
        let checker = PermissionChecker::new(strict_config());
        assert!(checker.is_allowed_path(Path::new("/home/user/project")));
        assert!(checker.is_allowed_path(Path::new("/home/user/project/src")));
        assert!(checker.is_allowed_path(Path::new("/home/user/project/src/lib.rs")));
        assert!(!checker.is_allowed_path(Path::new("/home/user/other")));
    }

    #[test]
    fn test_config_accessor() {
        let config = strict_config();
        let checker = PermissionChecker::new(config.clone());
        assert_eq!(checker.config().mode, SandboxMode::Strict);
        assert_eq!(checker.config().allowed_paths.len(), 1);
    }

    #[test]
    fn test_strict_write_to_allowed_path() {
        let checker = PermissionChecker::new(strict_config());
        // Write to an allowed path (not denied) should succeed in strict mode
        assert!(
            checker
                .check_path(Path::new("/home/user/project/src/main.rs"), true)
                .is_ok()
        );
    }

    #[test]
    fn test_strict_write_to_denied_path() {
        let checker = PermissionChecker::new(strict_config());
        assert!(
            checker
                .check_path(Path::new("/home/user/project/.env"), true)
                .is_err()
        );
    }
}
