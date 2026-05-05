#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
mod open_args;
#[cfg(target_os = "windows")]
mod windows;

use std::path::PathBuf;

use crate::app_selector::DesktopAppSelector;

/// Run the app install/open logic for the current OS.
#[cfg(target_os = "macos")]
pub async fn run_app_open_or_install(
    workspace: PathBuf,
    download_url_override: Option<String>,
    selector: Option<DesktopAppSelector>,
    config_overrides: Vec<String>,
) -> anyhow::Result<()> {
    mac::run_mac_app_open_or_install(workspace, download_url_override, selector, config_overrides)
        .await
}

/// Run the app install/open logic for the current OS.
#[cfg(target_os = "windows")]
pub async fn run_app_open_or_install(
    workspace: PathBuf,
    download_url_override: Option<String>,
    selector: Option<DesktopAppSelector>,
    config_overrides: Vec<String>,
) -> anyhow::Result<()> {
    windows::run_windows_app_open_or_install(
        workspace,
        download_url_override,
        selector,
        config_overrides,
    )
    .await
}
