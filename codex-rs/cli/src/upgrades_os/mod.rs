use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod mac;

/// Run the app upgrade/installation logic for the current OS.
pub async fn run_app_upgrade(workspace: PathBuf, download_url: String) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        mac::run_mac_app_upgrade(workspace, download_url).await
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!(
            "`codex app` is only available on macOS right now. For the latest updates, see https://chatgpt.com/codex."
        );
    }
}
