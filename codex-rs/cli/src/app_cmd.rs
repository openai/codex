use clap::Parser;
use codex_utils_cli::CliConfigOverrides;
use std::path::PathBuf;

use crate::app_selector::app_selector_from_options;
use crate::app_selector::validate_download_url_selector_combination;

#[derive(Debug, Parser)]
pub struct AppCommand {
    /// Workspace path to open in Codex Desktop.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Override the app installer download URL (advanced).
    #[arg(long = "download-url")]
    pub download_url_override: Option<String>,

    /// Open a specific installed Codex app bundle by bundle identifier.
    #[arg(
        long = "bundle-id",
        value_name = "BUNDLE_ID",
        conflicts_with = "app_path"
    )]
    pub bundle_id: Option<String>,

    /// Open a specific installed Codex .app bundle at this path.
    #[arg(
        long = "app-path",
        value_name = "APP_PATH",
        conflicts_with = "bundle_id"
    )]
    pub app_path: Option<PathBuf>,
}

pub async fn run_app(cmd: AppCommand, config_overrides: CliConfigOverrides) -> anyhow::Result<()> {
    let selector =
        app_selector_from_options(cmd.bundle_id, cmd.app_path).map_err(anyhow::Error::msg)?;
    validate_download_url_selector_combination(&selector, &cmd.download_url_override)
        .map_err(anyhow::Error::msg)?;
    let workspace = std::fs::canonicalize(&cmd.path).unwrap_or(cmd.path);
    #[cfg(target_os = "macos")]
    {
        crate::desktop_app::run_app_open_or_install(
            workspace,
            cmd.download_url_override,
            selector,
            config_overrides.raw_overrides,
        )
        .await
    }
    #[cfg(target_os = "windows")]
    {
        crate::desktop_app::run_app_open_or_install(
            workspace,
            cmd.download_url_override,
            selector,
            config_overrides.raw_overrides,
        )
        .await
    }
}
