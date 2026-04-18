#[cfg(target_os = "macos")]
mod mac;

/// Run the app install/open logic for the current OS.
#[cfg(target_os = "macos")]
pub async fn run_app_open_or_install(
    workspace: std::path::PathBuf,
    download_url_override: Option<String>,
) -> anyhow::Result<()> {
    mac::run_mac_app_open_or_install(workspace, download_url_override).await
}
