//! Uninstalls the legacy Windows sandbox without loading config or removing user data.

use clap::Args;

#[derive(Debug, Args)]
#[command(
    about = "Remove the legacy Windows sandbox's machine-wide accounts and network rules",
    after_help = "Run as administrator after stopping apps using the legacy sandbox and its provisioning service, if installed.\nPreserves Codex home, including sandbox directories and filesystem permissions."
)]
pub(crate) struct SandboxUninstallCommand {}

impl SandboxUninstallCommand {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        #[cfg(target_os = "windows")]
        {
            codex_windows_sandbox::clean_up_legacy_windows_sandbox()
        }
        #[cfg(not(target_os = "windows"))]
        anyhow::bail!("`codex sandbox uninstall` is only supported on Windows")
    }
}
