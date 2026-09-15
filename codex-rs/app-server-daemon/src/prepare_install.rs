//! Prepares complete local CLI packages for a stopped daemon. Legacy standalone
//! installations are left to their installer; new releases are immutable.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_install_context::CodexPackageManifest;
use codex_install_context::InstallContext;

use crate::Daemon;
use crate::install_lock::acquire_install_lock;
use crate::managed_install;
use crate::settings::DaemonSettings;

/// Prepare a missing package while the caller holds the daemon operation lock.
pub(super) async fn prepare(daemon: &Daemon, settings: &DaemonSettings) -> Result<()> {
    let source = InstallContext::current().package_layout.as_ref();
    prepare_from_package(
        daemon,
        settings,
        source.map(|layout| layout.package_dir.as_path()),
        &std::env::current_exe()?,
    )
    .await
}

async fn prepare_from_package(
    daemon: &Daemon,
    settings: &DaemonSettings,
    source: Option<&Path>,
    running_exe: &Path,
) -> Result<()> {
    let home = daemon
        .settings_file
        .parent()
        .and_then(Path::parent)
        .context("daemon settings path has no Codex home")?;
    let root = managed_install::package_root(home);
    anyhow::ensure!(
        daemon.managed_codex_bin.starts_with(&root),
        "daemon package location changed; retry the command"
    );
    if !root.ends_with("app-server-daemon")
        || daemon.running_backend_instance(settings).await?.is_some()
        || crate::client::probe(&daemon.socket_path).await.is_ok()
    {
        return Ok(());
    }
    if !matches!(root.join("current").symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        || ["daemon.pid", "daemon.stderr.log", "daemon-updater.pid", "daemon-updater.stderr.log"]
            .iter().any(|name| !matches!(home.join("app-server-daemon").join(name).symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound))
    {
        daemon.ensure_managed_codex_bin()?;
        return Ok(());
    }
    std::fs::create_dir_all(&root)?;
    let _install_lock = acquire_install_lock(&root).await?;
    // An older CLI may have launched a legacy daemon while this CLI waited.
    anyhow::ensure!(
        managed_install::package_root(home) == root,
        "daemon package location changed; retry the command"
    );
    anyhow::ensure!(
        daemon.running_backend_instance(settings).await?.is_none()
            && crate::client::probe(&daemon.socket_path).await.is_err(),
        "an app server started while preparing the daemon; retry the command"
    );
    let selected = managed_install::managed_codex_bin(home);
    let missing = !selected.is_file();
    if !missing {
        return Ok(());
    }
    anyhow::ensure!(
        matches!(root.join("current").symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound),
        "the selected daemon package is incomplete; repair its installation before starting"
    );
    let source = source.context(
        "this CLI has no complete local package; install a packaged Codex CLI or use the standalone installer",
    )?;
    anyhow::ensure!(
        !root.canonicalize()?.starts_with(source.canonicalize()?),
        "CODEX_HOME must be outside the source CLI package"
    );
    let manifest_bytes = std::fs::read(source.join("codex-package.json"))?;
    let manifest: CodexPackageManifest = serde_json::from_slice(&manifest_bytes)?;
    let version = manifest.version.to_string();
    let target = platform_target()?;
    let metadata: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    let entrypoint = if cfg!(windows) {
        "bin/codex.exe"
    } else {
        "bin/codex"
    };
    anyhow::ensure!(
        metadata["target"] == target && metadata["entrypoint"] == entrypoint,
        "the CLI package does not match this platform or executable"
    );
    validate_package(source)?;
    eprintln!(
        "Installing daemon from CLI version {version} into {}...",
        root.display()
    );
    let stable = stable_version(&version).is_some();
    let running_identity = managed_install::executable_identity(running_exe).await?;
    let releases = root.join("releases");
    std::fs::create_dir_all(&releases)?;
    let stage = tempfile::Builder::new()
        .prefix(".staging.")
        .tempdir_in(&releases)?;
    let digest = package_tree(source, Some(stage.path()))?;
    validate_package(stage.path())?;
    let staged_exe = if cfg!(target_os = "macos") && stage.path().join("CodexCLI.app").is_dir() {
        stage.path().join("CodexCLI.app/Contents/MacOS/codex")
    } else {
        stage.path().join(entrypoint)
    };
    anyhow::ensure!(
        package_tree(source, /*destination*/ None)? == digest
            && std::fs::read(stage.path().join("codex-package.json"))? == manifest_bytes
            && managed_install::executable_identity(&staged_exe).await? == running_identity,
        "the CLI package changed while preparing the daemon or differs from the running executable"
    );
    let binary_version =
        managed_install::managed_codex_version(&stage.path().join(entrypoint)).await?;
    anyhow::ensure!(
        !stable || version == binary_version,
        "the CLI package version does not match its executable"
    );
    let name = if stable {
        format!("{version}-{target}")
    } else {
        format!("local-{digest}-{target}")
    };
    let release = releases.join(&name);
    if release.try_exists()? {
        anyhow::ensure!(
            !release.symlink_metadata()?.file_type().is_symlink()
                && package_tree(&release, /*destination*/ None)? == digest,
            "an existing daemon release has different contents; refusing to overwrite it"
        );
    } else {
        #[cfg(unix)]
        if !stage.path().join("codex").exists() {
            std::os::unix::fs::symlink("bin/codex", stage.path().join("codex"))?;
        }
        std::fs::rename(stage.path(), &release)?;
    }
    let standalone = home.join("packages/standalone");
    let canonical_source = source.canonicalize()?;
    let follows_latest = stable
        && (standalone.join("current").canonicalize().ok().as_deref()
            != Some(canonical_source.as_path())
            || std::fs::read_to_string(standalone.join("auto-update-version"))
                .ok()
                .as_deref()
                == canonical_source.file_name().and_then(|name| name.to_str()));
    anyhow::ensure!(
        managed_install::package_root(home) == root
            && daemon.running_backend_instance(settings).await?.is_none()
            && crate::client::probe(&daemon.socket_path).await.is_err(),
        "daemon state changed while preparing its package; retry the command"
    );
    let marker = root.join("auto-update-version");
    if follows_latest {
        let temporary = tempfile::NamedTempFile::new_in(&root)?;
        std::fs::write(temporary.path(), &name)?;
        temporary.persist(marker)?;
    } else if marker.exists() {
        std::fs::remove_file(marker)?;
    }
    #[cfg(unix)]
    {
        let temporary = tempfile::TempDir::new_in(&root)?;
        let link = temporary.path().join("current");
        std::os::unix::fs::symlink(&release, &link)?;
        std::fs::rename(link, root.join("current"))?;
    }
    #[cfg(windows)]
    windows::select_release(&root, &release)?;
    Ok(())
}

/// Hash the complete tree and optionally copy those same bytes. Relative file
/// links are materialized; escaping links and directory links are rejected.
fn package_tree(root: &Path, destination: Option<&Path>) -> Result<String> {
    let canonical_root = root.canonicalize()?;
    let mut hasher = blake3::Hasher::new();
    let mut paths = vec![root.to_path_buf()];
    while let Some(path) = paths.pop() {
        let relative = path.strip_prefix(root)?;
        // The Unix installer adds this alias outside the package layout.
        if cfg!(unix)
            && relative == Path::new("codex")
            && std::fs::read_link(&path).ok().as_deref() == Some(Path::new("bin/codex"))
        {
            continue;
        }
        anyhow::ensure!(
            path.canonicalize()?.starts_with(&canonical_root),
            "package link escapes its root"
        );
        let metadata = path.metadata()?;
        hasher.update(relative.as_os_str().as_encoded_bytes());
        hasher.update(&[0]);
        if metadata.is_dir() {
            anyhow::ensure!(
                !path.symlink_metadata()?.file_type().is_symlink(),
                "package contains a directory link"
            );
            hasher.update(b"directory");
            if let Some(destination) = destination
                && !relative.as_os_str().is_empty()
            {
                std::fs::create_dir(destination.join(relative))?;
            }
            let mut entries = std::fs::read_dir(&path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            entries.sort();
            paths.extend(entries);
        } else {
            anyhow::ensure!(metadata.is_file(), "package contains an unsupported file");
            let bytes = std::fs::read(&path)?;
            hasher.update(b"file");
            hasher.update(blake3::hash(&bytes).as_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                hasher.update(&(metadata.permissions().mode() & 0o777).to_le_bytes());
            }
            if let Some(destination) = destination {
                let target = destination.join(relative);
                std::fs::write(&target, bytes)?;
                std::fs::set_permissions(target, metadata.permissions())?;
            }
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn stable_version(value: &str) -> Option<semver::Version> {
    let version = semver::Version::parse(value).ok()?;
    (version.pre.is_empty()
        && version.build.is_empty()
        && (version.major, version.minor, version.patch) != (0, 0, 0))
        .then_some(version)
}

fn validate_package(root: &Path) -> Result<()> {
    let mut names = vec![
        "codex-package.json",
        if cfg!(windows) {
            "bin/codex.exe"
        } else {
            "bin/codex"
        },
        if cfg!(windows) {
            "bin/codex-code-mode-host.exe"
        } else {
            "bin/codex-code-mode-host"
        },
        if cfg!(windows) {
            "codex-path/rg.exe"
        } else {
            "codex-path/rg"
        },
    ];
    if cfg!(windows) {
        names.extend([
            "codex-resources/codex-command-runner.exe",
            "codex-resources/codex-windows-sandbox-setup.exe",
        ]);
    } else if cfg!(target_os = "linux") {
        names.push("codex-resources/bwrap");
    }
    for name in names {
        if !root.join(name).is_file() {
            return Err(anyhow!(
                "local Codex package is missing {name}; reinstall the CLI or use the standalone installer"
            ));
        }
        #[cfg(unix)]
        if name != "codex-package.json" {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(root.join(name))?.permissions().mode() & 0o111 == 0 {
                return Err(anyhow!(
                    "local Codex package file {name} is not executable; reinstall the CLI or use the standalone installer"
                ));
            }
        }
    }
    Ok(())
}

fn platform_target() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("linux", "aarch64") if cfg!(target_env = "gnu") => Ok("aarch64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-musl"),
        ("linux", "x86_64") if cfg!(target_env = "gnu") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-musl"),
        ("windows", "aarch64") => Ok("aarch64-pc-windows-msvc"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        (os, arch) => Err(anyhow!("unsupported packaged daemon platform {os}/{arch}")),
    }
}

#[cfg(windows)]
#[path = "prepare_install_windows.rs"]
mod windows;

#[cfg(test)]
#[path = "prepare_install_tests.rs"]
mod tests;
