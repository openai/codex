#[cfg(target_os = "macos")]
mod mac;

/// Run the app install/open logic for the current OS.
pub async fn run_app_open_or_install(
    workspace: std::path::PathBuf,
    download_url: String,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        mac::run_mac_app_open_or_install(workspace, download_url).await
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (workspace, download_url);
        anyhow::bail!(
            "`codex app` is only available on macOS right now. For the latest updates, see https://chatgpt.com/codex."
        );
    }
}
