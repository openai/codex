use anyhow::Context as _;
use std::path::Path;
use std::path::PathBuf;
use tempfile::Builder;
use tokio::process::Command;

pub async fn run_mac_app_open_or_install(
    workspace: PathBuf,
    download_url: String,
) -> anyhow::Result<()> {
    if let Some(app_path) = find_existing_codex_app_path() {
        eprintln!(
            "Opening Codex Desktop at {app_path}...",
            app_path = app_path.display()
        );
        open_codex_app(&app_path, &workspace).await?;
        return Ok(());
    }
    eprintln!("Codex Desktop not found; downloading installer...");
    let installed_app = download_and_install_codex_to_user_applications(&download_url)
        .await
        .context("failed to download/install Codex Desktop")?;
    eprintln!(
        "Launching Codex Desktop from {installed_app}...",
        installed_app = installed_app.display()
    );
    open_codex_app(&installed_app, &workspace).await?;
    Ok(())
}

fn find_existing_codex_app_path() -> Option<PathBuf> {
    candidate_codex_app_paths()
        .into_iter()
        .find(|candidate| candidate.is_dir())
}

fn candidate_codex_app_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/Applications/Codex.app")];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Applications").join("Codex.app"));
    }
    paths
}

async fn open_codex_app(app_path: &Path, workspace: &Path) -> anyhow::Result<()> {
    eprintln!(
        "Opening workspace {workspace}...",
        workspace = workspace.display()
    );
    let status = Command::new("open")
        .arg("-a")
        .arg(app_path)
        .arg(workspace)
        .status()
        .await
        .context("failed to invoke `open`")?;

    if status.success() {
        return Ok(());
    }

    anyhow::bail!(
        "`open -a {app_path} {workspace}` exited with {status}",
        app_path = app_path.display(),
        workspace = workspace.display()
    );
}

async fn download_and_install_codex_to_user_applications(dmg_url: &str) -> anyhow::Result<PathBuf> {
    let temp_dir = Builder::new()
        .prefix("codex-app-installer-")
        .tempdir()
        .context("failed to create temp dir")?;
    let tmp_root = temp_dir.path().to_path_buf();
    let _temp_dir = temp_dir;

    let dmg_path = tmp_root.join("Codex.dmg");
    download_dmg(dmg_url, &dmg_path).await?;

    eprintln!("Mounting Codex Desktop installer...");
    let mount_point = mount_dmg(&dmg_path).await?;
    eprintln!(
        "Installer mounted at {mount_point}.",
        mount_point = mount_point.display()
    );
    let result = async {
        let app_in_volume = find_codex_app_in_mount(&mount_point)
            .context("failed to locate Codex.app in mounted dmg")?;
        install_codex_app_bundle(&app_in_volume).await
    }
    .await;

    let detach_result = detach_dmg(&mount_point).await;
    if let Err(err) = detach_result {
        eprintln!(
            "warning: failed to detach dmg at {mount_point}: {err}",
            mount_point = mount_point.display()
        );
    }

    result
}

async fn install_codex_app_bundle(app_in_volume: &Path) -> anyhow::Result<PathBuf> {
    for applications_dir in candidate_applications_dirs()? {
        eprintln!(
            "Installing Codex Desktop into {applications_dir}...",
            applications_dir = applications_dir.display()
        );
        std::fs::create_dir_all(&applications_dir).with_context(|| {
            format!(
                "failed to create applications dir {applications_dir}",
                applications_dir = applications_dir.display()
            )
        })?;

        let dest_app = applications_dir.join("Codex.app");
        if dest_app.is_dir() {
            return Ok(dest_app);
        }

        match copy_app_bundle(app_in_volume, &dest_app).await {
            Ok(()) => return Ok(dest_app),
            Err(err) => {
                eprintln!(
                    "warning: failed to install Codex.app to {applications_dir}: {err}",
                    applications_dir = applications_dir.display()
                );
            }
        }
    }

    anyhow::bail!("failed to install Codex.app to any applications directory");
}

fn candidate_applications_dirs() -> anyhow::Result<Vec<PathBuf>> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    dirs.push(user_applications_dir()?);
    Ok(dirs)
}

async fn download_dmg(url: &str, dest: &Path) -> anyhow::Result<()> {
    eprintln!("Downloading installer...");
    let status = Command::new("curl")
        .arg("-fL")
        .arg("--retry")
        .arg("3")
        .arg("--retry-delay")
        .arg("1")
        .arg("-o")
        .arg(dest)
        .arg(url)
        .status()
        .await
        .context("failed to invoke `curl`")?;

    if status.success() {
        return Ok(());
    }
    anyhow::bail!("curl download failed with {status}");
}

async fn mount_dmg(dmg_path: &Path) -> anyhow::Result<PathBuf> {
    let output = Command::new("hdiutil")
        .arg("attach")
        .arg("-nobrowse")
        .arg("-readonly")
        .arg(dmg_path)
        .output()
        .await
        .context("failed to invoke `hdiutil attach`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`hdiutil attach` failed with {status}: {stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_hdiutil_attach_mount_point(&stdout)
        .map(PathBuf::from)
        .with_context(|| format!("failed to parse mount point from hdiutil output:\n{stdout}"))
}

async fn detach_dmg(mount_point: &Path) -> anyhow::Result<()> {
    let status = Command::new("hdiutil")
        .arg("detach")
        .arg(mount_point)
        .status()
        .await
        .context("failed to invoke `hdiutil detach`")?;

    if status.success() {
        return Ok(());
    }
    anyhow::bail!("hdiutil detach failed with {status}");
}

fn find_codex_app_in_mount(mount_point: &Path) -> anyhow::Result<PathBuf> {
    let direct = mount_point.join("Codex.app");
    if direct.is_dir() {
        return Ok(direct);
    }

    for entry in std::fs::read_dir(mount_point).with_context(|| {
        format!(
            "failed to read {mount_point}",
            mount_point = mount_point.display()
        )
    })? {
        let entry = entry.context("failed to read mount directory entry")?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "app") && path.is_dir() {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "no .app bundle found at {mount_point}",
        mount_point = mount_point.display()
    );
}

async fn copy_app_bundle(src_app: &Path, dest_app: &Path) -> anyhow::Result<()> {
    let status = Command::new("ditto")
        .arg(src_app)
        .arg(dest_app)
        .status()
        .await
        .context("failed to invoke `ditto`")?;

    if status.success() {
        return Ok(());
    }
    anyhow::bail!("ditto copy failed with {status}");
}

fn user_applications_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join("Applications"))
}

fn parse_hdiutil_attach_mount_point(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        if !line.contains("/Volumes/") {
            return None;
        }
        if let Some((_, mount)) = line.rsplit_once('\t') {
            return Some(mount.trim().to_string());
        }
        line.split_whitespace()
            .find(|field| field.starts_with("/Volumes/"))
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::parse_hdiutil_attach_mount_point;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_mount_point_from_tab_separated_hdiutil_output() {
        let output = "/dev/disk2s1\tApple_HFS\tCodex\t/Volumes/Codex\n";
        assert_eq!(
            parse_hdiutil_attach_mount_point(output).as_deref(),
            Some("/Volumes/Codex")
        );
    }

    #[test]
    fn parses_mount_point_with_spaces() {
        let output = "/dev/disk2s1\tApple_HFS\tCodex Installer\t/Volumes/Codex Installer\n";
        assert_eq!(
            parse_hdiutil_attach_mount_point(output).as_deref(),
            Some("/Volumes/Codex Installer")
        );
    }
}
