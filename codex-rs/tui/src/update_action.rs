#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;
use std::fmt;
#[cfg(any(not(debug_assertions), test))]
use std::path::Path;
use std::path::PathBuf;
#[cfg(any(not(debug_assertions), test))]
use std::process::Command;

#[cfg(any(not(debug_assertions), test))]
const MANAGED_PACKAGE_ROOT_ENV: &str = "CODEX_MANAGED_PACKAGE_ROOT";

/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g @openai/codex@latest`.
    NpmGlobalLatest,
    /// Update via `bun install -g @openai/codex@latest`.
    BunGlobalLatest,
    /// Update via `brew upgrade codex`.
    BrewUpgrade,
    /// Update via `curl -fsSL https://chatgpt.com/codex/install.sh | sh`.
    StandaloneUnix,
    /// Update via `irm https://chatgpt.com/codex/install.ps1|iex`.
    StandaloneWindows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateActionStatus {
    Ready(UpdateAction),
    Blocked(UpdateBlocker),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateBlocker {
    NpmGlobalRootMismatch {
        running_package_root: PathBuf,
        npm_package_root: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct PromptedUpdate {
    pub action: UpdateAction,
    pub target_version: String,
    pub version_file: PathBuf,
}

impl UpdateBlocker {
    pub fn remediation_lines(&self) -> Vec<String> {
        match self {
            Self::NpmGlobalRootMismatch {
                running_package_root,
                npm_package_root,
            } => vec![
                "You are running Codex from:".to_string(),
                format!("  {}", running_package_root.display()),
                "but `npm install -g @openai/codex@latest` would update:".to_string(),
                format!("  {}", npm_package_root.display()),
                "Fix your shell PATH or remove the stale Codex install, then restart Codex."
                    .to_string(),
            ],
        }
    }
}

impl fmt::Display for UpdateBlocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NpmGlobalRootMismatch {
                running_package_root,
                npm_package_root,
            } => write!(
                f,
                "You are running Codex from {}, but `npm install -g @openai/codex@latest` would update {}. Fix your shell PATH or remove the stale Codex install, then restart Codex.",
                running_package_root.display(),
                npm_package_root.display(),
            ),
        }
    }
}

impl UpdateAction {
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match context {
            InstallContext::Npm => Some(UpdateAction::NpmGlobalLatest),
            InstallContext::Bun => Some(UpdateAction::BunGlobalLatest),
            InstallContext::Brew => Some(UpdateAction::BrewUpgrade),
            InstallContext::Standalone { platform, .. } => Some(match platform {
                StandalonePlatform::Unix => UpdateAction::StandaloneUnix,
                StandalonePlatform::Windows => UpdateAction::StandaloneWindows,
            }),
            InstallContext::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
            UpdateAction::StandaloneUnix => (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"],
            ),
            UpdateAction::StandaloneWindows => (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(any(not(debug_assertions), test))]
#[cfg_attr(test, allow(dead_code))]
pub fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current())
}

#[cfg(any(not(debug_assertions), test))]
pub fn get_update_action_status() -> UpdateActionStatus {
    let Some(action) = UpdateAction::from_install_context(InstallContext::current()) else {
        return UpdateActionStatus::Unavailable;
    };

    if let Some(blocker) = update_blocker(action) {
        UpdateActionStatus::Blocked(blocker)
    } else {
        UpdateActionStatus::Ready(action)
    }
}

#[cfg(any(not(debug_assertions), test))]
fn update_blocker(action: UpdateAction) -> Option<UpdateBlocker> {
    match action {
        UpdateAction::NpmGlobalLatest => npm_global_root_mismatch(),
        UpdateAction::BunGlobalLatest
        | UpdateAction::BrewUpgrade
        | UpdateAction::StandaloneUnix
        | UpdateAction::StandaloneWindows => None,
    }
}

#[cfg(any(not(debug_assertions), test))]
fn npm_global_root_mismatch() -> Option<UpdateBlocker> {
    let running_package_root = std::env::var_os(MANAGED_PACKAGE_ROOT_ENV)?;
    let running_package_root = std::fs::canonicalize(PathBuf::from(running_package_root)).ok()?;
    let npm_global_root = npm_global_root()?;
    mismatch_from_npm_roots(&running_package_root, &npm_global_root)
}

#[cfg(any(not(debug_assertions), test))]
fn npm_global_root() -> Option<PathBuf> {
    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/C", "npm", "root", "-g"])
        .output()
        .ok()?;
    #[cfg(not(windows))]
    let output = Command::new("npm").args(["root", "-g"]).output().ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let npm_global_root = stdout.trim();
    if npm_global_root.is_empty() {
        return None;
    }
    std::fs::canonicalize(npm_global_root).ok()
}

#[cfg(any(not(debug_assertions), test))]
fn mismatch_from_npm_roots(
    running_package_root: &Path,
    npm_global_root: &Path,
) -> Option<UpdateBlocker> {
    let npm_package_root = npm_global_root.join("@openai").join("codex");
    (running_package_root != npm_package_root.as_path()).then(|| {
        UpdateBlocker::NpmGlobalRootMismatch {
            running_package_root: running_package_root.to_path_buf(),
            npm_package_root,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn maps_install_context_to_update_action() {
        let native_release_dir = PathBuf::from("/tmp/native-release");

        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Other),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Npm),
            Some(UpdateAction::NpmGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Bun),
            Some(UpdateAction::BunGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Brew),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneUnix)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Windows,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneWindows)
        );
    }

    #[test]
    fn standalone_update_commands_rerun_latest_installer() {
        assert_eq!(
            UpdateAction::StandaloneUnix.command_args(),
            (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"][..],
            )
        );
        assert_eq!(
            UpdateAction::StandaloneWindows.command_args(),
            (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"][..],
            )
        );
    }

    #[test]
    fn npm_root_mismatch_is_blocked_when_update_targets_another_install() {
        assert_eq!(
            mismatch_from_npm_roots(
                Path::new("/prefix-a/lib/node_modules/@openai/codex"),
                Path::new("/prefix-b/lib/node_modules"),
            ),
            Some(UpdateBlocker::NpmGlobalRootMismatch {
                running_package_root: PathBuf::from("/prefix-a/lib/node_modules/@openai/codex"),
                npm_package_root: PathBuf::from("/prefix-b/lib/node_modules/@openai/codex"),
            })
        );
    }

    #[test]
    fn npm_root_match_keeps_update_available() {
        assert_eq!(
            mismatch_from_npm_roots(
                Path::new("/prefix-a/lib/node_modules/@openai/codex"),
                Path::new("/prefix-a/lib/node_modules"),
            ),
            None
        );
    }
}
