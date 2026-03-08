#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::NativePlatform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    Npm,
    Bun,
    Brew,
    NativeUnix,
    NativeWindows,
}

impl UpdateAction {
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match context {
            InstallContext::Native { platform, .. } => Some(match platform {
                NativePlatform::Unix => UpdateAction::NativeUnix,
                NativePlatform::Windows => UpdateAction::NativeWindows,
            }),
            InstallContext::Npm => Some(UpdateAction::Npm),
            InstallContext::Bun => Some(UpdateAction::Bun),
            InstallContext::Brew => Some(UpdateAction::Brew),
            InstallContext::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (String, Vec<String>) {
        match self {
            UpdateAction::Npm => (
                "npm".to_string(),
                vec!["install".into(), "-g".into(), "@openai/codex".into()],
            ),
            UpdateAction::Bun => (
                "bun".to_string(),
                vec!["install".into(), "-g".into(), "@openai/codex".into()],
            ),
            UpdateAction::NativeUnix => (
                "sh".to_string(),
                vec![
                    "-c".into(),
                    "curl -fsSL https://chatgpt.com/codex/install.sh | sh".into(),
                ],
            ),
            UpdateAction::NativeWindows => (
                "powershell".to_string(),
                vec![
                    "-c".into(),
                    "irm https://chatgpt.com/codex/install.ps1|iex".into(),
                ],
            ),
            UpdateAction::Brew => (
                "brew".to_string(),
                vec!["upgrade".into(), "--cask".into(), "codex".into()],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command.as_str()).chain(args.iter().map(String::as_str)))
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
            Some(UpdateAction::Npm)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Bun),
            Some(UpdateAction::Bun)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Brew),
            Some(UpdateAction::Brew)
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
