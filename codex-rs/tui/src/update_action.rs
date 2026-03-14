#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::NativePlatform;

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
    NativeUnix,
    /// Update via `irm https://chatgpt.com/codex/install.ps1|iex`.
    NativeWindows,
}

impl UpdateAction {
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match context {
            InstallContext::Npm => Some(UpdateAction::NpmGlobalLatest),
            InstallContext::Bun => Some(UpdateAction::BunGlobalLatest),
            InstallContext::Brew => Some(UpdateAction::BrewUpgrade),
            InstallContext::Native { platform, .. } => Some(match platform {
                NativePlatform::Unix => UpdateAction::NativeUnix,
                NativePlatform::Windows => UpdateAction::NativeWindows,
            }),
            InstallContext::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
            UpdateAction::NativeUnix => (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"],
            ),
            UpdateAction::NativeWindows => (
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

#[cfg(not(debug_assertions))]
pub(crate) fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current())
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
            UpdateAction::from_install_context(&InstallContext::Native {
                platform: NativePlatform::Unix,
                release_dir: native_release_dir.clone(),
                version: "1.2.3".to_string(),
                target: "x86_64-unknown-linux-musl".to_string(),
                rg_command: native_release_dir.join("rg"),
            }),
            Some(UpdateAction::NativeUnix)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Native {
                platform: NativePlatform::Windows,
                release_dir: native_release_dir.clone(),
                version: "1.2.3".to_string(),
                target: "x86_64-pc-windows-msvc".to_string(),
                rg_command: native_release_dir.join("rg.exe"),
            }),
            Some(UpdateAction::NativeWindows)
        );
    }
}
