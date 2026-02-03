use clap::Parser;
use std::path::PathBuf;

const DEFAULT_CODEX_DMG_URL: &str = "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg";

#[derive(Debug, Parser)]
pub struct AppCommand {
    /// Workspace path to open in Codex Desktop.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Override the macOS DMG download URL (advanced).
    #[arg(long, default_value = DEFAULT_CODEX_DMG_URL)]
    pub download_url: String,
}

#[cfg(target_os = "macos")]
pub async fn run_app(cmd: AppCommand) -> anyhow::Result<()> {
    let workspace = std::fs::canonicalize(&cmd.path).unwrap_or(cmd.path);
    crate::desktop_app::run_app_open_or_install(workspace, cmd.download_url).await
}
