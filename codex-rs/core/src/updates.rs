use std::path::Path;
use std::path::PathBuf;

const NPM_REINSTALL_INSTRUCTIONS: &str = "npm install -g @openai/codex@latest";
const BUN_REINSTALL_INSTRUCTIONS: &str = "bun install -g @openai/codex@latest";
const BREW_REINSTALL_INSTRUCTIONS: &str = "brew upgrade --cask codex";

/// CLI update actions that modify the install tracked by the current binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g @openai/codex@latest`.
    NpmGlobalLatest,
    /// Update via `bun install -g @openai/codex@latest`.
    BunGlobalLatest,
    /// Update via `brew upgrade --cask codex`.
    BrewUpgrade,
}

impl UpdateAction {
    /// Returns a reusable literal suitable for user-visible reinstall prompts.
    pub fn reinstall_instruction(self) -> &'static str {
        match self {
            UpdateAction::NpmGlobalLatest => NPM_REINSTALL_INSTRUCTIONS,
            UpdateAction::BunGlobalLatest => BUN_REINSTALL_INSTRUCTIONS,
            UpdateAction::BrewUpgrade => BREW_REINSTALL_INSTRUCTIONS,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

/// Detects the update path used to install the current binary.
pub fn get_update_action() -> Option<UpdateAction> {
    let exe = std::env::current_exe().unwrap_or_default();
    let managed_by_npm = std::env::var_os("CODEX_MANAGED_BY_NPM").is_some();
    let managed_by_bun = std::env::var_os("CODEX_MANAGED_BY_BUN").is_some();
    if managed_by_npm {
        Some(UpdateAction::NpmGlobalLatest)
    } else if managed_by_bun {
        Some(UpdateAction::BunGlobalLatest)
    } else if cfg!(target_os = "macos")
        && (exe.starts_with("/opt/homebrew") || exe.starts_with("/usr/local"))
    {
        Some(UpdateAction::BrewUpgrade)
    } else {
        None
    }
}

/// Returns a user-friendly reinstall hint based on the detected installation method.
pub fn get_reinstall_hint() -> String {
    if let Some(action) = get_update_action() {
        action.reinstall_instruction().to_string()
    } else {
        "https://developers.openai.com/codex/cli/".to_string()
    }
}

/// Searches PATH for a valid codex binary.
///
/// On Windows, searches for `.cmd` and `.ps1` shims (used by npm/bun) and runs them
/// with `--print-binary-path` to get the actual binary location.
///
/// Windows package managers (npm, bun) install binaries differently than Unix:
/// - On Unix: direct executable file (e.g., `/usr/local/bin/codex`)
/// - On Windows: wrapper scripts that invoke the actual binary
///   - `.cmd` files (batch scripts for npm on Windows)
///   - `.ps1` files (PowerShell scripts for some package managers)
///   These shims know where the real binary is located, so we execute them
///   with `--print-binary-path` to discover the actual executable path.
///
/// On Unix, searches for the `codex` binary directly.
pub(crate) async fn try_codex_in_path() -> Option<PathBuf> {
    use std::time::Duration;

    let path_var = std::env::var_os("PATH")?;

    let search_names = if cfg!(windows) {
        vec!["codex.exe", "codex.cmd", "codex.ps1", "codex"]
    } else {
        vec!["codex"]
    };

    for exe_name in search_names {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(exe_name);
            if !candidate.is_file() {
                continue;
            }

            let canonical = candidate.canonicalize().ok()?;

            // Verify this is a valid codex binary by running --print-binary-path
            // Use timeout to prevent hanging if the binary is unresponsive
            if let Ok(Some(exec_path)) =
                tokio::time::timeout(Duration::from_secs(5), try_print_binary_path(&canonical))
                    .await
            {
                return Some(exec_path);
            }
        }
    }
    None
}

async fn try_print_binary_path(candidate: &Path) -> Option<PathBuf> {
    let output = tokio::process::Command::new(candidate)
        .arg("--print-binary-path")
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    let result_path = PathBuf::from(trimmed);
    if !result_path.is_file() {
        return None;
    }

    result_path.canonicalize().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn get_update_action_without_env_var() {
        temp_env::with_var_unset("CODEX_MANAGED_BY_NPM", || {
            temp_env::with_var_unset("CODEX_MANAGED_BY_BUN", || {
                assert_eq!(get_update_action(), None);
            });
        });
    }

    #[test]
    #[serial]
    fn get_update_action_prefers_npm_when_flagged() {
        temp_env::with_var("CODEX_MANAGED_BY_NPM", Some("1"), || {
            assert_eq!(get_update_action(), Some(UpdateAction::NpmGlobalLatest));
        });
    }

    #[test]
    #[serial]
    fn get_update_action_prefers_bun_when_flagged() {
        temp_env::with_var_unset("CODEX_MANAGED_BY_NPM", || {
            temp_env::with_var("CODEX_MANAGED_BY_BUN", Some("1"), || {
                assert_eq!(get_update_action(), Some(UpdateAction::BunGlobalLatest));
            });
        });
    }

    #[tokio::test]
    async fn try_print_binary_path_returns_none_for_nonexistent() {
        let nonexistent = Path::new("/nonexistent/path/to/binary");
        assert_eq!(try_print_binary_path(nonexistent).await, None);
    }

    #[tokio::test]
    async fn try_codex_in_path_does_not_panic() {
        // This is hard to test without a real codex binary in PATH,
        // but we can at least verify the function doesn't panic
        let _result = try_codex_in_path().await;
        // If we get here without panicking, the test passes
    }

    #[tokio::test]
    #[serial]
    async fn try_codex_in_path_returns_none_when_not_found() {
        // Set PATH to an empty directory where codex doesn't exist
        use tempfile::TempDir;
        let empty_dir = TempDir::new().expect("create temp dir");
        let empty_path = empty_dir.path().display().to_string();

        temp_env::with_var("PATH", Some(empty_path), || async {
            let result = try_codex_in_path().await;
            assert_eq!(result, None, "Should return None when codex is not in PATH");
        })
        .await;
    }

    #[test]
    fn test_reinstall_instruction_format() {
        assert_eq!(
            UpdateAction::NpmGlobalLatest.reinstall_instruction(),
            "npm install -g @openai/codex@latest"
        );
        assert_eq!(
            UpdateAction::BunGlobalLatest.reinstall_instruction(),
            "bun install -g @openai/codex@latest"
        );
        assert_eq!(
            UpdateAction::BrewUpgrade.reinstall_instruction(),
            "brew upgrade --cask codex"
        );
    }

    #[test]
    fn test_command_str_format() {
        assert!(UpdateAction::NpmGlobalLatest.command_str().contains("npm"));
        assert!(UpdateAction::BunGlobalLatest.command_str().contains("bun"));
        assert!(UpdateAction::BrewUpgrade.command_str().contains("brew"));
    }

    #[test]
    fn test_command_args() {
        let (cmd, args) = UpdateAction::NpmGlobalLatest.command_args();
        assert_eq!(cmd, "npm");
        assert_eq!(args, &["install", "-g", "@openai/codex@latest"]);

        let (cmd, args) = UpdateAction::BunGlobalLatest.command_args();
        assert_eq!(cmd, "bun");
        assert_eq!(args, &["install", "-g", "@openai/codex@latest"]);

        let (cmd, args) = UpdateAction::BrewUpgrade.command_args();
        assert_eq!(cmd, "brew");
        assert_eq!(args, &["upgrade", "--cask", "codex"]);
    }

    #[test]
    #[serial]
    fn test_get_reinstall_hint_with_npm() {
        temp_env::with_var("CODEX_MANAGED_BY_NPM", Some("1"), || {
            let hint = get_reinstall_hint();
            assert_eq!(hint, "npm install -g @openai/codex@latest");
        });
    }

    #[test]
    #[serial]
    fn test_get_reinstall_hint_without_action() {
        temp_env::with_var_unset("CODEX_MANAGED_BY_NPM", || {
            temp_env::with_var_unset("CODEX_MANAGED_BY_BUN", || {
                let hint = get_reinstall_hint();
                assert_eq!(hint, "https://developers.openai.com/codex/cli/");
            });
        });
    }

    #[tokio::test]
    #[serial]
    async fn try_codex_in_path_returns_none_with_empty_path() {
        temp_env::with_var("PATH", Some(""), || async {
            let result = try_codex_in_path().await;
            assert_eq!(result, None, "Should return None when PATH is empty");
        })
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn try_codex_in_path_returns_none_with_unset_path() {
        temp_env::with_var_unset("PATH", || async {
            let result = try_codex_in_path().await;
            assert_eq!(result, None, "Should return None when PATH is unset");
        })
        .await;
    }

    #[tokio::test]
    async fn try_print_binary_path_with_directory_returns_none() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().expect("create temp dir");
        // Pass a directory instead of a file
        let result = try_print_binary_path(temp_dir.path()).await;
        assert_eq!(result, None, "Should return None for directories");
    }
}
