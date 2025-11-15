//! Platform-specific program resolution for MCP server execution.
//!
//! This module provides a unified interface for resolving executable paths
//! across different operating systems. The key challenge it addresses is that
//! Windows cannot execute script files (e.g., `.cmd`, `.bat`) directly through
//! `Command::new()` without their file extensions, while Unix systems handle
//! scripts natively through shebangs.
//!
//! The `resolve` function abstracts these platform differences:
//! - On Unix: Returns the program unchanged (OS handles script execution)
//! - On Windows: Uses the `which` crate to resolve full paths including extensions

use std::collections::HashMap;
use std::ffi::OsString;

#[cfg(windows)]
use std::env;
#[cfg(windows)]
use tracing::debug;

/// Resolves a program to its executable path on Unix systems.
///
/// Unix systems handle PATH resolution and script execution natively through
/// the kernel's shebang (`#!`) mechanism, so this function simply returns
/// the program name unchanged.
#[cfg(unix)]
pub fn resolve(program: OsString, _env: &HashMap<String, String>) -> std::io::Result<OsString> {
    Ok(program)
}

/// Resolves a program to its executable path on Windows systems.
///
/// Windows requires explicit file extensions for script execution. This function
/// uses the `which` crate to search the `PATH` environment variable and find
/// the full path to the executable, including necessary script extensions
/// (`.cmd`, `.bat`, etc.) defined in `PATHEXT`.
///
/// This enables tools like `npx`, `pnpm`, and `yarn` to work correctly on Windows
/// without requiring users to specify full paths or extensions in their configuration.
#[cfg(windows)]
pub fn resolve(program: OsString, env: &HashMap<String, String>) -> std::io::Result<OsString> {
    // Get current directory for relative path resolution
    let cwd = env::current_dir()
        .map_err(|e| std::io::Error::other(format!("Failed to get current directory: {e}")))?;

    // Extract PATH from environment for search locations
    let search_path = env.get("PATH");

    // Attempt resolution via which crate
    match which::which_in(&program, search_path, &cwd) {
        Ok(resolved) => {
            debug!("Resolved {:?} to {:?}", program, resolved);
            Ok(resolved.into_os_string())
        }
        Err(e) => {
            debug!(
                "Failed to resolve {:?}: {}. Using original path",
                program, e
            );
            // Fallback to original program - let Command::new() handle the error
            Ok(program)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::create_env_for_mcp_server;
    use anyhow::Result;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::process::Command;

    /// Unix: Verifies the OS handles script execution without file extensions.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_unix_executes_script_without_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let mut cmd = Command::new(&env.program_name);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(output.is_ok(), "Unix should execute scripts directly");
        Ok(())
    }

    /// Windows: Verifies scripts fail to execute without the proper extension.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_fails_without_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let mut cmd = Command::new(&env.program_name);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(
            output.is_err(),
            "Windows requires .cmd/.bat extension for direct execution"
        );
        Ok(())
    }

    /// Windows: Verifies scripts with an explicit extension execute correctly.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_succeeds_with_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        // Append the `.cmd` extension to the program name
        let program_with_ext = format!("{}.cmd", env.program_name);
        let mut cmd = Command::new(&program_with_ext);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(
            output.is_ok(),
            "Windows should execute scripts when the extension is provided"
        );
        Ok(())
    }

    /// Verifies program resolution enables successful execution on all platforms.
    #[tokio::test]
    async fn test_resolved_program_executes_successfully() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let program = OsString::from(&env.program_name);

        // Apply platform-specific resolution
        let resolved = resolve(program, &env.mcp_env)?;

        // Verify resolved path executes successfully
        let mut cmd = Command::new(resolved);
        cmd.envs(&env.mcp_env);
        let output = cmd.output().await;

        assert!(
            output.is_ok(),
            "Resolved program should execute successfully"
        );
        Ok(())
    }

    // Test fixture for creating temporary executables in a controlled environment.
    struct TestExecutableEnv {
        // Held to prevent the temporary directory from being deleted.
        _temp_dir: TempDir,
        program_name: String,
        mcp_env: HashMap<String, String>,
    }

    impl TestExecutableEnv {
        const TEST_PROGRAM: &'static str = "test_mcp_server";

        fn new() -> Result<Self> {
            let temp_dir = TempDir::new()?;
            let dir_path = temp_dir.path();

            Self::create_executable(dir_path)?;

            // Build a clean environment with the temp dir in the PATH.
            let mut extra_env = HashMap::new();
            extra_env.insert("PATH".to_string(), Self::build_path(dir_path));

            #[cfg(windows)]
            extra_env.insert("PATHEXT".to_string(), Self::ensure_cmd_extension());

            let mcp_env = create_env_for_mcp_server(Some(extra_env), &[]);

            Ok(Self {
                _temp_dir: temp_dir,
                program_name: Self::TEST_PROGRAM.to_string(),
                mcp_env,
            })
        }

        /// Creates a simple, platform-specific executable script.
        fn create_executable(dir: &Path) -> Result<()> {
            #[cfg(windows)]
            {
                let file = dir.join(format!("{}.cmd", Self::TEST_PROGRAM));
                fs::write(&file, "@echo off\nexit 0")?;
            }

            #[cfg(unix)]
            {
                let file = dir.join(Self::TEST_PROGRAM);
                fs::write(&file, "#!/bin/sh\nexit 0")?;
                Self::set_executable(&file)?;
            }

            Ok(())
        }

        #[cfg(unix)]
        fn set_executable(path: &Path) -> Result<()> {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
            Ok(())
        }

        /// Prepends the given directory to the system's PATH variable.
        fn build_path(dir: &Path) -> String {
            let current = std::env::var("PATH").unwrap_or_default();
            let sep = if cfg!(windows) { ";" } else { ":" };
            format!("{}{sep}{current}", dir.to_string_lossy())
        }

        /// Ensures `.CMD` is in the `PATHEXT` variable on Windows for script discovery.
        #[cfg(windows)]
        fn ensure_cmd_extension() -> String {
            let current = std::env::var("PATHEXT").unwrap_or_default();
            if current.to_uppercase().contains(".CMD") {
                current
            } else {
                format!(".CMD;{current}")
            }
        }
    }
}
