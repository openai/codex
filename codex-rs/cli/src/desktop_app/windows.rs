use anyhow::Context as _;
use std::path::PathBuf;
use tokio::process::Command;

const CODEX_WINDOWS_INSTALLER_URL: &str =
    "https://get.microsoft.com/installer/download/9PLM9XGG6VKS?cid=website_cta_psi";
const CODEX_MICROSOFT_STORE_WEB_URL: &str = "https://apps.microsoft.com/detail/9plm9xgg6vks";

pub async fn run_windows_app_open_or_install(
    workspace: PathBuf,
    download_url_override: Option<String>,
) -> anyhow::Result<()> {
    if let Some(app_id) = find_codex_app_id().await? {
        eprintln!("Opening Codex Desktop...");
        open_installed_codex_app(&app_id).await?;
        eprintln!(
            "In Codex Desktop, open workspace {workspace}.",
            workspace = workspace.display()
        );
        return Ok(());
    }

    eprintln!("Codex Desktop not found; opening Windows installer...");
    let download_url = download_url_override
        .as_deref()
        .unwrap_or(CODEX_WINDOWS_INSTALLER_URL);
    if open_url(download_url).await.is_err() && download_url_override.is_none() {
        open_url(CODEX_MICROSOFT_STORE_WEB_URL).await?;
    }
    eprintln!(
        "After installing Codex Desktop, open workspace {workspace}.",
        workspace = workspace.display()
    );
    Ok(())
}

async fn find_codex_app_id() -> anyhow::Result<Option<String>> {
    let output = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("Get-StartApps -Name 'Codex' | Select-Object -First 1 -ExpandProperty AppID")
        .output()
        .await
        .context("failed to invoke `powershell.exe`")?;

    if !output.status.success() {
        return Ok(None);
    }

    let app_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if app_id.is_empty() {
        Ok(None)
    } else {
        Ok(Some(app_id))
    }
}

async fn open_installed_codex_app(app_id: &str) -> anyhow::Result<()> {
    let target = format!("shell:AppsFolder\\{app_id}");
    let status = Command::new("explorer.exe")
        .arg(target)
        .status()
        .await
        .context("failed to invoke `explorer.exe`")?;

    if status.success() {
        return Ok(());
    }
    anyhow::bail!("explorer.exe failed with {status}");
}

async fn open_url(url: &str) -> anyhow::Result<()> {
    let status = Command::new("explorer.exe")
        .arg(url)
        .status()
        .await
        .with_context(|| format!("failed to open {url}"))?;

    if status.success() {
        return Ok(());
    }
    anyhow::bail!("failed to open {url} with {status}");
}
