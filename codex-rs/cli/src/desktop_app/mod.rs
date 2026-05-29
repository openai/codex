#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod windows;

/// Run the app install/open logic for the current OS.
#[cfg(target_os = "macos")]
pub async fn run_app_open_or_install(
    workspace: std::path::PathBuf,
    download_url_override: Option<String>,
) -> anyhow::Result<()> {
    mac::run_mac_app_open_or_install(workspace, download_url_override).await
}

/// Run the app install/open logic for the current OS.
#[cfg(target_os = "windows")]
pub async fn run_app_open_or_install(
    workspace: std::path::PathBuf,
    download_url_override: Option<String>,
) -> anyhow::Result<()> {
    windows::run_windows_app_open_or_install(workspace, download_url_override).await
}
